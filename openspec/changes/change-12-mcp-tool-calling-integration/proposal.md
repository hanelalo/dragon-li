# Proposal: change-12-mcp-tool-calling-integration

## Why
连接器管理就绪后，系统的终极目标是**将 MCP 的 Tools 注入到大模型的对话流中**，让 Agent 真正能够调用这些工具解决用户问题。同时，为了安全管控，调用行为不能完全黑盒，必须引入用户显式授权机制（以及快捷的“总是允许”记录）。

## What
- **大模型适配**: 扩展 `chat_provider.rs` 中的 OpenAI/Anthropic Adapter，支持 `tools` Schema 的注入与 `tool_calls` 响应解析。
- **权限管控**: 新增 `mcp_permissions` 存储“总是允许”的规则。当遇到未授权调用时，暂停流式输出，向前端发送授权请求。
- **UI 交互**: 前端收到调用请求时，显示 `mcp calling: [工具名]...` 的状态，并弹出卡片让用户选择（Approve / Reject / Always Allow）。
- **对话闭环**: 授权通过后，后端执行 MCP Tool，获取 `tool_result`，再拼接到历史记录中，触发大模型进行二次推断生成最终回复。

## Dependencies
- `change-11-mcp-management-ui`
