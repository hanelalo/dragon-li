use crate::runtime::{ApiResponse, AppState};
use crate::sqlite_store::NewMcpConnector;
use crate::utils::{iso_now, map_store_error};
use serde_json::json;
use tracing::{error, info};

#[tauri::command]
pub fn mcp_connector_create(state: tauri::State<'_, AppState>, connector: NewMcpConnector) -> ApiResponse {
    info!("Creating new MCP connector with id: {}", connector.id);
    match state.sqlite_store.create_mcp_connector(&connector) {
        Ok(()) => {
            info!("MCP connector created successfully: {}", connector.id);
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
    endpoint: String,
    status: String,
    allowed_domains_json: Option<String>,
    enabled: bool,
    updated_at: Option<String>,
) -> ApiResponse {
    let updated_at = updated_at.unwrap_or_else(iso_now);
    info!("Updating MCP connector for id: {}", id);
    match state.sqlite_store.update_mcp_connector(
        &id,
        &name,
        &mcp_type,
        &endpoint,
        &status,
        allowed_domains_json.as_deref(),
        enabled,
        &updated_at,
    ) {
        Ok(_) => {
            info!("MCP connector updated successfully: {}", id);
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
            ApiResponse::ok(json!({ "deleted": true, "connector_id": id }))
        }
        Err(err) => {
            error!("Failed to delete MCP connector for id {}: {}", id, err);
            map_store_error(err)
        }
    }
}