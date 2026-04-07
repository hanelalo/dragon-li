# Design: change-12-mcp-tool-calling-integration

## 1. Prompt & Schema 注入
- 当用户发起 `chat_send` 时，后端从 `AppState::McpConnectionManager` 获取所有状态为 `connected` 且对应数据库记录 `enabled=1` 的 MCP Servers，调用 `tools/list`，将其组装成大模型原生支持的 Tools Schema 数组格式。
- 修改 `ChatProvider`，将组装好的 Tools 追加至 OpenAI / Anthropic 的请求 Payload 中。

## 2. 授权与权限存储
新增 SQLite 表 `mcp_permissions`：
```sql
CREATE TABLE IF NOT EXISTS mcp_permissions (
  server_id TEXT NOT NULL,
  tool_name TEXT NOT NULL,
  always_allow INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  PRIMARY KEY (server_id, tool_name)
);
```

## 3. 流式 Tool Calls 缓冲拼装 (Buffer Interceptor)
大模型在 SSE 流式输出时，工具参数是以 Token 碎片返回的。
- 在 `ChatProvider` 的流解析管道中新增一个状态机缓冲区。
- 当检测到进入 `tool_calls` 生成阶段时，静默收集所有 Chunk，不向下游 Emit 文本。
- 直到收到结束标志（如 `finish_reason="tool_calls"`），将缓冲区内容校验并反序列化为完整的 JSON 对象。

## 4. Tool Calling 授权与执行流水线 (Multi-turn Loop)
1. **抛出事件**: 后端拼装完完整的 `tool_calls` 后，向前端 Emit 特殊事件：`ChatStreamEvent::ToolCallRequested { tool_name, args }`。
2. **权限拦截**:
   - 后端检查 `mcp_permissions` 是否包含 `always_allow=1` 的记录。
   - 若有，直接进入执行阶段。
   - 若无，后端通过 `tokio::sync::oneshot` 挂起当前对话的异步任务，等待用户决策。同时监听前端取消事件或超时，防止死锁。
3. **前端交互**: 前端渲染一条特殊气泡：“mcp calling: [tool_name]”，并显示授权卡片（Approve / Reject / Always Allow）。用户点击后，调用 Tauri Command `mcp_approve_tool_call` 唤醒后端。
4. **执行 Tool (带边界防御)**: 
   - 授权通过，后端调用对应的 `McpClient::call_tool`。
   - **执行超时控制**：必须包裹在 `tokio::time::timeout(Duration::from_secs(30))` 中，防止外部脚本死循环。
5. **结果截断与重入**: 
   - 获取到 `result_json` 后，进行**最大长度限制（20000 字符）**校验。超出部分截断并追加 `...[Truncated]`，防止撑爆大模型上下文窗口。
   - 将截断后的结果拼入对话历史（作为 `tool` 角色消息），再次向大模型发起流式请求，继续生成最终回复。
