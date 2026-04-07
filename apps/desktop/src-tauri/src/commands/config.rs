use crate::config_guardrails::{config_to_json, ApiProfilesConfig};
use crate::runtime::{ApiResponse, AppState};
use crate::utils::map_config_error;
use serde_json::json;
use tracing::{info, error};

#[tauri::command]
pub fn config_get(state: tauri::State<'_, AppState>) -> ApiResponse {
    info!("Fetching configuration...");
    let mut manager = match state.config_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            error!("Failed to lock config manager for config_get");
            return ApiResponse::err("CONFIG_LOCK_FAILED", "failed to lock config manager");
        }
    };

    match manager.load_or_reload() {
        Ok(cfg) => {
            info!("Configuration fetched successfully");
            ApiResponse::ok(json!({ "config": config_to_json(cfg) }))
        },
        Err(err) => {
            error!("Failed to load configuration: {}", err);
            map_config_error(err)
        }
    }
}

#[tauri::command]
pub fn config_save(state: tauri::State<'_, AppState>, config: ApiProfilesConfig) -> ApiResponse {
    info!("Saving new configuration...");
    let mut manager = match state.config_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            error!("Failed to lock config manager for config_save");
            return ApiResponse::err("CONFIG_LOCK_FAILED", "failed to lock config manager");
        }
    };

    match manager.save_and_apply(config) {
        Ok(()) => {
            info!("Configuration saved and applied successfully");
            let current = manager.current().map(config_to_json);
            ApiResponse::ok(json!({ "saved": true, "config": current }))
        }
        Err(err) => {
            error!("Failed to save configuration: {}", err);
            map_config_error(err)
        }
    }
}

#[tauri::command]
pub fn config_check_external_change(state: tauri::State<'_, AppState>) -> ApiResponse {
    let mut manager = match state.config_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return ApiResponse::err("CONFIG_LOCK_FAILED", "failed to lock config manager");
        }
    };

    match manager.check_external_change() {
        Ok(changed) => ApiResponse::ok(json!({ "external_changed": changed })),
        Err(err) => map_config_error(err),
    }
}

#[tauri::command]
pub fn config_apply_external_change(state: tauri::State<'_, AppState>, confirm: bool) -> ApiResponse {
    let mut manager = match state.config_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return ApiResponse::err("CONFIG_LOCK_FAILED", "failed to lock config manager");
        }
    };

    match manager.apply_external_change(confirm) {
        Ok(cfg) => ApiResponse::ok(json!({
            "reloaded": true,
            "config": config_to_json(cfg)
        })),
        Err(err) => map_config_error(err),
    }
}