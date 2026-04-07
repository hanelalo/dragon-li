use crate::runtime::{ApiResponse, AppState};
use crate::utils::map_guardrail_error;
use serde_json::json;

#[tauri::command]
pub fn guardrail_validate_path(state: tauri::State<'_, AppState>, path: String) -> ApiResponse {
    match state.guardrails.validate_file_path(&path) {
        Ok(resolved) => ApiResponse::ok(json!({ "allowed": true, "resolved_path": resolved })),
        Err(err) => map_guardrail_error(err),
    }
}

#[tauri::command]
pub fn guardrail_validate_capability(
    state: tauri::State<'_, AppState>,
    capability: String,
) -> ApiResponse {
    match state.guardrails.validate_capability(&capability) {
        Ok(()) => ApiResponse::ok(json!({ "allowed": true })),
        Err(err) => map_guardrail_error(err),
    }
}

#[tauri::command]
pub fn guardrail_validate_network(
    state: tauri::State<'_, AppState>,
    target_url: String,
) -> ApiResponse {
    let manager = match state.config_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return ApiResponse::err("CONFIG_LOCK_FAILED", "failed to lock config manager");
        }
    };

    let cfg = match manager.current() {
        Some(c) => c,
        None => {
            return ApiResponse::err(
                "CONFIG_NOT_FOUND",
                "config is not loaded; call config_get or config_save first",
            );
        }
    };

    match state.guardrails.validate_network_url(&target_url, cfg) {
        Ok(host) => ApiResponse::ok(json!({ "allowed": true, "host": host })),
        Err(err) => map_guardrail_error(err),
    }
}