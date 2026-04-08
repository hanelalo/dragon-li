# Tasks: change-18-search-api-key-config

- [x] **Rust 端**：新增 `ToolsConfig` 结构体并在其中增加 `brave_search_api_key`。修改相关配置读取和保存的接口以支持嵌套的 `ToolsConfig`，保证不与 `ApiProfilesConfig` 耦合。
- [x] **Python 端**：修改 `models.py` 增加 `ToolsConfig` 和顶层 `AppConfig`，或者在传递配置的地方将 tools 配置独立传递。
- [x] **前端逻辑**：修改 `SettingsPage.vue` 的 `loadConfig` 和 `saveConfig`，支持 `tools.brave_search_api_key` 的读取与写入。
- [x] **前端 UI**：在 `SettingsPage.vue` 中添加 Brave API Key 的输入框。
- [x] **测试验证**：启动应用，在设置中填写并保存，检查本地 `.dragon-li/config/api_profiles.json` 文件是否正确写入该字段。