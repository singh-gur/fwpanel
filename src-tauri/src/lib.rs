// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod service;

use fwpanel_protocol::{PowerSnapshot, ServiceInfo};

#[tauri::command]
async fn get_service_info() -> Result<ServiceInfo, String> {
    // Blocking D-Bus call stays off the GUI thread and the async executor.
    tauri::async_runtime::spawn_blocking(service::get_service_info)
        .await
        .map_err(|e| format!("service call task failed: {e}"))?
}

#[tauri::command]
async fn get_power() -> Result<PowerSnapshot, String> {
    tauri::async_runtime::spawn_blocking(service::get_power)
        .await
        .map_err(|e| format!("service call task failed: {e}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_service_info, get_power])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
