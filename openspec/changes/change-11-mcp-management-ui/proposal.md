# Proposal: change-11-mcp-management-ui

## Why
MCP 协议层打通后，用户需要在 UI 上配置和管理这些连接器（MCP Servers）。因为各个 MCP Server 的启动方式与鉴权机制不同，前端需要提供配置入口，提供必要的运行环境（如执行命令、环境变量）或网络凭证（如 endpoint URL、Headers），并能直观看到连接状态和可用能力（Tools）。

## What
- **存储层**: 在 SQLite 中新增 `mcp_servers` 表，记录配置参数及状态。
- **Tauri Commands**: 提供增删改查及状态同步（连通性测试、拉取 Tools 列表）的接口。
- **前端 UI**: 扩展现有的 `SettingsPage.vue`，增加 "MCP Servers" 管理面板。
  - 支持三种协议的不同配置表单。
  - 状态指示灯（connected, error, disconnected, disabled）。
  - 连接成功后展示该 Server 提供的 Tools 列表。

## Dependencies
- `change-10-mcp-core-and-transport`
