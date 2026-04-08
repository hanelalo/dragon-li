# Proposal: change-18-search-api-key-config

## 核心目标
在系统配置层（Rust 后端、Python Agent、前端 UI）支持 Brave Search API Key 的全局配置与持久化，为网络搜索功能提供基础支撑。

## 背景与痛点
当前系统的配置主要围绕 LLM Provider（`ApiProfilesConfig` 中的 `profiles` 数组），缺乏全局级别的插件/工具配置项。要实现搜索，需要有地方安全地配置并读取 Brave Search API Key。

## 解决方案
1. **配置模型扩展**：在 `ApiProfilesConfig` 顶层新增 `brave_search_api_key` 字段。
2. **多端同步**：同步更新 Rust 层 (`config_guardrails.rs`) 和 Python 层 (`models.py`) 的结构体定义。
3. **前端支持**：在 `SettingsPage.vue` 中新增输入框，绑定全局状态，允许用户配置并保存。

## 验收标准
- 可以在前端设置页面输入并保存 Brave API Key。
- 重启应用后，Key 能够正确从本地 `.dragon-li/config/api_profiles.json` 中读取并回显。