//! Typed blocking client for the fwpanel system service.
//!
//! Calls run on the caller's thread (the Tauri command wraps them in
//! `spawn_blocking`), use a fresh system-bus connection per call, and decode
//! replies through `fwpanel_protocol`, so the UI only ever sees typed values
//! or clear error strings.

use fwpanel_protocol::{Reply, ServiceInfo};
use zbus::blocking::Connection;
use zbus::proxy;

#[proxy(
    interface = "io.github.singh_gur.Fwpanel1",
    default_service = "io.github.singh_gur.Fwpanel1",
    default_path = "/io/github/singh_gur/Fwpanel1"
)]
trait Fwpanel1 {
    fn get_service_info(&self) -> zbus::Result<String>;
}

/// Fetch the service description. Errors mention the absent service rather
/// than raw transport details.
pub fn get_service_info() -> Result<ServiceInfo, String> {
    let json = call("GetServiceInfo", |proxy| proxy.get_service_info())?;
    Reply::decode(&json).map_err(|e| e.to_string())
}

fn call(
    method: &str,
    invoke: impl FnOnce(&Fwpanel1ProxyBlocking) -> zbus::Result<String>,
) -> Result<String, String> {
    let connection =
        Connection::system().map_err(|e| format!("cannot reach the system D-Bus bus: {e}"))?;
    let proxy = Fwpanel1ProxyBlocking::new(&connection)
        .map_err(|e| format!("fwpanel service proxy failed: {e}"))?;
    invoke(&proxy).map_err(|e| {
        format!("fwpanel service is unavailable for {method} (is fwpanel-service installed?): {e}")
    })
}
