# Proposal: change-14-python-llm-migration

## 核心问题
目前 Rust 和 OpenAI/Anthropic 的 API 交互以及流式数据解析全部在 `chat_provider.rs` 中进行。这意味着如果未来大模型在聊天过程中下发了 `tool_calls` 指令，我们将无法方便地调度 Python 生态中的 MCP SDK 和第三方工具执行代码。

## 解决方案
本 Change 的核心目标是**把大语言模型的通信与流式处理“搬家”到 Python**，但同时不能丢失已有的真流式打字机体验。
1. **Python 端 (`agent/models.py`, `llm_provider.py`)**：引入 Pydantic，严格对齐 Rust 的 `ChatRequestInput` 和 `ChatStreamEvent`。引入 `openai` 和 `anthropic` 库，提供一个 `/v1/chat/stream` 路由处理流式返回。
2. **Rust 端 (`chat_provider.rs`)**：删除内部的 API 适配器。Rust 现在只作为一个“透明代理”，把前端的请求组装成 JSON，通过 `reqwest` 发给本地的 Python `/v1/chat/stream` 端口，并将读到的 SSE 数据原封不动推给 Tauri 渲染进程。

## 影响范围
- `agent/models.py` (新增)
- `agent/llm_provider.py` (新增)
- `agent/runtime_agent.py` (新增 `/v1/chat/stream` 路由)
- `apps/desktop/src-tauri/src/chat_provider.rs` (大面积删减，重构请求对象)