# Proposal: change-06-chat-provider-structured-output

## Why
目前 `chat_provider.rs` 的核心实现 `ChatAdapter` 只支持面向用户的 SSE 流式对话（Streaming Text）。
但在未来的业务场景中（如自动记忆提取、MCP/Tool Calling），后端需要向 LLM 发起同步的非流式请求，并严格要求其返回结构化的 JSON 数据（Structured Output），以供 Rust 后端反序列化和业务处理。

## What
- 在 `ChatAdapter` Trait 中新增 `chat_completion_json`（或类似方法）的非流式接口。
- 针对 OpenAI，支持传入 `response_format: { type: "json_object" }` 以强制 JSON 输出。
- 针对 Anthropic，通过 Prompt 约束并解析非流式的 JSON 响应。
- 统一定义并封装返回结果的错误处理（如 JSON 解析失败、模型拒绝等）。

## Dependencies
- `chat-provider-core` (MVP)