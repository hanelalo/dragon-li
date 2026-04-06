use crate::config_guardrails::{config_path, ConfigManager, Guardrails};
use crate::memory_pipeline::MemoryPipeline;
use crate::sqlite_store::{default_db_path, SqliteStore};
use dirs::home_dir;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const RUNTIME_DIR_NAME: &str = ".dragon-li";
const RUNTIME_SUBDIRS: [&str; 5] = ["data", "memory", "config", "logs", "backups"];

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
        let runtime_root = init_runtime_dirs()?;
        let script_path = resolve_agent_script_path()?;

        Ok(Self {
            config_manager: Mutex::new(ConfigManager::new(config_path(&runtime_root))),
            guardrails: Guardrails::new(runtime_root.clone()),
            sqlite_store: SqliteStore::new(default_db_path(&runtime_root)),
            memory_pipeline: MemoryPipeline::new(
                runtime_root.clone(),
                default_db_path(&runtime_root),
            ),
            runtime_root,
            agent_manager: Mutex::new(AgentManager::new(script_path)),
        })
    }
}

pub struct AgentManager {
    script_path: PathBuf,
    child: Option<Child>,
}

impl AgentManager {
    pub fn new(script_path: PathBuf) -> Self {
        Self {
            script_path,
            child: None,
        }
    }

    pub fn start(&mut self) -> Result<Option<u32>, RuntimeError> {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait()?.is_none() {
                return Ok(child.id().into());
            }
            self.child = None;
        }

        let child = Command::new(python_binary())
            .arg(&self.script_path)
            .arg("--serve")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let pid = child.id();
        self.child = Some(child);

        Ok(Some(pid))
    }

    pub fn stop(&mut self) -> Result<(), RuntimeError> {
        if let Some(mut child) = self.child.take() {
            child.kill()?;
            let _ = child.wait();
        }

        Ok(())
    }

    pub fn status(&mut self) -> Result<(bool, Option<u32>), RuntimeError> {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait()?.is_none() {
                return Ok((true, child.id().into()));
            }
            self.child = None;
        }

        Ok((false, None))
    }

    pub fn health_check(&self) -> Result<bool, RuntimeError> {
        let output = Command::new(python_binary())
            .arg(&self.script_path)
            .arg("--health-check")
            .output()?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim() == "ok")
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

fn resolve_agent_script_path() -> Result<PathBuf, RuntimeError> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script_path = manifest_dir
        .join("..")
        .join("..")
        .join("..")
        .join("agent")
        .join("runtime_agent.py");

    if !script_path.exists() {
        return Err(RuntimeError::AgentScriptMissing(
            script_path.display().to_string(),
        ));
    }

    Ok(script_path)
}

fn python_binary() -> String {
    std::env::var("DRAGON_LI_PYTHON").unwrap_or_else(|_| "python3".to_string())
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
        let script_path = resolve_agent_script_path().expect("agent script should exist");
        let manager = AgentManager::new(script_path);
        let healthy = manager.health_check().expect("health check should execute");
        assert!(healthy);
    }

    #[test]
    fn start_status_stop_agent() {
        let script_path = resolve_agent_script_path().expect("agent script should exist");
        let mut manager = AgentManager::new(script_path);

        let pid = manager.start().expect("agent should start");
        assert!(pid.is_some());

        let (running, _) = manager.status().expect("status should work");
        assert!(running);

        manager.stop().expect("stop should work");

        let (running, _) = manager.status().expect("status after stop should work");
        assert!(!running);
    }
}
