#!/bin/bash
set -e

mkdir -p src-tauri/src/commands

# utils.rs
cat << 'INNER_EOF' > src-tauri/src/utils.rs
use crate::runtime::ApiResponse;
use crate::sqlite_store::StoreError;
use crate::memory_pipeline::MemoryError;
use crate::config_guardrails::ConfigError;

pub fn iso_now() -> String {
    chrono::Local::now().to_rfc3339()
}

pub fn next_log_id(request_id: &str, suffix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("log_{request_id}_{suffix}_{nanos}")
}

pub fn error_code_from_text(text: &str, fallback: &str) -> String {
    text.split(':')
        .next()
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

pub fn map_store_error(err: StoreError) -> ApiResponse {
    let text = err.to_string();
    let code = error_code_from_text(&text, "INTERNAL_ERROR");
    ApiResponse::err(&code, text)
}

pub fn map_memory_error(err: MemoryError) -> ApiResponse {
    let text = err.to_string();
    let code = error_code_from_text(&text, "INTERNAL_ERROR");
    ApiResponse::err(&code, text)
}

pub fn config_error_parts(err: ConfigError) -> (String, String) {
    let message = err.to_string();
    let code = error_code_from_text(&message, "INTERNAL_ERROR");
    (code, message)
}

pub fn map_config_error(err: ConfigError) -> ApiResponse {
    let (code, message) = config_error_parts(err);
    ApiResponse::err(&code, message)
}

pub fn map_guardrail_error(err: crate::config_guardrails::GuardrailError) -> ApiResponse {
    let text = err.to_string();
    let code = error_code_from_text(&text, "INVALID_REQUEST");
    ApiResponse::err(&code, text)
}
INNER_EOF

# commands/mod.rs
cat << 'INNER_EOF' > src-tauri/src/commands/mod.rs
pub mod agent;
pub mod chat;
pub mod config;
pub mod guardrail;
pub mod memory;
pub mod session;
pub mod system;
INNER_EOF

# commands/agent.rs
cat << 'INNER_EOF' > src-tauri/src/commands/agent.rs
use crate::runtime::{ApiResponse, AppState};
use serde_json::json;

#[tauri::command]
pub async fn start_agent(state: tauri::State<'_, AppState>) -> Result<ApiResponse, String> {
    let mut manager = match state.agent_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return Ok(ApiResponse::err("AGENT_LOCK_FAILED", "failed to lock agent manager"));
        }
    };

    match manager.start() {
        Ok(pid) => Ok(ApiResponse::ok(json!({ "running": true, "pid": pid }))),
        Err(err) => Ok(ApiResponse::err("AGENT_START_FAILED", err.to_string())),
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
INNER_EOF

# commands/config.rs
cat << 'INNER_EOF' > src-tauri/src/commands/config.rs
use crate::config_guardrails::{config_to_json, ApiProfilesConfig};
use crate::runtime::{ApiResponse, AppState};
use crate::utils::map_config_error;
use serde_json::json;

#[tauri::command]
pub fn config_get(state: tauri::State<'_, AppState>) -> ApiResponse {
    let mut manager = match state.config_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return ApiResponse::err("CONFIG_LOCK_FAILED", "failed to lock config manager");
        }
    };

    match manager.load_or_reload() {
        Ok(cfg) => ApiResponse::ok(json!({ "config": config_to_json(cfg) })),
        Err(err) => map_config_error(err),
    }
}

#[tauri::command]
pub fn config_save(state: tauri::State<'_, AppState>, config: ApiProfilesConfig) -> ApiResponse {
    let mut manager = match state.config_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return ApiResponse::err("CONFIG_LOCK_FAILED", "failed to lock config manager");
        }
    };

    match manager.save_and_apply(config) {
        Ok(()) => {
            let current = manager.current().map(config_to_json);
            ApiResponse::ok(json!({ "saved": true, "config": current }))
        }
        Err(err) => map_config_error(err),
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
INNER_EOF

# commands/guardrail.rs
cat << 'INNER_EOF' > src-tauri/src/commands/guardrail.rs
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
INNER_EOF

# commands/session.rs
cat << 'INNER_EOF' > src-tauri/src/commands/session.rs
use crate::runtime::{ApiResponse, AppState};
use crate::sqlite_store::{NewMessage, NewRequestLog, NewSession};
use crate::utils::{iso_now, map_store_error};
use serde_json::json;

#[tauri::command]
pub fn db_init(state: tauri::State<'_, AppState>) -> ApiResponse {
    match state.sqlite_store.init_schema() {
        Ok(()) => ApiResponse::ok(json!({ "initialized": true })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
pub fn session_create(state: tauri::State<'_, AppState>, session: NewSession) -> ApiResponse {
    match state.sqlite_store.create_session(&session) {
        Ok(()) => ApiResponse::ok(json!({ "created": true, "session_id": session.id })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
pub fn session_list(state: tauri::State<'_, AppState>) -> ApiResponse {
    match state.sqlite_store.list_sessions() {
        Ok(items) => ApiResponse::ok(json!({ "sessions": items })),
        Err(err) => map_store_error(err),
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
    match state
        .sqlite_store
        .update_session_title(&session_id, &title, &updated_at)
    {
        Ok(_) => ApiResponse::ok(json!({ "updated": true, "session_id": session_id })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
pub fn message_create(state: tauri::State<'_, AppState>, message: NewMessage) -> ApiResponse {
    match state.sqlite_store.create_message(&message) {
        Ok(()) => ApiResponse::ok(json!({ "created": true, "message_id": message.id })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
pub fn message_list(state: tauri::State<'_, AppState>, session_id: String) -> ApiResponse {
    match state.sqlite_store.list_messages(&session_id) {
        Ok(items) => ApiResponse::ok(json!({ "messages": items })),
        Err(err) => map_store_error(err),
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
    match state.sqlite_store.soft_delete_session(&session_id, &deleted_at) {
        Ok(result) => ApiResponse::ok(json!({
            "deleted": true,
            "session_id": session_id,
            "affected": result
        })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
pub fn session_restore(state: tauri::State<'_, AppState>, session_id: String) -> ApiResponse {
    match state.sqlite_store.restore_session(&session_id) {
        Ok(result) => ApiResponse::ok(json!({
            "restored": true,
            "session_id": session_id,
            "affected": result
        })),
        Err(err) => map_store_error(err),
    }
}
INNER_EOF

# commands/memory.rs
cat << 'INNER_EOF' > src-tauri/src/commands/memory.rs
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
INNER_EOF

# commands/chat.rs
cat << 'INNER_EOF' > src-tauri/src/commands/chat.rs
use crate::chat_provider::{error_to_parts, ChatRequestInput, ChatService, ChatStreamEvent};
use crate::config_guardrails::{ApiProfilesConfig, Provider};
use crate::memory_pipeline::AutoExtractionResult;
use crate::runtime::{ApiResponse, AppState};
use crate::sqlite_store::{NewRequestLog, SqliteStore, StoreError};
use crate::utils::{config_error_parts, error_code_from_text, iso_now, map_store_error, next_log_id};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;
use tauri::Emitter;
use tracing::{error, info};

const MEMORY_INJECTION_TOP_N: usize = 3;

#[derive(Debug, Clone, Serialize)]
pub struct MemoryInjectionItem {
    pub memory_id: String,
    pub summary: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryInjectionReport {
    pub limit: usize,
    pub items: Vec<MemoryInjectionItem>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

fn provider_name(provider: &Provider) -> String {
    match provider {
        Provider::Openai => "openai".to_string(),
        Provider::Anthropic => "anthropic".to_string(),
    }
}

fn resolve_chat_log_context(
    cfg: &ApiProfilesConfig,
    request: &ChatRequestInput,
) -> (Option<String>, Option<String>) {
    let profile = cfg.profiles.iter().find(|profile| profile.id == request.profile_id);
    let provider = profile.map(|profile| provider_name(&profile.provider));
    let model = request
        .model
        .clone()
        .or_else(|| profile.map(|profile| profile.default_model.clone()));
    (provider, model)
}

#[derive(Debug, Clone, Serialize)]
pub struct TitleGenerateRequest<'a> {
    pub profile_id: String,
    pub model: Option<String>,
    pub user_text: String,
    pub cfg: &'a ApiProfilesConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TitleGenerateResponse {
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryExtractRequest<'a> {
    pub profile_id: String,
    pub model: Option<String>,
    pub session_id: String,
    pub user_text: String,
    pub assistant_text: String,
    pub history: Vec<crate::chat_provider::ChatMessageContext>,
    pub cfg: &'a ApiProfilesConfig,
}

pub fn chat_error_response(code: &str, message: String, request_id: &str) -> ApiResponse {
    ApiResponse::err(code, format!("{message} [request_id={request_id}]"))
}

pub fn create_chat_failure_log(
    store: &SqliteStore,
    request: &ChatRequestInput,
    provider: Option<String>,
    model: Option<String>,
    code: &str,
    latency_ms: i64,
) -> Result<(), StoreError> {
    store.init_schema()?;
    store.create_request_log(&NewRequestLog {
        id: next_log_id(&request.request_id, "err"),
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        provider,
        model,
        status: "failed".to_string(),
        latency_ms: Some(latency_ms),
        tokens_in: None,
        tokens_out: None,
        error_code: Some(code.to_string()),
        created_at: iso_now(),
    })
}

fn inject_memory_context(
    state: &AppState,
    request: &mut ChatRequestInput,
) -> MemoryInjectionReport {
    let mut report = MemoryInjectionReport {
        limit: MEMORY_INJECTION_TOP_N,
        items: Vec::new(),
        error_code: None,
        error_message: None,
    };

    if request.prompt.user.trim().is_empty() {
        return report;
    }

    let hits = match state
        .memory_pipeline
        .query_index(&request.prompt.user, 0.6, MEMORY_INJECTION_TOP_N)
    {
        Ok(hits) => hits,
        Err(err) => {
            let message = err.to_string();
            report.error_code = Some(error_code_from_text(&message, "INTERNAL_ERROR"));
            report.error_message = Some(message);
            return report;
        }
    };

    let mut first_read_error: Option<String> = None;
    for hit in hits.into_iter().take(MEMORY_INJECTION_TOP_N) {
        match state.memory_pipeline.read_memory_doc(&hit.memory_id) {
            Ok(doc) => {
                let summary = if doc.summary.trim().is_empty() {
                    doc.markdown.lines().next().unwrap_or_default().to_string()
                } else {
                    doc.summary
                };
                report.items.push(MemoryInjectionItem {
                    memory_id: hit.memory_id,
                    summary,
                    score: hit.score,
                });
            }
            Err(err) => {
                if first_read_error.is_none() {
                    first_read_error = Some(err.to_string());
                }
            }
        }
    }

    if report.items.is_empty() {
        if let Some(message) = first_read_error {
            report.error_code = Some(error_code_from_text(&message, "INTERNAL_ERROR"));
            report.error_message = Some(message);
        }
        return report;
    }

    let mut blocks = Vec::new();
    if !request.prompt.memory.trim().is_empty() {
        blocks.push(request.prompt.memory.trim().to_string());
    }
    let refs = report
        .items
        .iter()
        .map(|item| format!("- [{}] {}", item.memory_id, item.summary))
        .collect::<Vec<_>>()
        .join("\n");
    blocks.push(format!("Injected memory context (top {}):\n{}", MEMORY_INJECTION_TOP_N, refs));
    request.prompt.memory = blocks.join("\n\n");
    report
}

#[tauri::command]
pub async fn chat_summarize_title(
    state: tauri::State<'_, AppState>,
    profile_id: String,
    user_text: String,
) -> Result<ApiResponse, String> {
    let cfg = match state.config_manager.lock().unwrap().current().cloned() {
        Some(c) => c,
        None => return Ok(ApiResponse::err("CONFIG_NOT_FOUND", "Config not loaded")),
    };
    
    let profile = match cfg.profiles.iter().find(|p| p.id == profile_id) {
        Some(p) => p.clone(),
        None => return Ok(ApiResponse::err("PROFILE_NOT_FOUND", "Profile not found")),
    };
    
    let model = profile.default_model.clone();
    
    let uds_path = match state.agent_manager.lock() {
        Ok(guard) => guard.get_uds_path(),
        Err(_) => return Ok(ApiResponse::err("AGENT_LOCK_FAILED", "failed to lock agent manager")),
    };

    let res = tauri::async_runtime::spawn(async move {
        let request = TitleGenerateRequest {
            profile_id,
            model: Some(model),
            user_text,
            cfg: &cfg,
        };

        let service = ChatService::new(uds_path);
        service.post_uds_json::<_, TitleGenerateResponse>("/v1/chat/summarize_title", &request).await
    })
    .await
    .map_err(|e| e.to_string())?;

    match res {
        Ok(result) => {
            Ok(ApiResponse::ok(json!({ "title": result.title })))
        }
        Err(err) => {
            let (code, message, _) = error_to_parts(err);
            Ok(ApiResponse::err(&code, message))
        }
    }
}

#[tauri::command]
pub fn chat_send(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    request: ChatRequestInput,
) -> ApiResponse {
    info!("Received chat_send request: {:?}", request);
    let started = Instant::now();
    let cfg = {
        let mut config_manager = match state.config_manager.lock() {
            Ok(guard) => guard,
            Err(_) => {
                let code = "CONFIG_LOCK_FAILED";
                let message = "failed to lock config manager".to_string();
                if let Err(err) = create_chat_failure_log(
                    &state.sqlite_store,
                    &request,
                    None,
                    request.model.clone(),
                    code,
                    started.elapsed().as_millis() as i64,
                ) {
                    return map_store_error(err);
                }
                return chat_error_response(code, message, &request.request_id);
            }
        };

        if config_manager.current().is_none() {
            match config_manager.load_or_reload() {
                Ok(cfg) => cfg.clone(),
                Err(err) => {
                    let (code, message) = config_error_parts(err);
                    if let Err(write_err) = create_chat_failure_log(
                        &state.sqlite_store,
                        &request,
                        None,
                        request.model.clone(),
                        &code,
                        started.elapsed().as_millis() as i64,
                    ) {
                        return map_store_error(write_err);
                    }
                    return chat_error_response(&code, message, &request.request_id);
                }
            }
        } else {
            match config_manager.check_external_change() {
                Ok(true) => {
                    let code = "CONFIG_RELOAD_REJECTED";
                    let message = "external config change detected; confirm via config_apply_external_change(confirm=true)".to_string();
                    if let Err(write_err) = create_chat_failure_log(
                        &state.sqlite_store,
                        &request,
                        None,
                        request.model.clone(),
                        code,
                        started.elapsed().as_millis() as i64,
                    ) {
                        return map_store_error(write_err);
                    }
                    return chat_error_response(code, message, &request.request_id);
                }
                Ok(false) => match config_manager.current().cloned() {
                    Some(cfg) => cfg,
                    None => {
                        let code = "CONFIG_NOT_FOUND";
                        let message =
                            "config not loaded; call config_get/config_save first".to_string();
                        if let Err(write_err) = create_chat_failure_log(
                            &state.sqlite_store,
                            &request,
                            None,
                            request.model.clone(),
                            code,
                            started.elapsed().as_millis() as i64,
                        ) {
                            return map_store_error(write_err);
                        }
                        return chat_error_response(code, message, &request.request_id);
                    }
                },
                Err(err) => {
                    let (code, message) = config_error_parts(err);
                    if let Err(write_err) = create_chat_failure_log(
                        &state.sqlite_store,
                        &request,
                        None,
                        request.model.clone(),
                        &code,
                        started.elapsed().as_millis() as i64,
                    ) {
                        return map_store_error(write_err);
                    }
                    return chat_error_response(&code, message, &request.request_id);
                }
            }
        }
    };

    let (resolved_provider, resolved_model) = resolve_chat_log_context(&cfg, &request);
    let mut request = request;
    let memory_injection = inject_memory_context(&state, &mut request);
    if let Some(error_code) = memory_injection.error_code.clone() {
        let _ = state.sqlite_store.init_schema();
        let _ = state.sqlite_store.create_request_log(&NewRequestLog {
            id: next_log_id(&request.request_id, "memory_injection"),
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            provider: resolved_provider.clone(),
            model: resolved_model.clone(),
            status: "memory_injection_failed".to_string(),
            latency_ms: Some(started.elapsed().as_millis() as i64),
            tokens_in: None,
            tokens_out: None,
            error_code: Some(error_code),
            created_at: iso_now(),
        });
    }
    let request_id_for_emit = request.request_id.clone();
    let app_clone = app.clone();
    let resolved_provider_clone = resolved_provider.clone();
    let resolved_model_clone = resolved_model.clone();
    
    let sqlite_store = state.sqlite_store.clone();
    let memory_pipeline = state.memory_pipeline.clone();

    let uds_path = match state.agent_manager.lock() {
        Ok(guard) => guard.get_uds_path(),
        Err(_) => return chat_error_response("AGENT_LOCK_FAILED", "failed to lock agent manager".to_string(), &request.request_id),
    };

    tauri::async_runtime::spawn(async move {
        let service = ChatService::new(uds_path.clone());
        let app_for_emit = app_clone.clone();
        let request_id_for_event = request_id_for_emit.clone();
        let mut emit_stream = move |event: &ChatStreamEvent| {
            if let Err(e) = app_for_emit.emit(
                "chat_stream_event",
                json!({
                    "request_id": request_id_for_event,
                    "event": event
                }),
            ) {
                error!("Failed to emit chat_stream_event: {}", e);
            }
        };
        info!("Sending chat request to provider...");
        match service.chat_with_retry_stream(&request, &cfg, &mut emit_stream).await {
            Ok(result) => {
                info!("Chat request completed successfully. Request ID: {}", request.request_id);
                let latency_ms = started.elapsed().as_millis() as i64;
                
                if let Err(e) = app_clone.emit(
                    "chat_stream_event",
                    json!({
                        "request_id": request_id_for_emit,
                        "event": {
                            "type": "done",
                            "payload": {
                                "latency_ms": latency_ms,
                                "tokens_in": result.tokens_in,
                                "tokens_out": result.tokens_out,
                            }
                        }
                    }),
                ) {
                    error!("Failed to emit chat_stream_event done payload: {}", e);
                }

                let msg_id = format!("m_ast_{}", request.request_id);
                if let Err(e) = sqlite_store.update_message_completion(
                    &msg_id,
                    &result.output_text,
                    &result.reasoning_text,
                    "ok",
                    Some(result.tokens_in as i64),
                    Some(result.tokens_out as i64),
                    Some(latency_ms),
                    None,
                    None,
                ) {
                    error!("Failed to update message completion in DB: {}", e);
                }

                let _ = sqlite_store.init_schema();
                if let Err(err) = sqlite_store.create_request_log(&NewRequestLog {
                    id: next_log_id(&request.request_id, "ok"),
                    request_id: request.request_id.clone(),
                    session_id: request.session_id.clone(),
                    provider: Some(result.provider.clone()).or_else(|| resolved_provider_clone.clone()),
                    model: Some(result.model.clone()).or_else(|| resolved_model_clone.clone()),
                    status: "ok".to_string(),
                    latency_ms: Some(latency_ms),
                    tokens_in: Some(result.tokens_in as i64),
                    tokens_out: Some(result.tokens_out as i64),
                    error_code: None,
                    created_at: iso_now(),
                }) {
                    error!("Failed to log request success: {}", err);
                }

                let app_for_bg = app_clone.clone();
                let sqlite_for_bg = sqlite_store.clone();
                let memory_for_bg = memory_pipeline.clone();
                let session_id_for_bg = request.session_id.clone();
                let cfg_for_bg = cfg.clone();

                let user_text_for_bg = request.prompt.user.clone();
                let assistant_text_for_bg = result.output_text.clone();
                let history_for_bg = request.history.clone();

                tauri::async_runtime::spawn(async move {
                    if let Some(session_id) = session_id_for_bg {
                        match sqlite_for_bg.get_latest_user_message(&session_id) {
                            Ok(user_msg) => {
                                info!("Starting auto memory extraction for session: {}", session_id);
                                
                                let extract_req = MemoryExtractRequest {
                                    profile_id: request.profile_id.clone(),
                                    model: request.model.clone(),
                                    session_id: session_id.clone(),
                                    user_text: user_text_for_bg,
                                    assistant_text: assistant_text_for_bg,
                                    history: history_for_bg,
                                    cfg: &cfg_for_bg,
                                };

                                let extraction_service = ChatService::new(uds_path.clone());
                                match extraction_service.post_uds_json::<_, AutoExtractionResult>("/v1/memory/extract", &extract_req).await {
                                    Ok(res) => {
                                        if !res.memories.is_empty() {
                                            match memory_for_bg.save_extracted_candidates(&session_id, &user_msg.id, res) {
                                                Ok(count) if count > 0 => {
                                                    info!("Auto-extracted {} memories for session {}", count, session_id);
                                                    if let Ok(unreviewed_count) = memory_for_bg.count_pending_candidates() {
                                                        if let Err(e) = app_for_bg.emit("memory_candidates_updated", json!({
                                                            "unreviewed_count": unreviewed_count,
                                                            "new_memories": count
                                                        })) {
                                                            error!("Failed to emit memory_candidates_updated: {}", e);
                                                        }
                                                    }
                                                }
                                                Ok(_) => {
                                                    info!("Auto-extracted memories were skipped (duplicates) for session {}", session_id);
                                                }
                                                Err(e) => error!("Failed to save extracted memories: {}", e),
                                            }
                                        } else {
                                            info!("No new memories found by LLM for session {}", session_id);
                                        }
                                    }
                                    Err(e) => error!("Auto extraction request failed: {}", e),
                                }
                            }
                            Err(e) => error!("Failed to get latest user message for auto extraction: {}", e),
                        }
                    }
                });
            }
            Err(err) => {
                error!("Chat request failed: {}", err);
                let (code, message, retryable) = error_to_parts(err);
                emit_stream(&ChatStreamEvent::Aborted {
                    code: code.clone(),
                    message: message.clone(),
                    retryable,
                });
                
                let latency_ms = started.elapsed().as_millis() as i64;
                let msg_id = format!("m_ast_{}", request.request_id);
                if let Err(e) = sqlite_store.update_message_completion(
                    &msg_id,
                    "",
                    "",
                    "failed",
                    None,
                    None,
                    Some(latency_ms),
                    Some(&code),
                    Some(&message),
                ) {
                    error!("Failed to update message failure in DB: {}", e);
                }

                if let Err(write_err) = create_chat_failure_log(
                    &sqlite_store,
                    &request,
                    resolved_provider_clone,
                    resolved_model_clone,
                    &code,
                    latency_ms,
                ) {
                    error!("Failed to log request failure: {}", write_err);
                }
            }
        }
    });

    ApiResponse::ok(json!({
        "status": "started",
        "memory_injection": memory_injection
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_provider::ChatPromptLayer;
    use crate::chat_provider::ChatRequestInput;
    use crate::sqlite_store::SqliteStore;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(prefix: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("dragon-li-{prefix}-{stamp}.db"))
    }

    fn sample_request() -> ChatRequestInput {
        ChatRequestInput {
            profile_id: "openai-main".to_string(),
            request_id: "req_test_1".to_string(),
            session_id: Some("s1".to_string()),
            model: Some("gpt-4o-mini".to_string()),
            prompt: ChatPromptLayer {
                system: "system".to_string(),
                runtime: "runtime".to_string(),
                memory: "memory".to_string(),
                user: "hello".to_string(),
            },
            history: vec![],
        }
    }

    #[test]
    fn chat_error_response_contains_request_id() {
        let response = chat_error_response("INVALID_REQUEST", "bad input".to_string(), "req_abc");
        assert!(!response.ok);
        let message = response
            .error
            .as_ref()
            .map(|error| error.message.clone())
            .unwrap_or_default();
        assert!(message.contains("request_id=req_abc"));
    }

    #[test]
    fn create_chat_failure_log_writes_request_log() {
        let db_path = temp_db_path("chat-failure-log");
        let store = SqliteStore::new(db_path.clone());
        let request = sample_request();

        create_chat_failure_log(
            &store,
            &request,
            Some("openai".to_string()),
            Some("gpt-4o-mini".to_string()),
            "CONFIG_NOT_FOUND",
            12,
        )
        .expect("failure log should be written");

        let logs = store
            .list_request_logs_by_request_id(&request.request_id)
            .expect("request logs should be readable");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].status, "failed");
        assert_eq!(logs[0].error_code.as_deref(), Some("CONFIG_NOT_FOUND"));
        assert_eq!(logs[0].provider.as_deref(), Some("openai"));
        assert_eq!(logs[0].model.as_deref(), Some("gpt-4o-mini"));
        assert_eq!(logs[0].session_id.as_deref(), Some("s1"));

        fs::remove_file(db_path).ok();
    }
}
INNER_EOF

# commands/system.rs
cat << 'INNER_EOF' > src-tauri/src/commands/system.rs
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
INNER_EOF

# main.rs
cat << 'INNER_EOF' > src-tauri/src/main.rs
mod config_guardrails;
mod chat_provider;
mod memory_pipeline;
mod runtime;
mod sqlite_store;
mod utils;
mod commands;

use runtime::AppState;
use tauri::Manager;
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = match AppState::bootstrap() {
                Ok(state) => state,
                Err(err) => {
                    eprintln!("failed to bootstrap runtime: {err}");
                    std::process::exit(1);
                }
            };
            
            if let Some(home) = dirs::home_dir() {
                let logs_dir = home.join(".dragon-li").join("logs");
                let file_appender = RollingFileAppender::new(Rotation::DAILY, logs_dir, "dragon-li.log");
                let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
                
                Box::leak(Box::new(_guard));

                tracing_subscriber::registry()
                    .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
                    .with(fmt::layer().with_writer(non_blocking))
                    .with(fmt::layer().with_writer(std::io::stdout))
                    .init();
                    
                info!("Logger initialized in ~/.dragon-li/logs/");
            }

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::ping,
            commands::system::runtime_info,
            commands::agent::start_agent,
            commands::agent::stop_agent,
            commands::agent::agent_status,
            commands::agent::agent_health_check,
            commands::config::config_get,
            commands::config::config_save,
            commands::config::config_check_external_change,
            commands::config::config_apply_external_change,
            commands::guardrail::guardrail_validate_path,
            commands::guardrail::guardrail_validate_capability,
            commands::guardrail::guardrail_validate_network,
            commands::session::db_init,
            commands::session::session_create,
            commands::session::session_list,
            commands::session::session_update_title,
            commands::session::message_create,
            commands::session::message_list,
            commands::session::request_log_create,
            commands::session::request_log_list_by_request_id,
            commands::session::session_soft_delete,
            commands::session::session_restore,
            commands::chat::chat_summarize_title,
            commands::chat::chat_send,
            commands::memory::memory_extract_candidates,
            commands::memory::memory_list_candidates,
            commands::memory::memory_count_pending,
            commands::memory::memory_review_candidate,
            commands::memory::memory_soft_delete,
            commands::memory::memory_restore,
            commands::memory::memory_read,
            commands::memory::memory_search,
            commands::memory::memory_list_long_term
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
INNER_EOF

cd src-tauri
source ~/.cargo/env
cargo check
