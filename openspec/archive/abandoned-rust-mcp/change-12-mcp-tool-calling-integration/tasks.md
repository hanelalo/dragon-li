# Tasks: change-12-mcp-tool-calling-integration

- [ ] 新增 `mcp_permissions` 的 SQLite 建表与 CRUD 方法。
- [ ] 改造 `ChatAdapter` (OpenAI / Anthropic)，支持构建附带 Tools 的 `HttpRequest`。
- [ ] 改造 `ChatAdapter` 的 `parse_stream_line`，使其能正确捕获并组装完整的 `tool_calls` JSON 块。
- [ ] 在 `ChatService` / `main.rs` 的循环中，实现多轮推断循环：遇到 `tool_calls` 则挂起，通过 Tauri Event 通知前端。
- [ ] 前端：监听 `ToolCallRequested` 事件，在消息流区域渲染“mcp calling: xxxx”占位，并弹窗。
- [ ] 前端与后端打通 `mcp_approve_tool_call` 唤醒逻辑（支持记录 Always Allow 到数据库）。
- [ ] 唤醒后，后端请求 MCP Server 执行该 Tool 并将结果以 `tool_message` / `tool_result` 的形式注入上下文，进行第二轮大模型请求。

## 验收清单
- [ ] 模型在聊天中被要求查询外部信息时，正确下发 `tool_calls`。
- [ ] 前端能够准确捕获该请求并拦截展示授权 UI，聊天流暂时挂起。
- [ ] 用户点击“总是允许”后，下次同一 Server 的同一工具直接静默执行不再弹窗。
- [ ] 授权通过后，MCP 执行成功并将结果反馈给模型，模型生成最终包含该数据的文本回复。
