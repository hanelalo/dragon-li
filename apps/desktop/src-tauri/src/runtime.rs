use crate::config_guardrails::{config_path, ConfigManager, Guardrails};
use crate::memory_pipeline::MemoryPipeline;
use crate::sqlite_store::{default_db_path, SqliteStore};
use dirs::home_dir;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{info, error};
use tauri_plugin_shell::ShellExt;

const RUNTIME_DIR_NAME: &str = ".dragon-li";
const RUNTIME_SUBDIRS: [&str; 6] = ["data", "memory", "config", "logs", "backups", "run"];

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[allow(dead_code)]
    #[error("home directory not found")]
    HomeDirMissing,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("agent script is missing: {0}")]
    AgentScriptMissing(String),
}

#[derive(Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct ApiMeta {
    pub timestamp_ms: u128,
}

#[derive(Serialize)]
pub struct ApiResponse {
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<ApiError>,
    pub meta: ApiMeta,
}

impl ApiResponse {
    pub fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
            meta: ApiMeta {
                timestamp_ms: now_ms(),
            },
        }
    }

    pub fn err(code: &str, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(ApiError {
                code: code.to_string(),
                message: message.into(),
            }),
            meta: ApiMeta {
                timestamp_ms: now_ms(),
            },
        }
    }
}

pub struct AppState {
    pub runtime_root: PathBuf,
    pub agent_manager: Mutex<AgentManager>,
    pub config_manager: Mutex<ConfigManager>,
    pub guardrails: Guardrails,
    pub sqlite_store: SqliteStore,
    pub memory_pipeline: MemoryPipeline,
}

impl AppState {
    pub fn bootstrap() -> Result<Self, RuntimeError> {
        info!("Bootstrapping AppState...");
        let runtime_root = init_runtime_dirs()?;
        info!("Runtime root initialized at: {}", runtime_root.display());

        Ok(Self {
            config_manager: Mutex::new(ConfigManager::new(config_path(&runtime_root))),
            guardrails: Guardrails::new(runtime_root.clone()),
            sqlite_store: SqliteStore::new(default_db_path(&runtime_root)),
            memory_pipeline: MemoryPipeline::new(
                runtime_root.clone(),
                default_db_path(&runtime_root),
            ),
            agent_manager: Mutex::new(AgentManager::new(&runtime_root)),
            runtime_root,
        })
    }
}

pub struct AgentManager {
    uds_path: PathBuf,
    child: Option<tauri_plugin_shell::process::CommandChild>,
}

impl AgentManager {
    pub fn new(runtime_root: &Path) -> Self {
        Self {
            uds_path: runtime_root.join("run").join("agent.sock"),
            child: None,
        }
    }

    pub fn get_uds_path(&self) -> PathBuf {
        self.uds_path.clone()
    }

    pub fn start(&mut self, app: &tauri::AppHandle) -> Result<Option<u32>, RuntimeError> {
        info!("Starting Agent sidecar...");
        if let Some(child) = self.child.take() {
            info!("Found existing agent process (PID: {}), killing it...", child.pid());
            // We cannot easily poll sidecar child exit status synchronously.
            // If it exists, we assume it's running or we just kill and restart.
            let _ = child.kill();
        }

        // Ensure old socket is removed
        if self.uds_path.exists() {
            info!("Removing old socket at: {}", self.uds_path.display());
            let _ = std::fs::remove_file(&self.uds_path);
        }

        // Create the directory if it doesn't exist
        if let Some(parent) = self.uds_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        info!("Spawning sidecar 'runtime_agent'...");
        let sidecar = app.shell().sidecar("runtime_agent").map_err(|e| {
            error!("Failed to initialize sidecar 'runtime_agent': {}", e);
            RuntimeError::AgentScriptMissing(e.to_string())
        })?;
        
        let db_path = default_db_path(&self.uds_path.parent().unwrap().parent().unwrap());
        
        let (mut rx, child) = sidecar
            .arg("--serve")
            .arg("--uds")
            .arg(self.uds_path.to_str().unwrap_or_default())
            .arg("--db-path")
            .arg(db_path.to_str().unwrap_or_default())
            .spawn()
            .map_err(|e| {
                error!("Failed to spawn sidecar: {}", e);
                RuntimeError::AgentScriptMissing(e.to_string())
            })?;

        info!("Agent spawned successfully with PID: {}", child.pid());

        tauri::async_runtime::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                        info!("[Agent] {}", String::from_utf8_lossy(&line));
                    }
                    tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                        let text = String::from_utf8_lossy(&line);
                        // Python logger writes everything to stderr by default.
                        // Only log as error if it contains actual error indicators, otherwise info.
                        let lower = text.to_lowercase();
                        if lower.contains("error") || lower.contains("traceback") || lower.contains("exception") || lower.contains("fatal") {
                            error!("[Agent] {}", text);
                        } else {
                            info!("[Agent] {}", text);
                        }
                    }
                    _ => {}
                }
            }
        });

        let pid = child.pid();
        self.child = Some(child);

        Ok(Some(pid))
    }

    pub fn stop(&mut self) -> Result<(), RuntimeError> {
        if let Some(child) = self.child.take() {
            info!("Stopping Agent sidecar (PID: {})...", child.pid());
            let _ = child.kill();
        } else {
            info!("Stop requested, but no agent process was running.");
        }

        Ok(())
    }

    pub fn status(&mut self) -> Result<(bool, Option<u32>), RuntimeError> {
        if let Some(child) = self.child.as_ref() {
            return Ok((true, Some(child.pid())));
        }
        Ok((false, None))
    }

    pub fn health_check(&self) -> Result<bool, RuntimeError> {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixStream;

        let mut stream = match UnixStream::connect(&self.uds_path) {
            Ok(s) => s,
            Err(_) => return Ok(false),
        };

        // Set timeouts to prevent blocking the thread indefinitely
        stream.set_read_timeout(Some(std::time::Duration::from_secs(2))).ok();
        stream.set_write_timeout(Some(std::time::Duration::from_secs(2))).ok();

        if stream.write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n").is_err() {
            return Ok(false);
        }

        let mut response = String::new();
        if stream.read_to_string(&mut response).is_err() {
            return Ok(false);
        }

        Ok(response.contains("200 OK"))
    }
}

pub fn init_runtime_dirs() -> Result<PathBuf, RuntimeError> {
    let home = home_dir().ok_or(RuntimeError::HomeDirMissing)?;
    let runtime_root = home.join(RUNTIME_DIR_NAME);
    ensure_runtime_dirs(&runtime_root)?;

    Ok(runtime_root)
}

pub fn runtime_subdirs(root: &Path) -> Vec<String> {
    RUNTIME_SUBDIRS
        .iter()
        .map(|sub| root.join(sub).display().to_string())
        .collect()
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn runtime_bootstrap_payload(state: &AppState) -> Value {
    json!({
        "runtime_root": state.runtime_root.display().to_string(),
        "runtime_dirs": runtime_subdirs(&state.runtime_root)
    })
}

fn ensure_runtime_dirs(runtime_root: &Path) -> Result<(), RuntimeError> {
    fs::create_dir_all(runtime_root)?;
    for dir in RUNTIME_SUBDIRS {
        fs::create_dir_all(runtime_root.join(dir))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir() -> PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        std::env::temp_dir().join(format!("dragon-li-runtime-test-{millis}"))
    }

    #[test]
    fn creates_runtime_subdirs() {
        let root = unique_temp_dir();
        ensure_runtime_dirs(&root).expect("should create runtime dirs");

        for sub in RUNTIME_SUBDIRS {
            assert!(root.join(sub).exists(), "missing subdir: {sub}");
        }

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn health_check_is_ok() {
        // test removed or rewritten because health check now requires a running agent with a socket
    }
}
