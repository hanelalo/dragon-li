# Design: change-18-search-api-key-config

## 1. 数据结构变更
新增 `ToolsConfig` 结构体用于存放所有非大模型提供商的第三方工具/插件配置，使其与 `ApiProfilesConfig` 职责分离。
- **Rust** (`src-tauri/src/config_guardrails.rs`):
  ```rust
  pub struct ToolsConfig {
      #[serde(default)]
      pub brave_search_api_key: Option<String>,
  }

  // 假设有一个顶层的 AppConfig 包含它们，或者分别读取
  pub struct AppConfig {
      pub api_profiles: ApiProfilesConfig,
      #[serde(default)]
      pub tools: ToolsConfig,
  }
  ```
- **Python** (`agent/models.py`):
  ```python
  class ToolsConfig(BaseModel):
      brave_search_api_key: Optional[str] = None

  class AppConfig(BaseModel):
      api_profiles: ApiProfilesConfig
      tools: ToolsConfig = Field(default_factory=ToolsConfig)
  ```

## 2. 前端状态与 UI 变更
- **状态管理** (`apps/desktop/src/state/appState.js` 或类似逻辑): 
  确保 `appState.settings.tools.braveSearchApiKey` 可以被赋值。
- **UI** (`apps/desktop/src/pages/SettingsPage.vue`):
  - 在 `.layout` 外部（或内部专门的全局配置区域），新增一个卡片用于填写 Brave API Key，专门划分为“工具配置”区域。
  - 使用 `<input type="password">`。
  - 修改 `loadConfig`，在读取配置时赋值：`appState.settings.tools.braveSearchApiKey = res.data.config.tools.brave_search_api_key`。
  - 修改 `saveConfig`，在保存 Payload 中带上：`config: { api_profiles: newProfiles, tools: { brave_search_api_key: appState.settings.tools.braveSearchApiKey } }`。