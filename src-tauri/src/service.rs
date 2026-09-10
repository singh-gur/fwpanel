//! Typed blocking client for the fwpanel system service.
//!
//! Calls run on the caller's thread (the Tauri command wraps them in
//! `spawn_blocking`), use a fresh system-bus connection per call with a
//! ten-second method deadline, and decode replies through
//! `fwpanel_protocol`, so the UI only ever sees typed values or clear error
//! strings.

use std::time::Duration;

use fwpanel_protocol::{PowerSnapshot, Reply, ServiceInfo};
use zbus::blocking::Connection;
use zbus::proxy;

/// Client read deadline: generous for the serialized service, short enough
/// that the UI never hangs on a dead one.
const METHOD_TIMEOUT: Duration = Duration::from_secs(10);

#[proxy(
    interface = "io.github.singh_gur.Fwpanel1",
    default_service = "io.github.singh_gur.Fwpanel1",
    default_path = "/io/github/singh_gur/Fwpanel1"
)]
trait Fwpanel1 {
    fn get_service_info(&self) -> zbus::Result<String>;
    fn get_power(&self) -> zbus::Result<String>;
}

/// Fetch the service description. Errors mention the absent service rather
/// than raw transport details.
pub fn get_service_info() -> Result<ServiceInfo, String> {
    let json = call("GetServiceInfo", |proxy| proxy.get_service_info())?;
    Reply::decode(&json).map_err(|e| e.to_string())
}

/// Fetch battery/AC status plus the current charge limit.
pub fn get_power() -> Result<PowerSnapshot, String> {
    let json = call("GetPower", |proxy| proxy.get_power())?;
    Reply::decode(&json).map_err(|e| e.to_string())
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
    invoke(&proxy).map_err(|e| {
        format!("fwpanel service is unavailable for {method} (is fwpanel-service installed?): {e}")
    })
}
