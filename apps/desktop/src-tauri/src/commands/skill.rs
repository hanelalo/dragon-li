use crate::runtime::{ApiResponse, AppState};
use crate::utils::{iso_now, map_store_error};
use crate::chat_provider::ChatService;
use serde_json::json;
use tracing::{error, info};

fn trigger_skill_rescan(state: &tauri::State<'_, AppState>) {
    if let Ok(guard) = state.agent_manager.lock() {
        let uds_path = guard.get_uds_path();
        tauri::async_runtime::spawn(async move {
            let chat_service = ChatService::new(uds_path);
            if let Err(e) = chat_service.post_uds_json::<_, serde_json::Value>("/v1/skill/rescan", &json!({})).await {
                error!("Failed to trigger skill rescan: {}", e);
            } else {
                info!("Skill rescan triggered successfully.");
            }
        });
    } else {
        error!("Failed to lock agent manager to trigger skill rescan.");
    }
}

#[tauri::command]
pub fn skill_list(state: tauri::State<'_, AppState>) -> ApiResponse {
    info!("Listing skills...");
    match state.sqlite_store.list_skills() {
        Ok(items) => {
            info!("Found {} skills.", items.len());
            ApiResponse::ok(json!({ "skills": items }))
        }
        Err(err) => {
            error!("Failed to list skills: {}", err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn skill_toggle(
    state: tauri::State<'_, AppState>,
    id: String,
    enabled: bool,
) -> ApiResponse {
    let updated_at = iso_now();
    info!("Toggling skill for id: {}, enabled: {}", id, enabled);
    match state.sqlite_store.update_skill_enabled(&id, enabled, &updated_at) {
        Ok(_) => {
            info!("Skill toggled successfully: {}", id);
            trigger_skill_rescan(&state);
            ApiResponse::ok(json!({ "toggled": true, "skill_id": id, "enabled": enabled }))
        }
        Err(err) => {
            error!("Failed to toggle skill for id {}: {}", id, err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn skill_rescan(state: tauri::State<'_, AppState>) -> ApiResponse {
    info!("Manual skill rescan triggered");
    trigger_skill_rescan(&state);
    ApiResponse::ok(json!({ "rescan_triggered": true }))
}