// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod service;

use fwpanel_protocol::{
    ChargeLimits, InputDeckSnapshot, PortsSnapshot, PowerSnapshot, ServiceInfo,
};

/// Temporary Phase 7 diagnostic: surface every failed service call on stderr
/// so sandboxed-run failures are visible outside the webview.
macro_rules! log_err {
    ($name:literal, $e:expr) => {{
        let e = $e;
        if let Err(ref m) = e {
            eprintln!("fwpanel: {} failed: {}", $name, m);
        }
        e
    }};
}

#[tauri::command]
async fn get_service_info() -> Result<ServiceInfo, String> {
    // Blocking D-Bus call stays off the GUI thread and the async executor.
    let r = tauri::async_runtime::spawn_blocking(service::get_service_info)
        .await
        .map_err(|e| format!("service call task failed: {e}"))
        .and_then(|r| r);
    log_err!("GetServiceInfo", r)
}

#[tauri::command]
async fn get_power() -> Result<PowerSnapshot, String> {
    let r = tauri::async_runtime::spawn_blocking(service::get_power)
        .await
        .map_err(|e| format!("service call task failed: {e}"))
        .and_then(|r| r);
    log_err!("GetPower", r)
}

#[tauri::command]
async fn get_ports() -> Result<PortsSnapshot, String> {
    let r = tauri::async_runtime::spawn_blocking(service::get_ports)
        .await
        .map_err(|e| format!("service call task failed: {e}"))
        .and_then(|r| r);
    log_err!("GetPorts", r)
}

#[tauri::command]
async fn get_input_deck() -> Result<InputDeckSnapshot, String> {
    let r = tauri::async_runtime::spawn_blocking(service::get_input_deck)
        .await
        .map_err(|e| format!("service call task failed: {e}"))
        .and_then(|r| r);
    log_err!("GetInputDeck", r)
}

#[tauri::command]
async fn set_charge_limit(maximum: u32) -> Result<ChargeLimits, String> {
    let r = tauri::async_runtime::spawn_blocking(move || service::set_charge_limit(maximum))
        .await
        .map_err(|e| format!("service call task failed: {e}"))
        .and_then(|r| r);
    log_err!("SetChargeLimit", r)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_service_info,
            get_power,
            get_ports,
            get_input_deck,
            set_charge_limit
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
