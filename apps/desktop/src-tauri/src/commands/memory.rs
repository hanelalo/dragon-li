use crate::memory_pipeline::{ExtractCandidatesInput, ReviewCandidateInput};
use crate::runtime::{ApiResponse, AppState};
use crate::sqlite_store::NewRequestLog;
use crate::utils::{iso_now, map_memory_error, map_store_error, next_log_id};
use serde_json::json;

#[tauri::command]
pub fn memory_extract_candidates(
    state: tauri::State<'_, AppState>,
    input: ExtractCandidatesInput,
) -> ApiResponse {
    match state.memory_pipeline.extract_candidates(input) {
        Ok(candidates) => ApiResponse::ok(json!({ "candidates": candidates })),
        Err(err) => map_memory_error(err),
    }
}

#[tauri::command]
pub fn memory_count_pending(state: tauri::State<'_, AppState>) -> ApiResponse {
    match state.memory_pipeline.count_pending_candidates() {
        Ok(count) => ApiResponse::ok(json!({ "count": count })),
        Err(err) => map_memory_error(err),
    }
}

#[tauri::command]
pub fn memory_list_candidates(
    state: tauri::State<'_, AppState>,
    session_id: Option<String>,
    status: Option<String>,
) -> ApiResponse {
    match state
        .memory_pipeline
        .list_candidates(session_id.as_deref(), status.as_deref())
    {
        Ok(candidates) => ApiResponse::ok(json!({ "candidates": candidates })),
        Err(err) => map_memory_error(err),
    }
}

#[tauri::command]
pub fn memory_review_candidate(
    state: tauri::State<'_, AppState>,
    input: ReviewCandidateInput,
) -> ApiResponse {
    let candidate_id = input.candidate_id.clone();
    match state.memory_pipeline.review_candidate(input) {
        Ok(memory_doc) => {
            if let Err(err) = state.sqlite_store.init_schema() {
                return map_store_error(err);
            }
            if let Err(err) = state.sqlite_store.create_request_log(&NewRequestLog {
                id: next_log_id("memory_review", "ok"),
                request_id: format!("memory_review_{candidate_id}"),
                session_id: if memory_doc.session_id.is_empty() { None } else { Some(memory_doc.session_id.clone()) },
                provider: None,
                model: None,
                status: "ok".to_string(),
                latency_ms: None,
                tokens_in: None,
                tokens_out: None,
                error_code: None,
                created_at: iso_now(),
            }) {
                return map_store_error(err);
            }
            ApiResponse::ok(json!({ "memory": memory_doc }))
        }
        Err(err) => {
            let text = err.to_string();
            let code = text
                .split(':')
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("INTERNAL_ERROR")
                .to_string();
            if let Err(init_err) = state.sqlite_store.init_schema() {
                return map_store_error(init_err);
            }
            if let Err(write_err) = state.sqlite_store.create_request_log(&NewRequestLog {
                id: next_log_id("memory_review", "err"),
                request_id: format!("memory_review_{candidate_id}"),
                session_id: None,
                provider: None,
                model: None,
                status: "failed".to_string(),
                latency_ms: None,
                tokens_in: None,
                tokens_out: None,
                error_code: Some(code),
                created_at: iso_now(),
            }) {
                return map_store_error(write_err);
            }
            map_memory_error(err)
        }
    }
}

#[tauri::command]
pub fn memory_soft_delete(
    state: tauri::State<'_, AppState>,
    memory_id: String,
    deleted_at: Option<String>,
) -> ApiResponse {
    let deleted_at = deleted_at.unwrap_or_else(iso_now);
    match state
        .memory_pipeline
        .soft_delete_memory(&memory_id, &deleted_at)
    {
        Ok(result) => ApiResponse::ok(json!({ "result": result })),
        Err(err) => map_memory_error(err),
    }
}

#[tauri::command]
pub fn memory_restore(state: tauri::State<'_, AppState>, memory_id: String) -> ApiResponse {
    match state.memory_pipeline.restore_memory(&memory_id) {
        Ok(result) => ApiResponse::ok(json!({ "result": result })),
        Err(err) => map_memory_error(err),
    }
}

#[tauri::command]
pub fn memory_read(state: tauri::State<'_, AppState>, memory_id: String) -> ApiResponse {
    match state.memory_pipeline.read_memory_doc(&memory_id) {
        Ok(memory_doc) => ApiResponse::ok(json!({ "memory": memory_doc })),
        Err(err) => map_memory_error(err),
    }
}

#[tauri::command]
pub fn memory_search(
    state: tauri::State<'_, AppState>,
    query: String,
    min_confidence: Option<f64>,
    limit: Option<usize>,
) -> ApiResponse {
    let min_confidence = min_confidence.unwrap_or(0.6);
    let limit = limit.unwrap_or(20);
    match state
        .memory_pipeline
        .query_index(&query, min_confidence, limit)
    {
        Ok(hits) => ApiResponse::ok(json!({ "hits": hits })),
        Err(err) => map_memory_error(err),
    }
}

#[tauri::command]
pub fn memory_list_long_term(
    state: tauri::State<'_, AppState>,
    candidate_type: Option<String>,
    status: Option<String>,
    min_confidence: Option<f64>,
    tags_csv: Option<String>,
    limit: Option<usize>,
) -> ApiResponse {
    let tags = tags_csv
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    match state.memory_pipeline.list_long_term(
        candidate_type.as_deref(),
        status.as_deref(),
        min_confidence.unwrap_or(0.6),
        &tags,
        limit.unwrap_or(50),
    ) {
        Ok(items) => ApiResponse::ok(json!({ "items": items })),
        Err(err) => map_memory_error(err),
    }
}