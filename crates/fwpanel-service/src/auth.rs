//! Polkit authorization for the fwpanel service.
//!
//! Every check uses the caller's *actual* unique bus name taken from the
//! incoming message header — never a client-supplied identity. All failures
//! (polkit unavailable, malformed result, transport error) fail closed: no
//! authorization, no hardware access.

use std::collections::HashMap;

use zbus::proxy;
use zbus::zvariant::Value;

pub const ACTION_READ: &str = "io.github.singh_gur.fwpanel.read-status";
pub const ACTION_SET_CHARGE_LIMIT: &str = "io.github.singh_gur.fwpanel.set-charge-limit";

/// CheckAuthorizationFlags: AllowUserInteraction.
const FLAG_ALLOW_USER_INTERACTION: u32 = 1;

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
