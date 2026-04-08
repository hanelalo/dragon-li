use crate::config_guardrails::{ApiProfile, ApiProfilesConfig, Provider};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info};

const MAX_RETRIES: usize = 15;
const RETRY_BACKOFF_MS: [u64; 2] = [500, 1500];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatPromptLayer {
    pub system: String,
    pub runtime: String,
    pub memory: String,
    pub user: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatMessageContext {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatRequestInput {
    pub profile_id: String,
    pub request_id: String,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub prompt: ChatPromptLayer,
    #[serde(default)]
    pub enable_web_search: bool,
    #[serde(default)]
    pub history: Vec<ChatMessageContext>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PythonChatRequest<'a> {
    #[serde(flatten)]
    pub req: &'a ChatRequestInput,
    pub cfg: &'a ApiProfilesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatStreamEvent {
    Delta { text: String },
    Reasoning { text: String },
    Usage { tokens_in: u32, tokens_out: u32 },
    Done,
    Aborted {
        code: String,
        message: String,
        retryable: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatResult {
    pub request_id: String,
    pub provider: String,
    pub model: String,
    pub events: Vec<ChatStreamEvent>,
    pub output_text: String,
    pub reasoning_text: String,
    pub attempts: usize,
    pub tokens_in: u32,
    pub tokens_out: u32,
}

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("{code}: {message}")]
    Provider {
        code: String,
        message: String,
        retryable: bool,
        http_status: Option<u16>,
    },
    #[error("CONFIG_PROFILE_NOT_FOUND: {0}")]
    ProfileNotFound(String),
    #[error("CONFIG_PROFILE_NOT_FOUND: profile is disabled: {0}")]
    ProfileDisabled(String),
    #[error("INVALID_REQUEST: {0}")]
    InvalidRequest(String),
}

pub struct ChatService {
    uds_path: PathBuf,
}

impl ChatService {
    pub fn new(uds_path: PathBuf) -> Self {
        Self { uds_path }
    }

    pub async fn chat_with_retry_stream<F>(
        &self,
        req: &ChatRequestInput,
        cfg: &ApiProfilesConfig,
        on_event: &mut F,
    ) -> Result<ChatResult, ChatError>
    where
        F: FnMut(&ChatStreamEvent) + Send,
    {
        validate_request(req)?;
        let profile = find_enabled_profile(cfg, &req.profile_id)?;
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| profile.default_model.clone());
        let provider_name = match profile.provider {
            Provider::Openai => "openai",
            Provider::Anthropic => "anthropic",
        };

        let mut attempt = 0usize;
        loop {
            info!("Attempt {} to send chat request to local python agent", attempt + 1);
            let call_result = self.chat_once(req, cfg, provider_name, &model, on_event).await;
            match call_result {
                Ok(result) => return Ok(ChatResult { attempts: attempt + 1, ..result }),
                Err(err) => {
                    attempt += 1;
                    if !is_retryable(&err) || attempt > MAX_RETRIES {
                        return Err(err);
                    }
                    let backoff = RETRY_BACKOFF_MS
                        .get(attempt - 1)
                        .copied()
                        .unwrap_or_else(|| *RETRY_BACKOFF_MS.last().unwrap_or(&1500));
                    info!("Agent unreachable or error ({}). Retrying in {}ms...", err, backoff);
                    tokio::time::sleep(Duration::from_millis(backoff)).await;
                }
            }
        }
    }

    async fn chat_once<F>(
        &self,
        req: &ChatRequestInput,
        cfg: &ApiProfilesConfig,
        provider_name: &str,
        model: &str,
        on_event: &mut F,
    ) -> Result<ChatResult, ChatError>
    where
        F: FnMut(&ChatStreamEvent) + Send,
    {
        let py_req = PythonChatRequest { req, cfg };
        let body_json = serde_json::to_string(&py_req).map_err(|e| ChatError::Provider {
            code: "INTERNAL_ERROR".to_string(),
            message: format!("serialize failed: {}", e),
            retryable: false,
            http_status: None,
        })?;

        let mut events = Vec::new();

        self.post_uds_stream(&body_json, &mut |event| {
            on_event(&event);
            events.push(event);
        }).await?;

        let output_text = events
            .iter()
            .filter_map(|e| match e {
                ChatStreamEvent::Delta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        let reasoning_text = events
            .iter()
            .filter_map(|e| match e {
                ChatStreamEvent::Reasoning { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        let mut tokens_in = 0;
        let mut tokens_out = 0;
        let mut aborted_err = None;

        for e in &events {
            match e {
                ChatStreamEvent::Usage { tokens_in: ti, tokens_out: to } => {
                    tokens_in += *ti;
                    tokens_out += *to;
                }
                ChatStreamEvent::Aborted { code, message, retryable } => {
                    aborted_err = Some(ChatError::Provider {
                        code: code.clone(),
                        message: message.clone(),
                        retryable: *retryable,
                        http_status: None,
                    });
                }
                _ => {}
            }
        }

        if let Some(err) = aborted_err {
            return Err(err);
        }

        Ok(ChatResult {
            request_id: req.request_id.clone(),
            provider: provider_name.to_string(),
            model: model.to_string(),
            events,
            output_text,
            reasoning_text,
            attempts: 1,
            tokens_in,
            tokens_out,
        })
    }

    async fn post_uds_stream<F>(
        &self,
        body_json: &str,
        on_event: &mut F,
    ) -> Result<(), ChatError>
    where
        F: FnMut(ChatStreamEvent) + Send,
    {
        let mut stream = UnixStream::connect(&self.uds_path).await.map_err(|e| ChatError::Provider {
            code: "AGENT_UNREACHABLE".to_string(),
            message: format!("failed to connect to python agent: {}", e),
            retryable: true,
            http_status: None,
        })?;

        let req_str = format!(
            "POST /v1/chat/stream HTTP/1.0\r\n\
            Host: localhost\r\n\
            Content-Type: application/json\r\n\
            Content-Length: {}\r\n\
            \r\n\
            {}",
            body_json.len(),
            body_json
        );

        stream.write_all(req_str.as_bytes()).await.map_err(|e| ChatError::Provider {
            code: "AGENT_WRITE_FAILED".to_string(),
            message: format!("failed to write to python agent: {}", e),
            retryable: true,
            http_status: None,
        })?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        let mut status_code = 0;

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await.unwrap_or(0);
            if n == 0 { break; }
            if line.starts_with("HTTP/") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    status_code = parts[1].parse().unwrap_or(500);
                }
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
        }

        if !(200..300).contains(&status_code) {
            let mut body = String::new();
            let _ = reader.read_to_string(&mut body).await;
            return Err(ChatError::Provider {
                code: "AGENT_HTTP_ERROR".to_string(),
                message: format!("Agent returned HTTP {}: {}", status_code, body),
                retryable: true,
                http_status: Some(status_code),
            });
        }

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await.unwrap_or(0);
            if n == 0 { break; }
            let trimmed = line.trim();
            if trimmed.starts_with("data:") {
                let payload = trimmed["data:".len()..].trim();
                if payload == "[DONE]" { continue; }
                match serde_json::from_str::<ChatStreamEvent>(payload) {
                    Ok(event) => on_event(event),
                    Err(e) => {
                        error!("Failed to parse SSE event: {}, payload: {}", e, payload);
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn get_uds_json<R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<R, ChatError> {
        let mut attempt = 0usize;
        let mut stream = loop {
            match UnixStream::connect(&self.uds_path).await {
                Ok(s) => break s,
                Err(e) => {
                    attempt += 1;
                    if attempt > MAX_RETRIES {
                        return Err(ChatError::Provider {
                            code: "AGENT_UNREACHABLE".to_string(),
                            message: format!("failed to connect to python agent: {}", e),
                            retryable: true,
                            http_status: None,
                        });
                    }
                    let backoff = RETRY_BACKOFF_MS
                        .get(attempt - 1)
                        .copied()
                        .unwrap_or_else(|| *RETRY_BACKOFF_MS.last().unwrap_or(&1500));
                    info!("Agent unreachable for json api ({}). Retrying in {}ms...", e, backoff);
                    tokio::time::sleep(Duration::from_millis(backoff)).await;
                }
            }
        };

        let req_str = format!(
            "GET {} HTTP/1.0\r\n\
            Host: localhost\r\n\
            \r\n",
            path
        );

        stream.write_all(req_str.as_bytes()).await.map_err(|e| ChatError::Provider {
            code: "AGENT_WRITE_FAILED".to_string(),
            message: format!("failed to write to python agent: {}", e),
            retryable: true,
            http_status: None,
        })?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        let mut status_code = 0;

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await.unwrap_or(0);
            if n == 0 { break; }
            if line.starts_with("HTTP/") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    status_code = parts[1].parse().unwrap_or(500);
                }
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
        }

        let mut body = String::new();
        let _ = reader.read_to_string(&mut body).await;

        if !(200..300).contains(&status_code) {
            return Err(ChatError::Provider {
                code: "AGENT_HTTP_ERROR".to_string(),
                message: format!("Agent returned HTTP {}: {}", status_code, body),
                retryable: true,
                http_status: Some(status_code),
            });
        }

        serde_json::from_str(&body).map_err(|e| ChatError::Provider {
            code: "PROVIDER_BAD_REQUEST".to_string(),
            message: format!("failed to parse agent response: {}", e),
            retryable: false,
            http_status: None,
        })
    }

    pub async fn post_uds_json<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, ChatError> {
        let body_json = serde_json::to_string(body).map_err(|e| ChatError::Provider {
            code: "INTERNAL_ERROR".to_string(),
            message: format!("serialize failed: {}", e),
            retryable: false,
            http_status: None,
        })?;

        let mut attempt = 0usize;
        let mut stream = loop {
            match UnixStream::connect(&self.uds_path).await {
                Ok(s) => break s,
                Err(e) => {
                    attempt += 1;
                    if attempt > MAX_RETRIES {
                        return Err(ChatError::Provider {
                            code: "AGENT_UNREACHABLE".to_string(),
                            message: format!("failed to connect to python agent: {}", e),
                            retryable: true,
                            http_status: None,
                        });
                    }
                    let backoff = RETRY_BACKOFF_MS
                        .get(attempt - 1)
                        .copied()
                        .unwrap_or_else(|| *RETRY_BACKOFF_MS.last().unwrap_or(&1500));
                    info!("Agent unreachable for json api ({}). Retrying in {}ms...", e, backoff);
                    tokio::time::sleep(Duration::from_millis(backoff)).await;
                }
            }
        };

        let req_str = format!(
            "POST {} HTTP/1.0\r\n\
            Host: localhost\r\n\
            Content-Type: application/json\r\n\
            Content-Length: {}\r\n\
            \r\n\
            {}",
            path,
            body_json.len(),
            body_json
        );

        stream.write_all(req_str.as_bytes()).await.map_err(|e| ChatError::Provider {
            code: "AGENT_WRITE_FAILED".to_string(),
            message: format!("failed to write to python agent: {}", e),
            retryable: true,
            http_status: None,
        })?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        let mut status_code = 0;

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await.unwrap_or(0);
            if n == 0 { break; }
            if line.starts_with("HTTP/") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    status_code = parts[1].parse().unwrap_or(500);
                }
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
        }

        let mut body = String::new();
        let _ = reader.read_to_string(&mut body).await;

        if !(200..300).contains(&status_code) {
            return Err(ChatError::Provider {
                code: "AGENT_HTTP_ERROR".to_string(),
                message: format!("Agent returned HTTP {}: {}", status_code, body),
                retryable: true,
                http_status: Some(status_code),
            });
        }

        serde_json::from_str(&body).map_err(|e| ChatError::Provider {
            code: "PROVIDER_BAD_REQUEST".to_string(),
            message: format!("failed to parse agent response: {}", e),
            retryable: false,
            http_status: None,
        })
    }
}

fn validate_request(req: &ChatRequestInput) -> Result<(), ChatError> {
    if req.request_id.trim().is_empty() {
        return Err(ChatError::InvalidRequest("request_id must not be empty".to_string()));
    }
    if req.profile_id.trim().is_empty() {
        return Err(ChatError::InvalidRequest("profile_id must not be empty".to_string()));
    }
    if let Some(session_id) = &req.session_id {
        if session_id.trim().is_empty() {
            return Err(ChatError::InvalidRequest("session_id must not be empty when provided".to_string()));
        }
    }
    if let Some(model) = &req.model {
        if model.trim().is_empty() {
            return Err(ChatError::InvalidRequest("model must not be empty when provided".to_string()));
        }
    }
    validate_prompt(&req.prompt)?;
    Ok(())
}

fn validate_prompt(prompt: &ChatPromptLayer) -> Result<(), ChatError> {
    if prompt.user.trim().is_empty() {
        return Err(ChatError::InvalidRequest("prompt.user must not be empty".to_string()));
    }
    if prompt.user.chars().count() > 16_000 {
        return Err(ChatError::InvalidRequest("prompt.user is too long (max 16000 chars)".to_string()));
    }
    Ok(())
}

fn is_retryable(err: &ChatError) -> bool {
    matches!(
        err,
        ChatError::Provider {
            retryable: true,
            ..
        }
    )
}

fn find_enabled_profile<'a>(
    cfg: &'a ApiProfilesConfig,
    profile_id: &str,
) -> Result<&'a ApiProfile, ChatError> {
    let p = cfg
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| ChatError::ProfileNotFound(profile_id.to_string()))?;
    if !p.enabled {
        return Err(ChatError::ProfileDisabled(profile_id.to_string()));
    }
    Ok(p)
}



pub fn error_to_parts(err: ChatError) -> (String, String, bool) {
    match err {
        ChatError::Provider { code, message, retryable, .. } => (code, message, retryable),
        ChatError::ProfileNotFound(msg) => ("CONFIG_PROFILE_NOT_FOUND".to_string(), msg, false),
        ChatError::ProfileDisabled(msg) => ("CONFIG_PROFILE_NOT_FOUND".to_string(), msg, false),
        ChatError::InvalidRequest(msg) => ("INVALID_REQUEST".to_string(), msg, false),
    }
}
