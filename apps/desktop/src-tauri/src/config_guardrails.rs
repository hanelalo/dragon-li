use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{hash_map::DefaultHasher, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("CONFIG_NOT_FOUND: {0}")]
    NotFound(String),
    #[error("CONFIG_INVALID_JSON: {0}")]
    InvalidJson(String),
    #[error("CONFIG_SCHEMA_INVALID: {0}")]
    SchemaInvalid(String),
    #[error("CONFIG_RELOAD_REJECTED: {0}")]
    ReloadRejected(String),
    #[error("IO_ERROR: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum GuardrailError {
    #[error("INVALID_REQUEST: {0}")]
    InvalidRequest(String),
    #[error("BOUNDARY_PATH_DENIED: {0}")]
    PathDenied(String),
    #[error("BOUNDARY_CAPABILITY_DENIED: {0}")]
    CapabilityDenied(String),
    #[error("BOUNDARY_DOMAIN_DENIED: {0}")]
    DomainDenied(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Openai,
    Anthropic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiProfile {
    pub id: String,
    pub name: String,
    pub provider: Provider,
    pub base_url: String,
    pub api_key: String,
    pub default_model: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default)]
    pub brave_search_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiProfilesConfig {
    pub profiles: Vec<ApiProfile>,
    #[serde(default)]
    pub tools: ToolsConfig,
}

pub struct ConfigManager {
    path: PathBuf,
    current: Option<ApiProfilesConfig>,
    loaded_hash: Option<u64>,
    pending_external_change: bool,
}

impl ConfigManager {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            current: None,
            loaded_hash: None,
            pending_external_change: false,
        }
    }

    pub fn load_or_reload(&mut self) -> Result<&ApiProfilesConfig, ConfigError> {
        let cfg = load_and_validate(&self.path)?;
        let hash = hash_config(&cfg)?;
        self.current = Some(cfg);
        self.loaded_hash = Some(hash);
        self.pending_external_change = false;
        Ok(self.current.as_ref().expect("config should exist"))
    }

    pub fn current(&self) -> Option<&ApiProfilesConfig> {
        self.current.as_ref()
    }

    pub fn save_and_apply(&mut self, cfg: ApiProfilesConfig) -> Result<(), ConfigError> {
        validate_config(&cfg)?;
        atomic_write_json(&self.path, &cfg)?;
        let hash = hash_config(&cfg)?;
        self.current = Some(cfg);
        self.loaded_hash = Some(hash);
        self.pending_external_change = false;
        Ok(())
    }

    pub fn check_external_change(&mut self) -> Result<bool, ConfigError> {
        if !self.path.exists() {
            return Ok(false);
        }

        let disk_cfg = load_and_validate(&self.path)?;
        let disk_hash = hash_config(&disk_cfg)?;
        match self.loaded_hash {
            Some(loaded) => {
                self.pending_external_change = loaded != disk_hash;
                Ok(self.pending_external_change)
            }
            None => {
                self.pending_external_change = true;
                Ok(true)
            }
        }
    }

    pub fn apply_external_change(&mut self, confirm: bool) -> Result<&ApiProfilesConfig, ConfigError> {
        if !self.pending_external_change {
            if self.current.is_none() {
                return self.load_or_reload();
            }
            return Ok(self.current.as_ref().expect("config should exist"));
        }

        if !confirm {
            return Err(ConfigError::ReloadRejected(
                "external config change detected but not confirmed".to_string(),
            ));
        }

        self.load_or_reload()
    }
}

pub struct Guardrails {
    runtime_root: PathBuf,
    runtime_root_canonical: PathBuf,
}

impl Guardrails {
    pub fn new(runtime_root: PathBuf) -> Self {
        let runtime_root_canonical =
            canonicalize_allow_missing(&runtime_root).unwrap_or_else(|_| runtime_root.clone());
        Self {
            runtime_root,
            runtime_root_canonical,
        }
    }

    pub fn validate_file_path(&self, input: &str) -> Result<PathBuf, GuardrailError> {
        if input.trim().is_empty() {
            return Err(GuardrailError::InvalidRequest("path must not be empty".to_string()));
        }

        let raw = PathBuf::from(input);
        let absolute = if raw.is_absolute() {
            raw
        } else {
            self.runtime_root.join(raw)
        };
        let normalized = normalize_path(&absolute)?;
        let resolved = canonicalize_allow_missing(&normalized)?;
        if !resolved.starts_with(&self.runtime_root_canonical) {
            return Err(GuardrailError::PathDenied(format!(
                "path is outside allowed root: {}",
                resolved.display()
            )));
        }

        Ok(resolved)
    }

    pub fn validate_capability(&self, capability: &str) -> Result<(), GuardrailError> {
        match capability {
            "shell_exec" | "skill_execute" | "mcp_execute" => {
                Err(GuardrailError::CapabilityDenied(format!("{capability} is blocked in MVP")))
            }
            _ => Ok(()),
        }
    }

    pub fn validate_network_url(
        &self,
        target_url: &str,
        cfg: &ApiProfilesConfig,
    ) -> Result<String, GuardrailError> {
        let target = Url::parse(target_url)
            .map_err(|e| GuardrailError::InvalidRequest(format!("invalid url: {e}")))?;
        let target_host = target
            .host_str()
            .ok_or_else(|| GuardrailError::InvalidRequest("url host is missing".to_string()))?;

        let allowed_domains = provider_domains(cfg);
        if allowed_domains.contains(target_host) {
            return Ok(target_host.to_string());
        }

        Err(GuardrailError::DomainDenied(format!(
            "{target_host} is not in provider whitelist"
        )))
    }
}

fn load_and_validate(path: &Path) -> Result<ApiProfilesConfig, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::NotFound(path.display().to_string()));
    }

    let content = fs::read_to_string(path)?;
    let cfg = serde_json::from_str::<ApiProfilesConfig>(&content)
        .map_err(|e| ConfigError::InvalidJson(e.to_string()))?;
    validate_config(&cfg)?;
    Ok(cfg)
}

fn validate_config(cfg: &ApiProfilesConfig) -> Result<(), ConfigError> {
    let mut ids = HashSet::new();
    for p in &cfg.profiles {
        if p.id.trim().is_empty() {
            return Err(ConfigError::SchemaInvalid("profile.id is required".to_string()));
        }
        if !ids.insert(p.id.clone()) {
            return Err(ConfigError::SchemaInvalid(format!("duplicate profile id: {}", p.id)));
        }
        if p.name.trim().is_empty() {
            return Err(ConfigError::SchemaInvalid(format!("profile {} name is required", p.id)));
        }
        if p.base_url.trim().is_empty() {
            return Err(ConfigError::SchemaInvalid(format!(
                "profile {} base_url is required",
                p.id
            )));
        }
        let parsed_base = Url::parse(&p.base_url)
            .map_err(|e| ConfigError::SchemaInvalid(format!("invalid base_url for {}: {e}", p.id)))?;
        if parsed_base.scheme() != "https" && parsed_base.scheme() != "http" {
            return Err(ConfigError::SchemaInvalid(format!(
                "base_url must use http or https for {}",
                p.id
            )));
        }
        if p.api_key.trim().is_empty() {
            return Err(ConfigError::SchemaInvalid(format!(
                "profile {} api_key is required",
                p.id
            )));
        }
        if p.default_model.trim().is_empty() {
            return Err(ConfigError::SchemaInvalid(format!(
                "profile {} default_model is required",
                p.id
            )));
        }
        if p.created_at.trim().is_empty() || p.updated_at.trim().is_empty() {
            return Err(ConfigError::SchemaInvalid(format!(
                "profile {} created_at/updated_at is required",
                p.id
            )));
        }
    }
    Ok(())
}

fn atomic_write_json(path: &Path, cfg: &ApiProfilesConfig) -> Result<(), ConfigError> {
    let parent = path
        .parent()
        .ok_or_else(|| ConfigError::SchemaInvalid("config path has no parent".to_string()))?;
    fs::create_dir_all(parent)?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let tmp_path = parent.join(format!(".api_profiles.{stamp}.tmp"));
    let serialized = serde_json::to_string_pretty(cfg)
        .map_err(|e| ConfigError::InvalidJson(e.to_string()))?;

    fs::write(&tmp_path, serialized).map_err(|e| {
        println!("Error writing tmp config file: {}", e);
        ConfigError::Io(e)
    })?;
    fs::rename(&tmp_path, path).map_err(|e| {
        println!("Error renaming tmp config file to final path: {}", e);
        ConfigError::Io(e)
    })?;
    Ok(())
}

fn hash_config(cfg: &ApiProfilesConfig) -> Result<u64, ConfigError> {
    let serialized = serde_json::to_vec(cfg).map_err(|e| ConfigError::InvalidJson(e.to_string()))?;
    let mut hasher = DefaultHasher::new();
    serialized.hash(&mut hasher);
    Ok(hasher.finish())
}

fn provider_domains(cfg: &ApiProfilesConfig) -> HashSet<String> {
    let mut domains = HashSet::new();
    for profile in &cfg.profiles {
        if !profile.enabled {
            continue;
        }
        if let Ok(url) = Url::parse(&profile.base_url) {
            if let Some(host) = url.host_str() {
                domains.insert(host.to_string());
            }
        }
    }
    domains
}

fn normalize_path(path: &Path) -> Result<PathBuf, GuardrailError> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) => out.push(component.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                if !out.pop() {
                    return Err(GuardrailError::PathDenied(
                        "path traversal is not allowed".to_string(),
                    ));
                }
            }
        }
    }
    Ok(out)
}

fn canonicalize_allow_missing(path: &Path) -> Result<PathBuf, GuardrailError> {
    if path.exists() {
        return fs::canonicalize(path).map_err(|e| {
            GuardrailError::InvalidRequest(format!("failed to resolve path: {e}"))
        });
    }

    let mut suffix = Vec::<PathBuf>::new();
    let mut cursor = path.to_path_buf();
    while !cursor.exists() {
        let name = cursor.file_name().ok_or_else(|| {
            GuardrailError::InvalidRequest("failed to resolve path root".to_string())
        })?;
        suffix.push(PathBuf::from(name));
        cursor = cursor.parent().ok_or_else(|| {
            GuardrailError::InvalidRequest("failed to resolve path parent".to_string())
        })?.to_path_buf();
    }

    let mut base = fs::canonicalize(&cursor).map_err(|e| {
        GuardrailError::InvalidRequest(format!("failed to canonicalize existing path: {e}"))
    })?;
    for part in suffix.iter().rev() {
        base.push(part);
    }
    Ok(base)
}

pub fn config_to_json(cfg: &ApiProfilesConfig) -> Value {
    serde_json::to_value(cfg).unwrap_or_else(|_| json!({ "profiles": [], "tools": {} }))
}

pub fn config_path(runtime_root: &Path) -> PathBuf {
    runtime_root.join("config").join("api_profiles.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        std::env::temp_dir().join(format!("dragon-li-{name}-{millis}"))
    }

    fn valid_config(base_url: &str) -> ApiProfilesConfig {
        ApiProfilesConfig {
            profiles: vec![ApiProfile {
                id: "p1".to_string(),
                name: "Primary".to_string(),
                provider: Provider::Openai,
                base_url: base_url.to_string(),
                api_key: "sk-test".to_string(),
                default_model: "gpt-4o-mini".to_string(),
                enabled: true,
                created_at: "2026-04-05T00:00:00+08:00".to_string(),
                updated_at: "2026-04-05T00:00:00+08:00".to_string(),
            }],
            tools: ToolsConfig::default(),
        }
    }

    #[test]
    fn validates_schema() {
        let bad = valid_config("ftp://api.openai.com/v1");
        let err = validate_config(&bad).expect_err("ftp should be rejected");
        assert!(err.to_string().contains("CONFIG_SCHEMA_INVALID"));
    }

    #[test]
    fn writes_atomically_and_loads() {
        let root = temp_path("cfg");
        let path = root.join("config").join("api_profiles.json");
        let mut manager = ConfigManager::new(path.clone());
        let cfg = valid_config("https://api.openai.com/v1");

        manager.save_and_apply(cfg.clone()).expect("save should work");
        let loaded = manager.load_or_reload().expect("load should work");
        assert_eq!(loaded.profiles.len(), 1);
        assert!(path.exists());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn detects_external_change_with_confirmation() {
        let root = temp_path("cfg-reload");
        let path = root.join("config").join("api_profiles.json");
        let mut manager = ConfigManager::new(path.clone());
        manager
            .save_and_apply(valid_config("https://api.openai.com/v1"))
            .expect("initial save");

        let mut changed = valid_config("https://api.anthropic.com");
        changed.profiles[0].id = "p2".to_string();
        atomic_write_json(&path, &changed).expect("simulate external edit");

        assert!(manager.check_external_change().expect("check change"));
        let err = manager
            .apply_external_change(false)
            .expect_err("should require confirmation");
        assert!(err.to_string().contains("CONFIG_RELOAD_REJECTED"));

        let applied = manager
            .apply_external_change(true)
            .expect("confirm should apply");
        assert_eq!(applied.profiles[0].id, "p2");

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn enforces_guardrails() {
        let root = temp_path("guardrails");
        let guardrails = Guardrails::new(root.join(".dragon-li"));

        let inside = guardrails
            .validate_file_path("data/dragon_li.db")
            .expect("relative path under root should pass");
        assert!(inside.to_string_lossy().contains(".dragon-li"));

        let outside = guardrails.validate_file_path("/etc/passwd");
        assert!(outside.is_err());

        let blocked = guardrails.validate_capability("shell_exec");
        assert!(blocked.is_err());

        let cfg = valid_config("https://api.openai.com/v1");
        let ok = guardrails
            .validate_network_url("https://api.openai.com/v1/chat/completions", &cfg)
            .expect("whitelisted domain should pass");
        assert_eq!(ok, "api.openai.com");

        let denied = guardrails.validate_network_url("https://example.com", &cfg);
        assert!(denied.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn blocks_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = temp_path("guardrails-symlink");
        let runtime = root.join(".dragon-li");
        let outside = root.join("outside");
        fs::create_dir_all(runtime.join("data")).expect("create runtime dir");
        fs::create_dir_all(&outside).expect("create outside dir");
        fs::write(outside.join("secret.txt"), "x").expect("write outside file");
        symlink(&outside, runtime.join("escape")).expect("create symlink");

        let guardrails = Guardrails::new(runtime.clone());
        let escaped = guardrails.validate_file_path(
            &runtime.join("escape").join("secret.txt").display().to_string(),
        );
        assert!(escaped.is_err(), "symlink escape should be blocked");

        fs::remove_dir_all(root).ok();
    }
}
