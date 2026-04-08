use crate::runtime::{ApiResponse, AppState};
use crate::sqlite_store::NewMcpConnector;
use crate::utils::{iso_now, map_store_error};
use crate::chat_provider::ChatService;
use serde_json::json;
use tracing::{error, info};

fn trigger_mcp_reload(state: &tauri::State<'_, AppState>) {
    if let Ok(guard) = state.agent_manager.lock() {
        let uds_path = guard.get_uds_path();
        tauri::async_runtime::spawn(async move {
            let chat_service = ChatService::new(uds_path);
            if let Err(e) = chat_service.post_uds_json::<_, serde_json::Value>("/v1/mcp/reload", &json!({})).await {
                error!("Failed to trigger MCP reload: {}", e);
            } else {
                info!("MCP reload triggered successfully.");
            }
        });
    } else {
        error!("Failed to lock agent manager to trigger MCP reload.");
    }
}

#[tauri::command]
pub fn mcp_connector_create(state: tauri::State<'_, AppState>, connector: NewMcpConnector) -> ApiResponse {
    info!("Creating new MCP connector with id: {}", connector.id);
    match state.sqlite_store.create_mcp_connector(&connector) {
        Ok(()) => {
            info!("MCP connector created successfully: {}", connector.id);
            trigger_mcp_reload(&state);
            ApiResponse::ok(json!({ "created": true, "connector_id": connector.id }))
        }
        Err(err) => {
            error!("Failed to create MCP connector: {}", err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn mcp_connector_list(state: tauri::State<'_, AppState>) -> ApiResponse {
    info!("Listing MCP connectors...");
    match state.sqlite_store.list_mcp_connectors() {
        Ok(items) => {
            info!("Found {} MCP connectors.", items.len());
            ApiResponse::ok(json!({ "connectors": items }))
        }
        Err(err) => {
            error!("Failed to list MCP connectors: {}", err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn mcp_connector_update(
    state: tauri::State<'_, AppState>,
    id: String,
    name: String,
    mcp_type: String,
    status: String,
    config_content: String,
    updated_at: Option<String>,
) -> ApiResponse {
    let updated_at = updated_at.unwrap_or_else(iso_now);
    info!("Updating MCP connector for id: {}", id);
    match state.sqlite_store.update_mcp_connector(
        &id,
        &name,
        &mcp_type,
        &status,
        &config_content,
        &updated_at,
    ) {
        Ok(_) => {
            info!("MCP connector updated successfully: {}", id);
            trigger_mcp_reload(&state);
            ApiResponse::ok(json!({ "updated": true, "connector_id": id }))
        }
        Err(err) => {
            error!("Failed to update MCP connector for id {}: {}", id, err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn mcp_connector_delete(state: tauri::State<'_, AppState>, id: String) -> ApiResponse {
    let deleted_at = iso_now();
    info!("Deleting MCP connector for id: {}", id);
    match state.sqlite_store.delete_mcp_connector(&id, &deleted_at) {
        Ok(_) => {
            info!("MCP connector deleted successfully: {}", id);
            trigger_mcp_reload(&state);
            ApiResponse::ok(json!({ "deleted": true, "connector_id": id }))
        }
        Err(err) => {
            error!("Failed to delete MCP connector for id {}: {}", id, err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub async fn mcp_connector_test(
    state: tauri::State<'_, AppState>,
    mcp_type: String,
    config_content: String,
) -> Result<ApiResponse, ()> {
    let uds_path = match state.agent_manager.lock() {
        Ok(guard) => guard.get_uds_path(),
        Err(_) => return Ok(ApiResponse::err("AGENT_LOCK_FAILED", "failed to lock agent manager")),
    };

    let chat_service = ChatService::new(uds_path);
    let payload = json!({
        "mcp_type": mcp_type,
        "config_content": config_content
    });

    match chat_service.post_uds_json::<_, serde_json::Value>("/v1/mcp/test", &payload).await {
        Ok(res) => {
            if let Some(status) = res.get("status").and_then(|s| s.as_str()) {
                if status == "ok" {
                    return Ok(ApiResponse::ok(json!({ "success": true, "tools": res.get("tools").cloned().unwrap_or(json!([])) })));
                }
            }
            let err_msg = res.get("error").and_then(|e| e.as_str()).unwrap_or("Unknown error");
            Ok(ApiResponse::err("MCP_TEST_FAILED", err_msg))
        }
        Err(e) => Ok(ApiResponse::err("AGENT_REQUEST_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn mcp_get_status(state: tauri::State<'_, AppState>) -> Result<ApiResponse, ()> {
    let uds_path = match state.agent_manager.lock() {
        Ok(guard) => guard.get_uds_path(),
        Err(_) => return Ok(ApiResponse::err("AGENT_LOCK_FAILED", "failed to lock agent manager")),
    };

    let chat_service = ChatService::new(uds_path);
    match chat_service.get_uds_json::<serde_json::Value>("/v1/mcp/status").await {
        Ok(res) => Ok(ApiResponse::ok(res)),
        Err(e) => Ok(ApiResponse::err("AGENT_REQUEST_FAILED", e.to_string())),
    }
}