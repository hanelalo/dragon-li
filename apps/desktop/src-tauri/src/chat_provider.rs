use crate::config_guardrails::{ApiProfile, ApiProfilesConfig, Provider};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(test)]
use std::sync::Mutex;
use std::io::{BufRead, BufReader};
use std::thread;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info};

const MAX_RETRIES: usize = 2;
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
    pub history: Vec<ChatMessageContext>,
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

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body_text: String,
}

#[derive(Debug, Error, Clone)]
pub enum TransportError {
    #[error("timeout")]
    Timeout,
    #[error("unreachable")]
    Unreachable,
    #[error("transport failed: {0}")]
    Failed(String),
}

pub trait Transport: Send + Sync {
    fn post_json(&self, req: &HttpRequest) -> Result<HttpResponse, TransportError>;

    fn post_json_stream(
        &self,
        req: &HttpRequest,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<HttpResponse, TransportError> {
        let resp = self.post_json(req)?;
        for line in resp.body_text.lines() {
            on_line(line);
        }
        Ok(resp)
    }
}

#[cfg(test)]
pub struct MockTransport {
    scripted: Mutex<Vec<Result<HttpResponse, TransportError>>>,
}
pub struct HttpTransport;

#[cfg(test)]
impl MockTransport {
    pub fn new(scripted: Vec<Result<HttpResponse, TransportError>>) -> Self {
        Self {
            scripted: Mutex::new(scripted),
        }
    }
}

#[cfg(test)]
impl Transport for MockTransport {
    fn post_json(&self, _req: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut guard = self.scripted.lock().expect("mock transport lock failed");
        if guard.is_empty() {
            return Err(TransportError::Failed("no scripted response".to_string()));
        }
        guard.remove(0)
    }
}


impl Transport for HttpTransport {
    fn post_json(&self, req: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = serde_json::to_string(&req.body)
            .map_err(|e| TransportError::Failed(format!("serialize request failed: {e}")))?;

        info!("Executing network request: URL={} Payload={}", req.url, body);

        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(60))
            .timeout_read(Duration::from_secs(120))
            .timeout_write(Duration::from_secs(60))
            .build();

        let mut request = agent.post(&req.url);
        for (k, v) in &req.headers {
            request = request.set(k, v);
        }

        match request.send_string(&body) {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.into_string().unwrap_or_default();
                Ok(HttpResponse {
                    status,
                    body_text: text,
                })
            }
            Err(ureq::Error::Status(status, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Ok(HttpResponse {
                    status,
                    body_text: text,
                })
            }
            Err(ureq::Error::Transport(t)) => {
                error!("HTTP request failed: {:?}", t);
                if t.to_string().to_lowercase().contains("timed out") {
                    return Err(TransportError::Timeout);
                }
                if t.kind() == ureq::ErrorKind::Dns {
                    return Err(TransportError::Unreachable);
                }
                Err(TransportError::Failed(t.to_string()))
            }
        }
    }

    fn post_json_stream(
        &self,
        req: &HttpRequest,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<HttpResponse, TransportError> {
        let body = serde_json::to_string(&req.body)
            .map_err(|e| TransportError::Failed(format!("serialize request failed: {e}")))?;

        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(30))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .build();

        let mut request = agent.post(&req.url);
        for (k, v) in &req.headers {
            request = request.set(k, v);
        }

        match request.send_string(&body) {
            Ok(resp) => {
                let status = resp.status();
                let mut body_text = String::new();
                let mut reader = BufReader::new(resp.into_reader());
                let mut line = String::new();
                loop {
                    line.clear();
                    let bytes = reader
                        .read_line(&mut line)
                        .map_err(|e| TransportError::Failed(format!("read stream failed: {e}")))?;
                    if bytes == 0 {
                        break;
                    }
                    on_line(line.trim_end_matches(['\r', '\n']));
                    body_text.push_str(&line);
                }
                Ok(HttpResponse { status, body_text })
            }
            Err(ureq::Error::Status(status, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Ok(HttpResponse {
                    status,
                    body_text: text,
                })
            }
            Err(ureq::Error::Transport(t)) => {
                if t.to_string().to_lowercase().contains("timed out") {
                    return Err(TransportError::Timeout);
                }
                if t.kind() == ureq::ErrorKind::Dns {
                    return Err(TransportError::Unreachable);
                }
                Err(TransportError::Failed(t.to_string()))
            }
        }
    }
}

pub struct ChatService<T: Transport> {
    transport: T,
}

impl<T: Transport> ChatService<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    #[allow(dead_code)]
    pub fn chat_with_retry(
        &self,
        req: &ChatRequestInput,
        cfg: &ApiProfilesConfig,
    ) -> Result<ChatResult, ChatError> {
        let mut no_op = |_event: &ChatStreamEvent| {};
        self.chat_with_retry_stream(req, cfg, &mut no_op)
    }

    pub fn chat_with_retry_stream(
        &self,
        req: &ChatRequestInput,
        cfg: &ApiProfilesConfig,
        on_event: &mut dyn FnMut(&ChatStreamEvent),
    ) -> Result<ChatResult, ChatError> {
        validate_request(req)?;
        let profile = find_enabled_profile(cfg, &req.profile_id)?;
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| profile.default_model.clone());

        let mut attempt = 0usize;
        loop {
            info!("Attempt {} to send chat request to {:?}", attempt + 1, profile.provider);
            let call_result = self.chat_once(req, profile, &model, on_event);
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
                    thread::sleep(Duration::from_millis(backoff));
                }
            }
        }
    }

    fn chat_once(
        &self,
        req: &ChatRequestInput,
        profile: &ApiProfile,
        model: &str,
        on_event: &mut dyn FnMut(&ChatStreamEvent),
    ) -> Result<ChatResult, ChatError> {
        let adapter = adapter_for_provider(&profile.provider);
        let http_req = adapter.build_request(req, profile, model);
        let mut events = Vec::new();
        let mut parse_error: Option<ChatError> = None;

        let resp = self
            .transport
            .post_json_stream(&http_req, &mut |line| {
                if parse_error.is_some() {
                    return;
                }
                match adapter.parse_stream_line(line) {
                    Ok(parsed_events) => {
                        for event in parsed_events {
                            on_event(&event);
                            events.push(event);
                        }
                    }
                    Err(err) => {
                        parse_error = Some(err);
                    }
                }
            })
            .map_err(map_transport_error)?;

        if !(200..300).contains(&resp.status) {
            return Err(map_http_error(resp.status, &resp.body_text));
        }

        if let Some(err) = parse_error {
            return Err(err);
        }
        if !matches!(events.last(), Some(ChatStreamEvent::Done)) {
            // We let main.rs emit the final done with latency, or emit a basic done here.
            // But since main.rs will emit the extended done payload, we can skip emitting Done here.
            // Actually, `chat_stream_event` listener might be confused if we send multiple `done`.
            // Let's remove Done from here and let main.rs send the rich Done event.
        }
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
        for e in &events {
            if let ChatStreamEvent::Usage { tokens_in: ti, tokens_out: to } = e {
                tokens_in += *ti;
                tokens_out += *to;
            }
        }

        Ok(ChatResult {
            request_id: req.request_id.clone(),
            provider: adapter.name().to_string(),
            model: model.to_string(),
            events,
            output_text,
            reasoning_text,
            attempts: 1,
            tokens_in,
            tokens_out,
        })
    }

    pub fn chat_with_retry_json<R: serde::de::DeserializeOwned>(
        &self,
        req: &ChatRequestInput,
        cfg: &ApiProfilesConfig,
    ) -> Result<R, ChatError> {
        validate_request(req)?;
        let profile = find_enabled_profile(cfg, &req.profile_id)?;
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| profile.default_model.clone());

        let mut attempt = 0usize;
        loop {
            info!("Attempt {} to send JSON request to {:?}", attempt + 1, profile.provider);
            let call_result = self.chat_once_json(req, profile, &model);
            match call_result {
                Ok(result) => return Ok(result),
                Err(err) => {
                    attempt += 1;
                    if !is_retryable(&err) || attempt > MAX_RETRIES {
                        return Err(err);
                    }
                    let backoff = RETRY_BACKOFF_MS
                        .get(attempt - 1)
                        .copied()
                        .unwrap_or_else(|| *RETRY_BACKOFF_MS.last().unwrap_or(&1500));
                    thread::sleep(Duration::from_millis(backoff));
                }
            }
        }
    }

    fn chat_once_json<R: serde::de::DeserializeOwned>(
        &self,
        req: &ChatRequestInput,
        profile: &ApiProfile,
        model: &str,
    ) -> Result<R, ChatError> {
        let adapter = adapter_for_provider(&profile.provider);
        let http_req = adapter.build_json_request(req, profile, model);

        let resp = self
            .transport
            .post_json(&http_req)
            .map_err(map_transport_error)?;

        if !(200..300).contains(&resp.status) {
            return Err(map_http_error(resp.status, &resp.body_text));
        }

        let json_value = adapter.parse_json_response(&resp.body_text)?;
        
        // Print the parsed JSON object to help with debugging
        info!("Parsed JSON from provider: {}", serde_json::to_string(&json_value).unwrap_or_default());
        
        serde_json::from_value(json_value).map_err(|e| ChatError::Provider {
            code: "PROVIDER_BAD_REQUEST".to_string(),
            message: format!("failed to deserialize typed JSON: {e}"),
            retryable: false,
            http_status: None,
        })
    }
}

fn validate_request(req: &ChatRequestInput) -> Result<(), ChatError> {
    if req.request_id.trim().is_empty() {
        return Err(ChatError::InvalidRequest(
            "request_id must not be empty".to_string(),
        ));
    }
    if req.profile_id.trim().is_empty() {
        return Err(ChatError::InvalidRequest(
            "profile_id must not be empty".to_string(),
        ));
    }
    if let Some(session_id) = &req.session_id {
        if session_id.trim().is_empty() {
            return Err(ChatError::InvalidRequest(
                "session_id must not be empty when provided".to_string(),
            ));
        }
    }
    if let Some(model) = &req.model {
        if model.trim().is_empty() {
            return Err(ChatError::InvalidRequest(
                "model must not be empty when provided".to_string(),
            ));
        }
    }
    validate_prompt(&req.prompt)?;
    Ok(())
}

fn validate_prompt(prompt: &ChatPromptLayer) -> Result<(), ChatError> {
    if prompt.user.trim().is_empty() {
        return Err(ChatError::InvalidRequest(
            "prompt.user must not be empty".to_string(),
        ));
    }
    if prompt.user.chars().count() > 16_000 {
        return Err(ChatError::InvalidRequest(
            "prompt.user is too long (max 16000 chars)".to_string(),
        ));
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

trait ProviderAdapter {
    fn name(&self) -> &'static str;
    fn build_request(&self, req: &ChatRequestInput, profile: &ApiProfile, model: &str) -> HttpRequest;
    fn parse_stream_line(&self, line: &str) -> Result<Vec<ChatStreamEvent>, ChatError>;
    
    fn build_json_request(&self, req: &ChatRequestInput, profile: &ApiProfile, model: &str) -> HttpRequest;
    fn parse_json_response(&self, response_body: &str) -> Result<serde_json::Value, ChatError>;
}

fn strip_markdown_json(text: &str) -> &str {
    let mut t = text.trim();
    if t.starts_with("```json") {
        t = t["```json".len()..].trim();
    } else if t.starts_with("```") {
        t = t["```".len()..].trim();
    }
    if t.ends_with("```") {
        t = t[..t.len() - 3].trim();
    }
    t
}

struct OpenAiAdapter;
struct AnthropicAdapter;

fn adapter_for_provider(provider: &Provider) -> Box<dyn ProviderAdapter> {
    match provider {
        Provider::Openai => Box::new(OpenAiAdapter),
        Provider::Anthropic => Box::new(AnthropicAdapter),
    }
}

impl ProviderAdapter for OpenAiAdapter {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn build_request(&self, req: &ChatRequestInput, profile: &ApiProfile, model: &str) -> HttpRequest {
        let mut messages = Vec::new();

        // 1. Combine system and runtime into a single system message
        let system_parts: Vec<&str> = [req.prompt.system.as_str(), req.prompt.runtime.as_str()]
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect();
        if !system_parts.is_empty() {
            messages.push(json!({"role": "system", "content": system_parts.join("\n\n")}));
        }

        // 2. Insert history messages
        for msg in &req.history {
            messages.push(json!({"role": msg.role.clone(), "content": msg.content.clone()}));
        }

        // 3. Combine memory and user into a single user message
        let user_parts: Vec<&str> = [req.prompt.memory.as_str(), req.prompt.user.as_str()]
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect();
        if !user_parts.is_empty() {
            messages.push(json!({"role": "user", "content": user_parts.join("\n\n")}));
        } else {
            // OpenAI requires at least a non-empty user message usually, but if it's completely empty, we send a space to avoid 400 error.
            messages.push(json!({"role": "user", "content": " "}));
        }

        HttpRequest {
            url: format!("{}/chat/completions", profile.base_url.trim_end_matches('/')),
            headers: vec![
                ("Authorization".to_string(), format!("Bearer {}", profile.api_key)),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: json!({
                "model": model,
                "stream": true,
                "stream_options": { "include_usage": true },
                "messages": messages
            }),
        }
    }

    fn parse_stream_line(&self, line: &str) -> Result<Vec<ChatStreamEvent>, ChatError> {
        let Some(payload) = parse_sse_payload(line) else {
            return Ok(vec![]);
        };
        if payload == "[DONE]" {
            return Ok(vec![]); // Will be handled by main.rs emitting rich Done event
        }

        let value: Value = serde_json::from_str(payload).map_err(|e| ChatError::Provider {
            code: "PROVIDER_BAD_REQUEST".to_string(),
            message: format!("invalid OpenAI stream payload: {e}"),
            retryable: false,
            http_status: None,
        })?;

        let mut events = Vec::new();
        
        if let Some(usage) = value.get("usage") {
            if let (Some(prompt_tokens), Some(completion_tokens)) = (
                usage.get("prompt_tokens").and_then(Value::as_u64),
                usage.get("completion_tokens").and_then(Value::as_u64),
            ) {
                events.push(ChatStreamEvent::Usage {
                    tokens_in: prompt_tokens as u32,
                    tokens_out: completion_tokens as u32,
                });
            }
        }

        if let Some(delta) = value
            .get("choices")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("delta"))
        {
            if let Some(reasoning) = delta.get("reasoning_content").and_then(Value::as_str) {
                if !reasoning.is_empty() {
                    events.push(ChatStreamEvent::Reasoning {
                        text: reasoning.to_string(),
                    });
                }
            }
            if let Some(content) = delta.get("content").and_then(Value::as_str) {
                if !content.is_empty() {
                    events.push(ChatStreamEvent::Delta {
                        text: content.to_string(),
                    });
                }
            }
        }
        Ok(events)
    }

    fn build_json_request(&self, req: &ChatRequestInput, profile: &ApiProfile, model: &str) -> HttpRequest {
        let mut http_req = self.build_request(req, profile, model);
        if let Some(obj) = http_req.body.as_object_mut() {
            obj.insert("stream".to_string(), json!(false));
            obj.remove("stream_options");
            obj.insert("response_format".to_string(), json!({"type": "json_object"}));
        }
        http_req
    }

    fn parse_json_response(&self, response_body: &str) -> Result<serde_json::Value, ChatError> {
        let value: Value = serde_json::from_str(response_body).map_err(|e| ChatError::Provider {
            code: "PROVIDER_BAD_REQUEST".to_string(),
            message: format!("invalid OpenAI json payload: {e}"),
            retryable: false,
            http_status: None,
        })?;

        let content = value
            .get("choices")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("message"))
            .and_then(|v| v.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let clean_json = strip_markdown_json(content);
        serde_json::from_str(clean_json).map_err(|e| ChatError::Provider {
            code: "PROVIDER_BAD_REQUEST".to_string(),
            message: format!("failed to parse structured JSON: {e}"),
            retryable: false,
            http_status: None,
        })
    }
}

impl ProviderAdapter for AnthropicAdapter {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn build_request(&self, req: &ChatRequestInput, profile: &ApiProfile, model: &str) -> HttpRequest {
        let system_parts: Vec<&str> = [
            req.prompt.system.as_str(),
            req.prompt.runtime.as_str(),
        ]
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect();
        let system_all = system_parts.join("\n\n");

        let user_parts: Vec<&str> = [
            req.prompt.memory.as_str(),
            req.prompt.user.as_str(),
        ]
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect();
        let user_all = if user_parts.is_empty() {
            " ".to_string()
        } else {
            user_parts.join("\n\n")
        };

        let mut raw_messages = Vec::new();
        for msg in &req.history {
            raw_messages.push((msg.role.clone(), msg.content.clone()));
        }
        raw_messages.push(("user".to_string(), user_all));

        // Merge consecutive messages of the same role
        let mut messages: Vec<Value> = Vec::new();
        for (role, content) in raw_messages {
            if let Some(last) = messages.last_mut() {
                if last["role"].as_str() == Some(role.as_str()) {
                    let old_content = last["content"].as_str().unwrap_or_default();
                    *last = json!({
                        "role": role,
                        "content": format!("{old_content}\n\n{content}")
                    });
                    continue;
                }
            }
            messages.push(json!({"role": role, "content": content}));
        }

        // Ensure the first message is 'user' for Anthropic
        if let Some(first) = messages.first() {
            if first["role"].as_str() == Some("assistant") {
                messages.insert(0, json!({"role": "user", "content": " "}));
            }
        }

        HttpRequest {
            url: format!("{}/v1/messages", profile.base_url.trim_end_matches('/')),
            headers: vec![
                ("x-api-key".to_string(), profile.api_key.clone()),
                ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ("content-type".to_string(), "application/json".to_string()),
            ],
            body: json!({
                "model": model,
                "stream": true,
                "max_tokens": 1024,
                "system": system_all,
                "messages": messages
            }),
        }
    }

    fn parse_stream_line(&self, line: &str) -> Result<Vec<ChatStreamEvent>, ChatError> {
        let Some(payload) = parse_sse_payload(line) else {
            return Ok(vec![]);
        };
        if payload == "[DONE]" {
            return Ok(vec![]); // Handled by main.rs
        }
        let value: Value = serde_json::from_str(payload).map_err(|e| ChatError::Provider {
            code: "PROVIDER_BAD_REQUEST".to_string(),
            message: format!("invalid Anthropic stream payload: {e}"),
            retryable: false,
            http_status: None,
        })?;
        let mut events = Vec::new();
        let event_type = value.get("type").and_then(Value::as_str).unwrap_or_default();
        if event_type == "message_start" {
            if let Some(usage) = value.get("message").and_then(|m| m.get("usage")) {
                if let Some(input_tokens) = usage.get("input_tokens").and_then(Value::as_u64) {
                    events.push(ChatStreamEvent::Usage {
                        tokens_in: input_tokens as u32,
                        tokens_out: 0,
                    });
                }
            }
        }
        
        if event_type == "message_delta" {
            if let Some(usage) = value.get("usage") {
                if let Some(output_tokens) = usage.get("output_tokens").and_then(Value::as_u64) {
                    events.push(ChatStreamEvent::Usage {
                        tokens_in: 0,
                        tokens_out: output_tokens as u32,
                    });
                }
            }
        }

        if event_type == "content_block_delta" {
            if let Some(delta) = value.get("delta") {
                let delta_type = delta.get("type").and_then(Value::as_str).unwrap_or_default();
                if delta_type == "text_delta" || delta.get("text").is_some() {
                    if let Some(text) = delta.get("text").and_then(Value::as_str) {
                        if !text.is_empty() {
                            events.push(ChatStreamEvent::Delta {
                                text: text.to_string(),
                            });
                        }
                    }
                }
                if delta_type == "thinking_delta" {
                    if let Some(thinking) = delta.get("thinking").and_then(Value::as_str) {
                        if !thinking.is_empty() {
                            events.push(ChatStreamEvent::Reasoning {
                                text: thinking.to_string(),
                            });
                        }
                    }
                }
            }
        }
        if event_type == "message_stop" {
            events.push(ChatStreamEvent::Done);
        }
        Ok(events)
    }

    fn build_json_request(&self, req: &ChatRequestInput, profile: &ApiProfile, model: &str) -> HttpRequest {
        let mut http_req = self.build_request(req, profile, model);
        if let Some(obj) = http_req.body.as_object_mut() {
            obj.insert("stream".to_string(), json!(false));
            if let Some(messages) = obj.get_mut("messages").and_then(|m| m.as_array_mut()) {
                messages.push(json!({
                    "role": "assistant",
                    "content": "{"
                }));
            }
        }
        http_req
    }

    fn parse_json_response(&self, response_body: &str) -> Result<serde_json::Value, ChatError> {
        let value: Value = serde_json::from_str(response_body).map_err(|e| ChatError::Provider {
            code: "PROVIDER_BAD_REQUEST".to_string(),
            message: format!("invalid Anthropic json payload: {e}"),
            retryable: false,
            http_status: None,
        })?;

        let content = value
            .get("content")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let full_content = format!("{{{content}");
        let clean_json = strip_markdown_json(&full_content);

        // Sometimes Anthropic may return the opening brace itself, resulting in `{{...`
        let clean_json = if clean_json.starts_with("{{") {
            clean_json[1..].to_string()
        } else {
            clean_json.to_string()
        };

        serde_json::from_str(&clean_json).map_err(|e| ChatError::Provider {
            code: "PROVIDER_BAD_REQUEST".to_string(),
            message: format!("failed to parse structured JSON: {e}"),
            retryable: false,
            http_status: None,
        })
    }
}

fn parse_sse_payload(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.starts_with("data:") {
        return None;
    }
    Some(trimmed.trim_start_matches("data:").trim())
}

fn map_transport_error(err: TransportError) -> ChatError {
    match err {
        TransportError::Timeout => ChatError::Provider {
            code: "PROVIDER_TIMEOUT".to_string(),
            message: "request timed out".to_string(),
            retryable: true,
            http_status: None,
        },
        TransportError::Unreachable => ChatError::Provider {
            code: "PROVIDER_UNREACHABLE".to_string(),
            message: "network unreachable or DNS failed".to_string(),
            retryable: true,
            http_status: None,
        },
        TransportError::Failed(msg) => ChatError::Provider {
            code: "INTERNAL_ERROR".to_string(),
            message: msg,
            retryable: false,
            http_status: None,
        },
    }
}

fn map_http_error(status: u16, body: &str) -> ChatError {
    match status {
        400..=499 => {
            if status == 401 || status == 403 {
                ChatError::Provider {
                    code: "PROVIDER_AUTH_FAILED".to_string(),
                    message: compact_error(body, "provider auth failed"),
                    retryable: false,
                    http_status: Some(status),
                }
            } else if status == 429 {
                ChatError::Provider {
                    code: "PROVIDER_RATE_LIMITED".to_string(),
                    message: compact_error(body, "provider rate limited"),
                    retryable: false,
                    http_status: Some(status),
                }
            } else {
                ChatError::Provider {
                    code: "PROVIDER_BAD_REQUEST".to_string(),
                    message: compact_error(body, "provider bad request"),
                    retryable: false,
                    http_status: Some(status),
                }
            }
        }
        500..=599 => ChatError::Provider {
            code: "PROVIDER_SERVER_ERROR".to_string(),
            message: compact_error(body, "provider server error"),
            retryable: true,
            http_status: Some(status),
        },
        _ => ChatError::Provider {
            code: "INTERNAL_ERROR".to_string(),
            message: compact_error(body, "unexpected provider response"),
            retryable: false,
            http_status: Some(status),
        },
    }
}

fn compact_error(body: &str, fallback: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return fallback.to_string();
    }
    trimmed.chars().take(300).collect()
}

pub fn error_to_parts(err: ChatError) -> (String, String, bool) {
    match err {
        ChatError::Provider {
            code,
            message,
            retryable,
            ..
        } => (code, message, retryable),
        ChatError::ProfileNotFound(msg) => ("CONFIG_PROFILE_NOT_FOUND".to_string(), msg, false),
        ChatError::ProfileDisabled(msg) => ("CONFIG_PROFILE_NOT_FOUND".to_string(), msg, false),
        ChatError::InvalidRequest(msg) => ("INVALID_REQUEST".to_string(), msg, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_guardrails::ApiProfile;

    fn cfg_with_profile(provider: Provider) -> ApiProfilesConfig {
        let (id, base_url, key, model) = match provider {
            Provider::Openai => ("openai-main", "https://api.openai.com/v1", "sk-openai", "gpt-4o-mini"),
            Provider::Anthropic => (
                "anthropic-main",
                "https://api.anthropic.com",
                "sk-ant",
                "claude-sonnet-4-5",
            ),
        };
        ApiProfilesConfig {
            profiles: vec![ApiProfile {
                id: id.to_string(),
                name: id.to_string(),
                provider,
                base_url: base_url.to_string(),
                api_key: key.to_string(),
                default_model: model.to_string(),
                enabled: true,
                created_at: "2026-04-05T00:00:00+08:00".to_string(),
                updated_at: "2026-04-05T00:00:00+08:00".to_string(),
            }],
        }
    }

    fn req(profile_id: &str) -> ChatRequestInput {
        ChatRequestInput {
            profile_id: profile_id.to_string(),
            request_id: "req_1".to_string(),
            session_id: Some("s1".to_string()),
            model: None,
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
    fn openai_stream_parsed() {
        let cfg = cfg_with_profile(Provider::Openai);
        let body = r#"data: {"choices":[{"delta":{"content":"Hi"}}]}
data: {"choices":[{"delta":{"content":" there"}}]}
data: [DONE]"#;
        let transport = MockTransport::new(vec![Ok(HttpResponse {
            status: 200,
            body_text: body.to_string(),
        })]);
        let svc = ChatService::new(transport);
        let out = svc.chat_with_retry(&req("openai-main"), &cfg).expect("chat ok");
        assert_eq!(out.output_text, "Hi there");
        assert_eq!(out.attempts, 1);
    }

    #[test]
    fn anthropic_stream_parsed() {
        let cfg = cfg_with_profile(Provider::Anthropic);
        let body = r#"data: {"type":"content_block_delta","delta":{"text":"Hello"}}
data: {"type":"content_block_delta","delta":{"text":" world"}}
data: {"type":"message_stop"}"#;
        let transport = MockTransport::new(vec![Ok(HttpResponse {
            status: 200,
            body_text: body.to_string(),
        })]);
        let svc = ChatService::new(transport);
        let out = svc
            .chat_with_retry(&req("anthropic-main"), &cfg)
            .expect("chat ok");
        assert_eq!(out.output_text, "Hello world");
    }

    #[test]
    fn retries_on_server_error() {
        let cfg = cfg_with_profile(Provider::Openai);
        let transport = MockTransport::new(vec![
            Ok(HttpResponse {
                status: 500,
                body_text: "upstream failed".to_string(),
            }),
            Ok(HttpResponse {
                status: 200,
                body_text: "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\ndata: [DONE]".to_string(),
            }),
        ]);
        let svc = ChatService::new(transport);
        let out = svc.chat_with_retry(&req("openai-main"), &cfg).expect("should retry");
        assert_eq!(out.output_text, "ok");
        assert_eq!(out.attempts, 2);
    }

    #[test]
    fn no_retry_on_429() {
        let cfg = cfg_with_profile(Provider::Openai);
        let transport = MockTransport::new(vec![Ok(HttpResponse {
            status: 429,
            body_text: "rate limited".to_string(),
        })]);
        let svc = ChatService::new(transport);
        let err = svc
            .chat_with_retry(&req("openai-main"), &cfg)
            .expect_err("429 should fail");
        let (code, _, retryable) = error_to_parts(err);
        assert_eq!(code, "PROVIDER_RATE_LIMITED");
        assert!(!retryable);
    }

    #[test]
    fn retries_on_timeout_then_success() {
        let cfg = cfg_with_profile(Provider::Openai);
        let transport = MockTransport::new(vec![
            Err(TransportError::Timeout),
            Ok(HttpResponse {
                status: 200,
                body_text: "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\ndata: [DONE]".to_string(),
            }),
        ]);
        let svc = ChatService::new(transport);
        let out = svc.chat_with_retry(&req("openai-main"), &cfg).expect("should retry");
        assert_eq!(out.output_text, "ok");
        assert_eq!(out.attempts, 2);
    }

    #[test]
    fn auth_failed_is_not_retryable() {
        let cfg = cfg_with_profile(Provider::Openai);
        let transport = MockTransport::new(vec![Ok(HttpResponse {
            status: 401,
            body_text: "unauthorized".to_string(),
        })]);
        let svc = ChatService::new(transport);
        let err = svc
            .chat_with_retry(&req("openai-main"), &cfg)
            .expect_err("401 should fail");
        let (code, _, retryable) = error_to_parts(err);
        assert_eq!(code, "PROVIDER_AUTH_FAILED");
        assert!(!retryable);
    }

    #[test]
    fn missing_profile_maps_to_config_profile_not_found() {
        let cfg = cfg_with_profile(Provider::Openai);
        let transport = MockTransport::new(vec![Ok(HttpResponse {
            status: 200,
            body_text: "data: [DONE]".to_string(),
        })]);
        let svc = ChatService::new(transport);
        let err = svc
            .chat_with_retry(&req("missing-profile"), &cfg)
            .expect_err("missing profile should fail");
        let (code, _, retryable) = error_to_parts(err);
        assert_eq!(code, "CONFIG_PROFILE_NOT_FOUND");
        assert!(!retryable);
    }

    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct DummyJson {
        message: String,
        status: String,
    }

    #[test]
    fn openai_json_parsed() {
        let cfg = cfg_with_profile(Provider::Openai);
        let body = r#"{"choices":[{"message":{"content":"```json\n{\"message\":\"hello\",\"status\":\"ok\"}\n```"}}]}"#;
        let transport = MockTransport::new(vec![Ok(HttpResponse {
            status: 200,
            body_text: body.to_string(),
        })]);
        let svc = ChatService::new(transport);
        let out: DummyJson = svc.chat_with_retry_json(&req("openai-main"), &cfg).expect("chat json ok");
        assert_eq!(out.message, "hello");
        assert_eq!(out.status, "ok");
    }

    #[test]
    fn anthropic_json_parsed() {
        let cfg = cfg_with_profile(Provider::Anthropic);
        // Note: Anthropic's output will be missing the leading `{` because of prefill.
        let body = r#"{"content":[{"text":"\n  \"message\": \"hello\",\n  \"status\": \"ok\"\n}"}]}"#;
        let transport = MockTransport::new(vec![Ok(HttpResponse {
            status: 200,
            body_text: body.to_string(),
        })]);
        let svc = ChatService::new(transport);
        let out: DummyJson = svc.chat_with_retry_json(&req("anthropic-main"), &cfg).expect("chat json ok");
        assert_eq!(out.message, "hello");
        assert_eq!(out.status, "ok");
    }
}
