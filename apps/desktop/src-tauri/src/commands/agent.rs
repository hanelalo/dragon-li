use crate::runtime::{ApiResponse, AppState};
use serde_json::json;

#[tauri::command]
pub async fn start_agent(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ApiResponse, String> {
    tracing::info!("start_agent called");
    let mut manager = match state.agent_manager.lock() {
        Ok(guard) => guard,
        Err(e) => {
            tracing::error!("failed to lock agent manager: {}", e);
            return Ok(ApiResponse::err("AGENT_LOCK_FAILED", "failed to lock agent manager"));
        }
    };

    match manager.start(&app) {
        Ok(pid) => {
            tracing::info!("agent started with pid {:?}", pid);
            Ok(ApiResponse::ok(json!({ "running": true, "pid": pid })))
        },
        Err(err) => {
            tracing::error!("agent failed to start: {}", err);
            Ok(ApiResponse::err("AGENT_START_FAILED", err.to_string()))
        },
    }
}

#[tauri::command]
pub fn stop_agent(state: tauri::State<'_, AppState>) -> ApiResponse {
    let mut manager = match state.agent_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return ApiResponse::err("AGENT_LOCK_FAILED", "failed to lock agent manager");
        }
    };

    match manager.stop() {
        Ok(_) => ApiResponse::ok(json!({ "running": false })),
        Err(err) => ApiResponse::err("AGENT_STOP_FAILED", err.to_string()),
    }
}

#[tauri::command]
pub fn agent_status(state: tauri::State<'_, AppState>) -> ApiResponse {
    let mut manager = match state.agent_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return ApiResponse::err("AGENT_LOCK_FAILED", "failed to lock agent manager");
        }
    };

    match manager.status() {
        Ok((running, pid)) => ApiResponse::ok(json!({ "running": running, "pid": pid })),
        Err(err) => ApiResponse::err("AGENT_STATUS_FAILED", err.to_string()),
    }
}

#[tauri::command]
pub fn agent_health_check(state: tauri::State<'_, AppState>) -> ApiResponse {
    let manager = match state.agent_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return ApiResponse::err("AGENT_LOCK_FAILED", "failed to lock agent manager");
        }
    };

    match manager.health_check() {
        Ok(healthy) => ApiResponse::ok(json!({ "healthy": healthy })),
        Err(err) => ApiResponse::err("AGENT_HEALTH_FAILED", err.to_string()),
    }
}