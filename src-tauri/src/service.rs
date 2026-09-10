//! Typed blocking client for the fwpanel system service.
//!
//! Calls run on the caller's thread (the Tauri command wraps them in
//! `spawn_blocking`), use a fresh system-bus connection per call with a
//! ten-second method deadline, and decode replies through
//! `fwpanel_protocol`, so the UI only ever sees typed values or clear error
//! strings.

use std::time::Duration;

use fwpanel_protocol::{
    ChargeLimits, ErrorCode, InputDeckSnapshot, PortsSnapshot, PowerSnapshot, Reply, ReplyError,
    ServiceInfo,
};
use zbus::blocking::Connection;
use zbus::proxy;

/// Stable error prefixes so the UI can classify failures without parsing
/// free-form transport messages. Keep in sync with `classify()` in the UI.
mod tag {
    pub const UNAVAILABLE: &str = "service-unavailable";
    pub const DENIED: &str = "service-denied";
    pub const INCOMPATIBLE: &str = "service-incompatible";
    pub const INVALID_REPLY: &str = "service-invalid-reply";
    pub const FAILED: &str = "service-failed";
}

fn tag_transport(detail: String) -> String {
    format!("{}: {detail}", tag::UNAVAILABLE)
}

fn tag_reply_error(error: ReplyError) -> String {
    let tag = match &error {
        ReplyError::Incompatible { .. } => tag::INCOMPATIBLE,
        ReplyError::Oversized | ReplyError::Malformed(_) => tag::INVALID_REPLY,
        ReplyError::Service { code, .. } if *code == ErrorCode::AccessDenied => tag::DENIED,
        ReplyError::Service { .. } => tag::FAILED,
    };
    format!("{tag}: {error}")
}

/// Client read deadline: generous for the serialized service, short enough
/// that the UI never hangs on a dead one.
const METHOD_TIMEOUT: Duration = Duration::from_secs(10);
/// Write deadline: accommodates the bounded (120s) administrator prompt.
const WRITE_TIMEOUT: Duration = Duration::from_secs(150);

#[proxy(
    interface = "io.github.singh_gur.Fwpanel1",
    default_service = "io.github.singh_gur.Fwpanel1",
    default_path = "/io/github/singh_gur/Fwpanel1"
)]
trait Fwpanel1 {
    fn get_service_info(&self) -> zbus::Result<String>;
    fn get_power(&self) -> zbus::Result<String>;
    fn get_ports(&self) -> zbus::Result<String>;
    fn get_input_deck(&self) -> zbus::Result<String>;
    fn set_charge_limit(&self, maximum: u32) -> zbus::Result<String>;
}

/// Fetch the service description. Errors mention the absent service rather
/// than raw transport details.
pub fn get_service_info() -> Result<ServiceInfo, String> {
    let json = call("GetServiceInfo", |proxy| proxy.get_service_info()).map_err(tag_transport)?;
    Reply::decode(&json).map_err(tag_reply_error)
}

/// Fetch battery/AC status plus the current charge limit.
pub fn get_power() -> Result<PowerSnapshot, String> {
    let json = call("GetPower", |proxy| proxy.get_power()).map_err(tag_transport)?;
    Reply::decode(&json).map_err(tag_reply_error)
}

/// Fetch the four USB-C PD port states.
pub fn get_ports() -> Result<PortsSnapshot, String> {
    let json = call("GetPorts", |proxy| proxy.get_ports()).map_err(tag_transport)?;
    Reply::decode(&json).map_err(tag_reply_error)
}

/// Fetch input-deck state and touchpad presence.
pub fn get_input_deck() -> Result<InputDeckSnapshot, String> {
    let json = call("GetInputDeck", |proxy| proxy.get_input_deck()).map_err(tag_transport)?;
    Reply::decode(&json).map_err(tag_reply_error)
}

/// Apply a new charge-limit maximum and return the verified readback.
/// Authentication may prompt; the deadline covers the bounded prompt flow.
pub fn set_charge_limit(maximum: u32) -> Result<ChargeLimits, String> {
    let connection = zbus::blocking::connection::Builder::system()
        .map_err(|e| tag_transport(format!("cannot reach the system D-Bus bus: {e}")))?
        .method_timeout(WRITE_TIMEOUT)
        .build()
        .map_err(|e| tag_transport(format!("cannot reach the system D-Bus bus: {e}")))?;
    let proxy = Fwpanel1ProxyBlocking::new(&connection)
        .map_err(|e| format!("{}: fwpanel service proxy failed: {e}", tag::UNAVAILABLE))?;
    let json = proxy
        .set_charge_limit(maximum)
        .map_err(|e| format!("{}: SetChargeLimit: {e}", tag::UNAVAILABLE))?;
    Reply::decode(&json).map_err(tag_reply_error)
}

fn connection() -> Result<Connection, String> {
    zbus::blocking::connection::Builder::system()
        .map_err(|e| format!("cannot reach the system D-Bus bus: {e}"))?
        .method_timeout(METHOD_TIMEOUT)
        .build()
        .map_err(|e| format!("cannot reach the system D-Bus bus: {e}"))
}

fn call(
    method: &str,
    invoke: impl FnOnce(&Fwpanel1ProxyBlocking) -> zbus::Result<String>,
) -> Result<String, String> {
    let connection = connection()?;
    let proxy = Fwpanel1ProxyBlocking::new(&connection)
        .map_err(|e| format!("fwpanel service proxy failed: {e}"))?;
    invoke(&proxy).map_err(|e| format!("{method}: {e}"))
}
