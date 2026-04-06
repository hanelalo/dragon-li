mod config_guardrails;
mod chat_provider;
mod memory_pipeline;
mod runtime;
mod sqlite_store;

use chat_provider::{error_to_parts, ChatRequestInput, ChatService, ChatStreamEvent, HttpTransport};
use config_guardrails::{config_to_json, ApiProfilesConfig, ConfigError, Provider};
use memory_pipeline::{ExtractCandidatesInput, MemoryError, ReviewCandidateInput};
use runtime::{runtime_bootstrap_payload, ApiResponse, AppState};
use serde::Serialize;
use serde_json::json;
use sqlite_store::{NewMessage, NewRequestLog, NewSession, SqliteStore, StoreError};
use std::time::Instant;
use tauri::Emitter;

const MEMORY_INJECTION_TOP_N: usize = 3;

#[derive(Debug, Clone, Serialize)]
struct MemoryInjectionItem {
    memory_id: String,
    summary: String,
    score: f64,
}

#[derive(Debug, Clone, Serialize)]
struct MemoryInjectionReport {
    limit: usize,
    items: Vec<MemoryInjectionItem>,
    error_code: Option<String>,
    error_message: Option<String>,
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

fn error_code_from_text(text: &str, fallback: &str) -> String {
    text.split(':')
        .next()
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn config_error_parts(err: ConfigError) -> (String, String) {
    let message = err.to_string();
    let code = error_code_from_text(&message, "INTERNAL_ERROR");
    (code, message)
}

fn chat_error_response(code: &str, message: String, request_id: &str) -> ApiResponse {
    ApiResponse::err(code, format!("{message} [request_id={request_id}]"))
}

fn create_chat_failure_log(
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
fn ping() -> ApiResponse {
    ApiResponse::ok(json!({ "event": "pong" }))
}

#[tauri::command]
fn runtime_info(state: tauri::State<'_, AppState>) -> ApiResponse {
    ApiResponse::ok(runtime_bootstrap_payload(&state))
}

#[tauri::command]
fn start_agent(state: tauri::State<'_, AppState>) -> ApiResponse {
    let mut manager = match state.agent_manager.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return ApiResponse::err("AGENT_LOCK_FAILED", "failed to lock agent manager");
        }
    };

    match manager.start() {
        Ok(pid) => ApiResponse::ok(json!({ "running": true, "pid": pid })),
        Err(err) => ApiResponse::err("AGENT_START_FAILED", err.to_string()),
    }
}

#[tauri::command]
fn stop_agent(state: tauri::State<'_, AppState>) -> ApiResponse {
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
fn agent_status(state: tauri::State<'_, AppState>) -> ApiResponse {
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
fn agent_health_check(state: tauri::State<'_, AppState>) -> ApiResponse {
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

#[tauri::command]
fn config_get(state: tauri::State<'_, AppState>) -> ApiResponse {
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
fn config_save(state: tauri::State<'_, AppState>, config: ApiProfilesConfig) -> ApiResponse {
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
fn config_check_external_change(state: tauri::State<'_, AppState>) -> ApiResponse {
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
fn config_apply_external_change(state: tauri::State<'_, AppState>, confirm: bool) -> ApiResponse {
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

#[tauri::command]
fn guardrail_validate_path(state: tauri::State<'_, AppState>, path: String) -> ApiResponse {
    match state.guardrails.validate_file_path(&path) {
        Ok(resolved) => ApiResponse::ok(json!({ "allowed": true, "resolved_path": resolved })),
        Err(err) => map_guardrail_error(err),
    }
}

#[tauri::command]
fn guardrail_validate_capability(
    state: tauri::State<'_, AppState>,
    capability: String,
) -> ApiResponse {
    match state.guardrails.validate_capability(&capability) {
        Ok(()) => ApiResponse::ok(json!({ "allowed": true })),
        Err(err) => map_guardrail_error(err),
    }
}

#[tauri::command]
fn guardrail_validate_network(
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

#[tauri::command]
fn db_init(state: tauri::State<'_, AppState>) -> ApiResponse {
    match state.sqlite_store.init_schema() {
        Ok(()) => ApiResponse::ok(json!({ "initialized": true })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
fn session_create(state: tauri::State<'_, AppState>, session: NewSession) -> ApiResponse {
    match state.sqlite_store.create_session(&session) {
        Ok(()) => ApiResponse::ok(json!({ "created": true, "session_id": session.id })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
fn session_list(state: tauri::State<'_, AppState>) -> ApiResponse {
    match state.sqlite_store.list_sessions() {
        Ok(items) => ApiResponse::ok(json!({ "sessions": items })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
fn session_update_title(
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
fn message_create(state: tauri::State<'_, AppState>, message: NewMessage) -> ApiResponse {
    match state.sqlite_store.create_message(&message) {
        Ok(()) => ApiResponse::ok(json!({ "created": true, "message_id": message.id })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
fn message_list(state: tauri::State<'_, AppState>, session_id: String) -> ApiResponse {
    match state.sqlite_store.list_messages(&session_id) {
        Ok(items) => ApiResponse::ok(json!({ "messages": items })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
fn request_log_create(state: tauri::State<'_, AppState>, log: NewRequestLog) -> ApiResponse {
    match state.sqlite_store.create_request_log(&log) {
        Ok(()) => ApiResponse::ok(json!({ "created": true, "request_log_id": log.id })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
fn request_log_list_by_request_id(
    state: tauri::State<'_, AppState>,
    request_id: String,
) -> ApiResponse {
    match state.sqlite_store.list_request_logs_by_request_id(&request_id) {
        Ok(items) => ApiResponse::ok(json!({ "request_logs": items })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
fn session_soft_delete(
    state: tauri::State<'_, AppState>,
    session_id: String,
    deleted_at: String,
) -> ApiResponse {
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
fn session_restore(state: tauri::State<'_, AppState>, session_id: String) -> ApiResponse {
    match state.sqlite_store.restore_session(&session_id) {
        Ok(result) => ApiResponse::ok(json!({
            "restored": true,
            "session_id": session_id,
            "affected": result
        })),
        Err(err) => map_store_error(err),
    }
}

#[tauri::command]
async fn chat_summarize_title(
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
    
    let res = tauri::async_runtime::spawn_blocking(move || {
        let request = ChatRequestInput {
            profile_id,
            request_id: format!("title_gen_{}", iso_now()),
            session_id: None,
            model: Some(model),
            prompt: chat_provider::ChatPromptLayer {
                system: "You are a strict title generator. Your ONLY task is to extract a highly concise title (1 to 6 words) from the user's text.\nCRITICAL RULES:\n1. The title MUST be in the exact same language as the user's text. (e.g., if user text is Chinese, title must be Chinese).\n2. DO NOT answer the user's questions.\n3. DO NOT output thinking processes or conversational fillers.\n4. Output ONLY the title text itself, with no quotes or punctuation.".to_string(),
                runtime: "".to_string(),
                memory: "".to_string(),
                user: format!("User text:\n<text>\n{}\n</text>\n\nGenerate title in the EXACT SAME LANGUAGE as the text above:", user_text),
            },
            history: vec![],
        };

        let service = ChatService::new(HttpTransport);
        service.chat_with_retry(&request, &cfg)
    })
    .await
    .map_err(|e| e.to_string())?;

    match res {
        Ok(result) => {
            // Aggressively clean up the model's output
            let mut title = result.output_text.trim().to_string();
            title = title.trim_matches(|c| c == '"' || c == '\'' || c == '「' || c == '」' || c == '\n').to_string();
            // If the model still outputs a huge paragraph, fallback to a hard limit
            if title.chars().count() > 30 {
                title = format!("{}...", title.chars().take(27).collect::<String>());
            }
            if title.is_empty() { 
                title = "New Chat".to_string(); 
            }
            Ok(ApiResponse::ok(json!({ "title": title })))
        }
        Err(err) => {
            let (code, message, _) = error_to_parts(err);
            Ok(ApiResponse::err(&code, message))
        }
    }
}

#[tauri::command]
fn chat_send(
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

    tauri::async_runtime::spawn_blocking(move || {
        let state_clone = app_clone.state::<AppState>();
        let service = ChatService::new(HttpTransport);
        let mut emit_stream = |event: &ChatStreamEvent| {
            if let Err(e) = app_clone.emit(
                "chat_stream_event",
                json!({
                    "request_id": request_id_for_emit,
                    "event": event
                }),
            ) {
                error!("Failed to emit chat_stream_event: {}", e);
            }
        };
        info!("Sending chat request to provider...");
        match service.chat_with_retry_stream(&request, &cfg, &mut emit_stream) {
            Ok(result) => {
                info!("Chat request completed successfully. Request ID: {}", request.request_id);
                let latency_ms = started.elapsed().as_millis() as i64;
                
                // Manually emit the Done event with latency and usage
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

                let _ = state_clone.sqlite_store.init_schema();
                if let Err(err) = state_clone.sqlite_store.create_request_log(&NewRequestLog {
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
            }
            Err(err) => {
                error!("Chat request failed: {}", err);
                let (code, message, retryable) = error_to_parts(err);
                emit_stream(&ChatStreamEvent::Aborted {
                    code: code.clone(),
                    message: message.clone(),
                    retryable,
                });
                if let Err(write_err) = create_chat_failure_log(
                    &state_clone.sqlite_store,
                    &request,
                    resolved_provider_clone,
                    resolved_model_clone,
                    &code,
                    started.elapsed().as_millis() as i64,
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

#[tauri::command]
fn memory_extract_candidates(
    state: tauri::State<'_, AppState>,
    input: ExtractCandidatesInput,
) -> ApiResponse {
    match state.memory_pipeline.extract_candidates(input) {
        Ok(candidates) => ApiResponse::ok(json!({ "candidates": candidates })),
        Err(err) => map_memory_error(err),
    }
}

#[tauri::command]
fn memory_list_candidates(
    state: tauri::State<'_, AppState>,
    session_id: String,
    status: Option<String>,
) -> ApiResponse {
    match state
        .memory_pipeline
        .list_candidates(&session_id, status.as_deref())
    {
        Ok(candidates) => ApiResponse::ok(json!({ "candidates": candidates })),
        Err(err) => map_memory_error(err),
    }
}

#[tauri::command]
fn memory_review_candidate(
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
                // fallback to candidate_id or dummy string if session_id is empty (which happens for rejected candidates)
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
fn memory_soft_delete(
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
fn memory_restore(state: tauri::State<'_, AppState>, memory_id: String) -> ApiResponse {
    match state.memory_pipeline.restore_memory(&memory_id) {
        Ok(result) => ApiResponse::ok(json!({ "result": result })),
        Err(err) => map_memory_error(err),
    }
}

#[tauri::command]
fn memory_read(state: tauri::State<'_, AppState>, memory_id: String) -> ApiResponse {
    match state.memory_pipeline.read_memory_doc(&memory_id) {
        Ok(memory_doc) => ApiResponse::ok(json!({ "memory": memory_doc })),
        Err(err) => map_memory_error(err),
    }
}

#[tauri::command]
fn memory_search(
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
fn memory_list_long_term(
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

fn iso_now() -> String {
    chrono::Local::now().to_rfc3339()
}

fn next_log_id(request_id: &str, suffix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("log_{request_id}_{suffix}_{nanos}")
}

fn map_store_error(err: StoreError) -> ApiResponse {
    let text = err.to_string();
    let code = text
        .split(':')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("INTERNAL_ERROR")
        .to_string();
    ApiResponse::err(&code, text)
}

fn map_config_error(err: config_guardrails::ConfigError) -> ApiResponse {
    let text = err.to_string();
    let code = text
        .split(':')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("INTERNAL_ERROR")
        .to_string();
    ApiResponse::err(&code, text)
}

fn map_guardrail_error(err: config_guardrails::GuardrailError) -> ApiResponse {
    let text = err.to_string();
    let code = text
        .split(':')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("INVALID_REQUEST")
        .to_string();
    ApiResponse::err(&code, text)
}

fn map_memory_error(err: MemoryError) -> ApiResponse {
    let text = err.to_string();
    let code = text
        .split(':')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("INTERNAL_ERROR")
        .to_string();
    ApiResponse::err(&code, text)
}

use tauri::Manager;
use tracing::{error, info};
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
            
            // Set up file logger
            if let Some(home) = dirs::home_dir() {
                let logs_dir = home.join(".dragon-li").join("logs");
                let file_appender = RollingFileAppender::new(Rotation::DAILY, logs_dir, "dragon-li.log");
                let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
                
                // Optional: Store the guard in app state if we don't want it dropped,
                // but since setup runs once, we can leak it or keep it alive globally.
                // Box::leak is safe here because the app runs until exit.
                Box::leak(Box::new(_guard));

                tracing_subscriber::registry()
                    .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
                    .with(fmt::layer().with_writer(non_blocking))
                    .with(fmt::layer().with_writer(std::io::stdout)) // also log to stdout
                    .init();
                    
                info!("Logger initialized in ~/.dragon-li/logs/");
            }

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            runtime_info,
            start_agent,
            stop_agent,
            agent_status,
            agent_health_check,
            config_get,
            config_save,
            config_check_external_change,
            config_apply_external_change,
            guardrail_validate_path,
            guardrail_validate_capability,
            guardrail_validate_network,
            db_init,
            session_create,
            session_list,
            session_update_title,
            message_create,
            message_list,
            request_log_create,
            request_log_list_by_request_id,
            session_soft_delete,
            session_restore,
            chat_summarize_title,
            chat_send,
            memory_extract_candidates,
            memory_list_candidates,
            memory_review_candidate,
            memory_soft_delete,
            memory_restore,
            memory_read,
            memory_search,
            memory_list_long_term
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use chat_provider::ChatPromptLayer;
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
