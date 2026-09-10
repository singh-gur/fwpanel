//! Polkit authorization for the fwpanel service.
//!
//! Every check uses the caller's *actual* unique bus name taken from the
//! incoming message header — never a client-supplied identity. All failures
//! (polkit unavailable, malformed result, transport error) fail closed: no
//! authorization, no hardware access.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::StreamExt;
use zbus::match_rule::MatchRule;
use zbus::message::Type as MessageType;
use zbus::proxy;
use zbus::zvariant::Value;
use zbus::MessageStream;

pub const ACTION_READ: &str = "io.github.singh_gur.fwpanel.read-status";
pub const ACTION_SET_CHARGE_LIMIT: &str = "io.github.singh_gur.fwpanel.set-charge-limit";

/// CheckAuthorizationFlags: AllowUserInteraction.
const FLAG_ALLOW_USER_INTERACTION: u32 = 1;

/// Upper bound on the whole write-authorization exchange (agent prompts
/// included), per the approved recovery rules.
const WRITE_AUTH_TIMEOUT: Duration = Duration::from_secs(120);

/// Unique-per-process polkit cancellation ids.
static CANCELLATION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[proxy(
    interface = "org.freedesktop.PolicyKit1.Authority",
    default_service = "org.freedesktop.PolicyKit1",
    default_path = "/org/freedesktop/PolicyKit1/Authority"
)]
trait Authority {
    /// Subject `(sa{sv})`, action id, details `a{ss}`, flags, cancellation id.
    /// Returns AuthorizationResult `(ba{ss})` — denial is `is_authorized =
    /// false`, not a D-Bus error.
    async fn check_authorization(
        &self,
        subject: (&'static str, HashMap<&'static str, Value<'static>>),
        action_id: &'static str,
        details: HashMap<&'static str, &'static str>,
        flags: u32,
        cancellation_id: &str,
    ) -> zbus::Result<(bool, bool, HashMap<String, String>)>;

    /// Cancel a pending CheckAuthorization by its cancellation id.
    async fn cancel_check_authorization(&self, cancellation_id: &str) -> zbus::Result<()>;
}

pub struct Polkit {
    connection: zbus::Connection,
}

impl Polkit {
    pub async fn connect() -> zbus::Result<Self> {
        Ok(Self {
            connection: zbus::Connection::system().await?,
        })
    }

    /// Check read access for the caller owning `sender` (a unique bus name).
    /// Interaction flags zero: polling must never trigger a prompt.
    pub async fn check_read(&self, sender: &str) -> bool {
        self.check(sender, ACTION_READ, 0).await
    }

    /// Check write access; may prompt through polkit's authentication agent.
    /// (Wired into the Phase 4 write flow.)
    #[allow(dead_code)]
    async fn check_write(&self, sender: &str) -> bool {
        self.check(sender, ACTION_SET_CHARGE_LIMIT, FLAG_ALLOW_USER_INTERACTION)
            .await
    }

    /// Is the given unique bus name still present on the bus?
    pub async fn caller_present(&self, sender: &str) -> bool {
        let Ok(dbus) = zbus::fdo::DBusProxy::new(&self.connection).await else {
            return false;
        };
        let Ok(name) = zbus::names::BusName::try_from(sender) else {
            return false;
        };
        dbus.name_has_owner(name).await.unwrap_or(false)
    }

    async fn check(&self, sender: &str, action: &'static str, flags: u32) -> bool {
        let Ok(proxy) = AuthorityProxy::new(&self.connection).await else {
            return false; // polkit unreachable: fail closed
        };
        let mut subject_details = HashMap::new();
        subject_details.insert("name", Value::from(sender.to_string()));
        match proxy
            .check_authorization(
                ("system-bus-name", subject_details),
                action,
                HashMap::new(),
                flags,
                "",
            )
            .await
        {
            Ok((is_authorized, _is_challenge, _details)) => is_authorized,
            Err(_) => false, // any error is a denial, never a bypass
        }
    }
}

/// Outcome of the bounded write-authorization exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteAuthOutcome {
    Authorized,
    /// Denied by polkit, unavailable, or any error — never dispatched.
    Denied,
    /// The caller left the bus while the prompt was pending.
    Disappeared,
    /// The 120-second bound elapsed; the pending check was cancelled.
    TimedOut,
}

impl Polkit {
    /// The write authorization for one caller: prompts via polkit with
    /// `AllowUserInteraction`, cancels if the caller disappears, and is
    /// bounded by [`WRITE_AUTH_TIMEOUT`]. Any transport or polkit error
    /// denies; there is no bypass.
    pub async fn check_write_bounded(&self, sender: &str) -> WriteAuthOutcome {
        let cancellation_id = format!(
            "fwpanel-{}-{}",
            std::process::id(),
            CANCELLATION_COUNTER.fetch_add(1, Ordering::Relaxed)
        );

        // Watch for the caller's unique name disappearing from the bus.
        let watch = MatchRule::builder()
            .msg_type(MessageType::Signal)
            .sender("org.freedesktop.DBus")
            .and_then(|b| b.interface("org.freedesktop.DBus"))
            .and_then(|b| b.member("NameOwnerChanged"))
            .and_then(|b| b.add_arg(sender))
            .map(|b| b.build());
        let watch = match watch {
            Ok(rule) => MessageStream::for_match_rule(rule, &self.connection, Some(1)).await,
            Err(e) => Err(e),
        };
        let mut watch = match watch {
            Ok(stream) => stream,
            Err(_) => return WriteAuthOutcome::Denied, // cannot watch: fail closed
        };

        let mut subject_details = HashMap::new();
        subject_details.insert("name", Value::from(sender.to_string()));
        let proxy = match AuthorityProxy::new(&self.connection).await {
            Ok(proxy) => proxy,
            Err(_) => return WriteAuthOutcome::Denied,
        };
        let mut check = Box::pin(proxy.check_authorization(
            ("system-bus-name", subject_details),
            ACTION_SET_CHARGE_LIMIT,
            HashMap::new(),
            FLAG_ALLOW_USER_INTERACTION,
            &cancellation_id,
        ));

        let outcome = tokio::select! {
            result = &mut check => match result {
                Ok((is_authorized, _, _)) if is_authorized => WriteAuthOutcome::Authorized,
                Ok(_) => WriteAuthOutcome::Denied,
                // polkit signals cancellation as an error; still denied.
                Err(_) => WriteAuthOutcome::Denied,
            },
            changed = watch.next() => match changed {
                Some(Ok(message)) => {
                    let body: Result<(String, String, String), _> =
                        message.body().deserialize();
                    match body {
                        // name gone: old owner is the caller, new owner empty
                        Ok((_, old, new)) if old == sender && new.is_empty() => {
                            WriteAuthOutcome::Disappeared
                        }
                        _ => WriteAuthOutcome::Denied,
                    }
                }
                Some(Err(_)) | None => WriteAuthOutcome::Denied,
            },
            _ = tokio::time::sleep(WRITE_AUTH_TIMEOUT) => WriteAuthOutcome::TimedOut,
        };

        if outcome != WriteAuthOutcome::Authorized {
            // Best-effort cancellation of a still-pending agent prompt.
            let _ = proxy
                .cancel_check_authorization(cancellation_id.as_str())
                .await;
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The subject wire format the authority expects: kind
    /// "system-bus-name" with the caller's unique name under key "name".
    #[test]
    fn subject_wire_signature_is_sa_sv() {
        use zbus::zvariant::Type;
        type Subject = (&'static str, HashMap<&'static str, Value<'static>>);
        assert_eq!(Subject::SIGNATURE.to_string(), "(sa{sv})");
    }

    #[test]
    fn action_ids_match_policy_files() {
        assert_eq!(ACTION_READ, "io.github.singh_gur.fwpanel.read-status");
        assert_eq!(
            ACTION_SET_CHARGE_LIMIT,
            "io.github.singh_gur.fwpanel.set-charge-limit"
        );
    }
}
