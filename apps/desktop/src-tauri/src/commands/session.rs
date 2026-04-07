use crate::runtime::{ApiResponse, AppState};
use crate::sqlite_store::{NewMessage, NewRequestLog, NewSession};
use crate::utils::{iso_now, map_store_error};
use serde_json::json;
use tracing::{info, error};

#[tauri::command]
pub fn db_init(state: tauri::State<'_, AppState>) -> ApiResponse {
    info!("Initializing database schema...");
    match state.sqlite_store.init_schema() {
        Ok(()) => {
            info!("Database schema initialized successfully.");
            ApiResponse::ok(json!({ "initialized": true }))
        },
        Err(err) => {
            error!("Failed to initialize database schema: {}", err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn session_create(state: tauri::State<'_, AppState>, session: NewSession) -> ApiResponse {
    info!("Creating new session with id: {}", session.id);
    match state.sqlite_store.create_session(&session) {
        Ok(()) => {
            info!("Session created successfully: {}", session.id);
            ApiResponse::ok(json!({ "created": true, "session_id": session.id }))
        },
        Err(err) => {
            error!("Failed to create session: {}", err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn session_list(state: tauri::State<'_, AppState>) -> ApiResponse {
    info!("Listing sessions...");
    match state.sqlite_store.list_sessions() {
        Ok(items) => {
            info!("Found {} sessions.", items.len());
            ApiResponse::ok(json!({ "sessions": items }))
        },
        Err(err) => {
            error!("Failed to list sessions: {}", err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn session_update_title(
    state: tauri::State<'_, AppState>,
    session_id: String,
    title: String,
    updated_at: Option<String>,
) -> ApiResponse {
    let updated_at = updated_at.unwrap_or_else(iso_now);
    info!("Updating session title for id: {} to '{}'", session_id, title);
    match state
        .sqlite_store
        .update_session_title(&session_id, &title, &updated_at)
    {
        Ok(_) => {
            info!("Session title updated successfully: {}", session_id);
            ApiResponse::ok(json!({ "updated": true, "session_id": session_id }))
        },
        Err(err) => {
            error!("Failed to update session title for id {}: {}", session_id, err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn message_create(state: tauri::State<'_, AppState>, message: NewMessage) -> ApiResponse {
    info!("Creating message in session: {} with id: {}", message.session_id, message.id);
    match state.sqlite_store.create_message(&message) {
        Ok(()) => {
            info!("Message created successfully: {}", message.id);
            ApiResponse::ok(json!({ "created": true, "message_id": message.id }))
        },
        Err(err) => {
            error!("Failed to create message {}: {}", message.id, err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn message_list(state: tauri::State<'_, AppState>, session_id: String) -> ApiResponse {
    info!("Listing messages for session: {}", session_id);
    match state.sqlite_store.list_messages(&session_id) {
        Ok(items) => {
            info!("Found {} messages in session: {}", items.len(), session_id);
            ApiResponse::ok(json!({ "messages": items }))
        },
        Err(err) => {
            error!("Failed to list messages for session {}: {}", session_id, err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn request_log_create(state: tauri::State<'_, AppState>, log: NewRequestLog) -> ApiResponse {
    match state.sqlite_store.create_request_log(&log) {
        Ok(()) => ApiResponse::ok(json!({ "created": true, "request_log_id": log.id })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
pub fn request_log_list_by_request_id(
    state: tauri::State<'_, AppState>,
    request_id: String,
) -> ApiResponse {
    match state.sqlite_store.list_request_logs_by_request_id(&request_id) {
        Ok(items) => ApiResponse::ok(json!({ "request_logs": items })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
pub fn session_soft_delete(
    state: tauri::State<'_, AppState>,
    session_id: String,
    deleted_at: Option<String>,
) -> ApiResponse {
    let deleted_at = deleted_at.unwrap_or_else(iso_now);
    info!("Soft deleting session: {}", session_id);
    match state.sqlite_store.soft_delete_session(&session_id, &deleted_at) {
        Ok(result) => {
            info!("Successfully soft deleted session: {} (affected: {:?})", session_id, result);
            ApiResponse::ok(json!({
                "deleted": true,
                "session_id": session_id,
                "affected": result
            }))
        },
        Err(err) => {
            error!("Failed to soft delete session {}: {}", session_id, err);
            map_store_error(err)
        }
    }
}

#[tauri::command]
pub fn session_restore(state: tauri::State<'_, AppState>, session_id: String) -> ApiResponse {
    info!("Restoring soft deleted session: {}", session_id);
    match state.sqlite_store.restore_session(&session_id) {
        Ok(result) => {
            info!("Successfully restored session: {} (affected: {:?})", session_id, result);
            ApiResponse::ok(json!({
                "restored": true,
                "session_id": session_id,
                "affected": result
            }))
        },
        Err(err) => {
            error!("Failed to restore session {}: {}", session_id, err);
            map_store_error(err)
        }
    }
}