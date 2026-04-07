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