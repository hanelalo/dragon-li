# Tasks: change-14-python-llm-migration

## 数据结构对齐
- [x] 在 Python 端创建 `agent/models.py`，使用 Pydantic 严格对齐 Rust `chat_provider.rs` 中的输入结构：
  - `ChatPromptLayer` (system, runtime, memory, user)
  - `ChatMessageContext` (role, content)
  - `ChatRequestInput` (profile_id, request_id, session_id, model, prompt, history)
- [x] 在 `agent/models.py` 中对齐 Rust 端的流式输出事件 `ChatStreamEvent`：
  - 构造支持 `delta`, `reasoning`, `usage`, `done`, `aborted` 这 5 种类型的 Pydantic 模型。
  - 确保序列化后的 JSON 与 Rust 端的 `{ "type": "delta", "payload": { "text": "..." } }` 格式完全一致。

## Python 核心交互逻辑开发
- [x] 在 `agent/requirements.txt` 添加 `pydantic`, `openai`, `anthropic`。
- [x] 创建 `agent/llm_provider.py`，实现一个基于 `profile_id` 区分模型提供商的请求客户端。
- [x] 在 FastAPI 中实现 `POST /v1/chat/stream` 路由：
  - 接收并验证 `ChatRequestInput`。
  - 从环境变量或 HTTP 请求头获取对应的 API Key。
  - 组装系统提示词、记忆、运行时上下文、用户输入和历史消息。
  - 请求大模型，并在内部包装一个生成器（Generator），实时地将大模型返回的每个 token 包装为 `ChatStreamEvent`。
  - 使用 FastAPI 的 `StreamingResponse` 返回标准的 `text/event-stream` SSE 格式。

## Rust 宿主层 (ChatService) 退化为透明代理
- [x] 修改 Rust 端 `chat_provider.rs` 中的 `ChatService::chat_with_retry_stream`：
  - 删除内部原有的 `OpenAiAdapter` 和 `AnthropicAdapter` 的具体实现逻辑。
  - 构造包含所有历史记录的 `ChatRequestInput`，将其序列化为 JSON。
  - 注入 API Key 等关键配置。
  - 向 UDS Socket 发起 POST 请求（基于 Unix Domain Socket）。
- [x] 在 Rust 中处理 Python 返回的 SSE 流：
  - 逐行读取 `data: `，将其反序列化为 `ChatStreamEvent`。
  - 直接触发现有的 `emit_stream` 回调，将事件无缝推送给 Vue 前端，同时保证原来的错误重试和自动落库逻辑依然有效。

## 验收清单
- [x] 在 Rust 移除原始的 OpenAI 适配器代码后，项目能通过 `cargo check` 编译。
- [x] 打开前端进行提问，能够通过 Python Agent 的中转顺畅地看到流式的文字逐个输出。
- [x] 控制台能够看到请求已发送到 Python 进程，而不是直接发给 api.openai.com。