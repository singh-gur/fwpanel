//! fwpanel privileged host service.
//!
//! Owns `io.github.singh_gur.Fwpanel1` on the system bus and serves the
//! approved status/charge-limit methods. Every method authorizes the real
//! D-Bus sender through polkit before doing anything else; application-level
//! outcomes (including denials and unimplemented features) are returned in the
//! JSON reply envelope, so clients get typed errors instead of transport
//! errors. Phase 1 serves `GetServiceInfo` only; the hardware-backed methods
//! return `unsupported_feature` before touching hardware or write
//! authentication.

mod auth;

use fwpanel_protocol::{ErrorCode, Reply, ServiceInfo, PROTOCOL_MAJOR};
use zbus::message::Header;

const BUS_NAME: &str = "io.github.singh_gur.Fwpanel1";
const OBJECT_PATH: &str = "/io/github/singh_gur/Fwpanel1";

/// `framework_lib` release this service is pinned against. Phase 2 adds the
/// actual dependency; until then this documents the pinned target.
const LIBRARY_VERSION: &str = "0.6.5";

struct Fwpanel1 {
    polkit: auth::Polkit,
}

#[zbus::interface(name = "io.github.singh_gur.Fwpanel1")]
impl Fwpanel1 {
    async fn get_service_info(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<String> {
        if let Err(reply) = self.require_read_access(&header).await {
            return Ok(reply);
        }
        ok_json(&service_info())
    }

    async fn get_power(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<String> {
        self.unimplemented(&header).await
    }

    async fn get_ports(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<String> {
        self.unimplemented(&header).await
    }

    async fn get_input_deck(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<String> {
        self.unimplemented(&header).await
    }

    async fn set_charge_limit(
        &self,
        #[zbus(header)] header: Header<'_>,
        _maximum: u32,
    ) -> zbus::fdo::Result<String> {
        self.unimplemented(&header).await
    }
}

impl Fwpanel1 {
    /// Authorize a read with the message's real sender; interaction flags
    /// zero so polling can never prompt. On failure, returns the error reply
    /// JSON to send instead of `()`.
    async fn require_read_access(&self, header: &Header<'_>) -> Result<(), String> {
        let sender = header.sender().map(|name| name.to_string());
        let Some(sender) = sender else {
            return Err(error_json(
                ErrorCode::AccessDenied,
                "requests without a sender are not authorized",
            ));
        };
        if self.polkit.check_read(&sender).await {
            Ok(())
        } else {
            Err(error_json(
                ErrorCode::AccessDenied,
                "this session is not allowed to read fwpanel status",
            ))
        }
    }

    /// Phase 1 behavior for every hardware-backed method: authorize the read,
    /// then report the feature as unimplemented before any hardware access or
    /// write authentication.
    async fn unimplemented(&self, header: &Header<'_>) -> zbus::fdo::Result<String> {
        if let Err(reply) = self.require_read_access(header).await {
            return Ok(reply);
        }
        Ok(error_json(
            ErrorCode::UnsupportedFeature,
            "this fwpanel-service build does not implement this feature yet",
        ))
    }
}

fn service_info() -> ServiceInfo {
    ServiceInfo {
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: PROTOCOL_MAJOR,
        library_version: LIBRARY_VERSION.to_string(),
        // No hardware feature is advertised until its implementation lands
        // and is verified (battery/charge-limit reads in Phase 2).
        features: Vec::new(),
    }
}

fn ok_json<T: serde::Serialize>(data: &T) -> zbus::fdo::Result<String> {
    Reply::ok_json(data).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
}

fn error_json(code: ErrorCode, message: &str) -> String {
    // Only fails if the error envelope itself cannot serialize, which would
    // be a programming error in fwpanel-protocol.
    Reply::<()>::error_json(code, message).expect("static error envelope serializes")
}

#[tokio::main]
async fn main() -> zbus::Result<()> {
    let polkit = auth::Polkit::connect().await?;
    let service = Fwpanel1 { polkit };
    let _connection = zbus::connection::Builder::system()?
        .name(BUS_NAME)?
        .serve_at(OBJECT_PATH, service)?
        .build()
        .await?;
    // D-Bus/systemd-activated: hold the name and serve until stopped.
    // `pending` never resolves; the type parameter satisfies main's Result.
    std::future::pending::<zbus::Result<()>>().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_info_advertises_no_hardware_features_in_phase_1() {
        let info = service_info();
        assert_eq!(info.protocol_version, PROTOCOL_MAJOR);
        assert!(info.features.is_empty());
        assert_eq!(info.library_version, LIBRARY_VERSION);
    }

    #[test]
    fn names_are_the_approved_contract() {
        assert_eq!(BUS_NAME, "io.github.singh_gur.Fwpanel1");
        assert_eq!(OBJECT_PATH, "/io/github/singh_gur/Fwpanel1");
        assert_eq!(auth::ACTION_READ, "io.github.singh_gur.fwpanel.read-status");
        assert_eq!(
            auth::ACTION_SET_CHARGE_LIMIT,
            "io.github.singh_gur.fwpanel.set-charge-limit"
        );
    }

    #[test]
    fn unimplemented_reply_envelope_decodes_to_unsupported_feature() {
        let json = error_json(ErrorCode::UnsupportedFeature, "not yet");
        let err = Reply::<ServiceInfo>::decode(&json).unwrap_err();
        assert!(matches!(
            err,
            fwpanel_protocol::ReplyError::Service {
                code: ErrorCode::UnsupportedFeature,
                ..
            }
        ));
    }
}
