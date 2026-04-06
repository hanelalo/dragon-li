# Chat Engine Specification

**Status**: Active (MVP Phase)
**Last Updated**: 2026-04-06

## 1. Overview
Chat Engine 是 Dragon Li 的核心对话运行时，主要负责与第三方 LLM 提供商（目前支持 OpenAI, Anthropic）的流式通信、协议抽象、会话状态管理以及元信息（标题生成、Token/耗时统计）的记录。

## 2. Core Capabilities

### 2.1 Stream Processing (SSE)
- 后端采用 `reqwest` 进行非阻塞的 HTTP 流式请求，通过 `chat_provider.rs` 中的 `parse_stream_line` 方法实时解析不同供应商（OpenAI/Anthropic）的 SSE（Server-Sent Events）格式。
- 将解析到的数据抽象为统一的内部枚举类型 `ChatStreamEvent`：
  - `Delta { text: String }`: 普通聊天文本块
  - `Reasoning { text: String }`: 深度思考模型的中间推理过程
  - `Usage { tokens_in, tokens_out }`: Token 消耗量统计
  - `Done`: 会话结束信号
  - `Aborted`: 错误与重试信号

### 2.2 Event Forwarding (Tauri Emit)
- 后端将 `ChatStreamEvent` 序列化为 JSON 载荷，通过 `app_handle.emit("chat_stream_event", ...)` 实时推送到前端。
- 在 `Done` 阶段，后端会汇总统筹计算网络耗时（`latency_ms`）及 Token 消耗（`tokens_in/tokens_out`），下发最终完成状态。前端接收后在消息气泡底部渲染这些统计信息。

### 2.3 Auto-Title Generation
- 用户在当前会话（Session）发送第一条消息时，系统会**自动触发**后台生成一个简短标题。
- **并发机制**: 为防止阻塞 UI 线程（Webview）与主聊天流，标题生成调用被放置在 `tauri::async_runtime::spawn_blocking` 或类似的并发运行时块中执行。
- **Prompt Engineering**: 系统底层提示词（System Prompt）强制约束模型必须输出且仅输出极简标题（1-6字），且**必须与用户输入语言保持严格一致**，拒绝生成多余的会话占位符。

### 2.4 Request Logs & Telemetry
- 每一次调用 `chat_send` 或其它 LLM 请求，都会在 `request_logs` 表中记录一行：包括 `request_id`, `provider`, `model`, `status` (ok/error), `latency_ms`, `tokens_in/out` 等。这些数据有助于后续进行成本统计或错误排查。