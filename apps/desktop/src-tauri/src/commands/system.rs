use crate::runtime::{runtime_bootstrap_payload, ApiResponse, AppState};
use serde_json::json;

#[tauri::command]
pub fn ping() -> ApiResponse {
    ApiResponse::ok(json!({ "event": "pong" }))
}

#[tauri::command]
pub fn runtime_info(state: tauri::State<'_, AppState>) -> ApiResponse {
    ApiResponse::ok(runtime_bootstrap_payload(&state))
}