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