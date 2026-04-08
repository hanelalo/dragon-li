# Design: change-22-search-history-persistence

## 1. 记录存储逻辑
为了简单起见，可以要求 Python 端在整个对话 Stream 结束前，发一个专门的事件把本次对话中执行过的所有 tool calls 详情带回给 Rust。或者，Rust 通过监听 `tool_call` 事件并在最后组装。
更简单的方案是，Python 返回 `done` 事件时，把额外的元数据（如 `tool_invocations`）带上。
**注意**：在扩展 `done` 事件时，Rust 端 (`src-tauri/src/chat_provider.rs`) `ChatStreamEvent::Done` 当前是一个无参变体，并且标记了 `#[serde(deny_unknown_fields)]`。需要将它修改为 `Done { tool_invocations: Option<Vec<...>> }`，否则 Python 返回多余字段会导致 Rust 反序列化报错并中断流。

Rust 收到后，将其写入 `capability_invocations` 表：
- `capability_id`: "web_search"
- `session_id`: 当前会话 ID
- `message_id`: 助手的回复 ID
- `input_payload`: 搜索词
- `output_payload`: 清洗后的搜索结果

## 2. 历史记录读取
目前 `messages` 表只存用户和助手的消息。这意味着只要我们不把 `role: tool` 强行写入 `messages` 表，多轮对话读取时自然就不会带有冗余 Token。
我们需要确保 Rust 在调用 `sqlite_store::save_message` 时，只保存最终的文字回答，而不是带有工具调用的中间态。

## 3. LLM API 校验规则适配（重点）
由于我们在上下文构建时丢弃了 `tool` 消息，必须**净化（降级）**大模型的 `assistant` 消息。
如果上一轮的 `assistant` 消息包含 `tool_calls`，而在随后的 `history` 中不提供对应的 `tool` 消息，OpenAI 等接口会抛出 400 错误。
因此，在 Rust 从 `messages` 表读取历史，或者在 Python 接收到 `history` 构建 Context 时，如果判断到这是作为普通对话历史发送给模型的 `assistant` 消息，必须将其 `tool_calls` 字段剔除，将其降级为一条只包含文本的纯 `assistant` 消息。