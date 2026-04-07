# Proposal: change-16-python-business-logic

## 核心目标
将所有涉及大模型交互（LLM Interaction）与提示词（Prompt）构建的业务逻辑从 Rust 宿主层彻底抽离，下沉到 Python Agent 中。实现“Rust 负责调度与存储，Python 负责智能处理”的清晰架构边界。

## 背景与痛点
在 `change-14` 的迁移过程中，虽然底层 HTTP 传输协议已经改由 Python UDS 代理接管，但如“自动总结对话标题”、“从历史对话中提取长效记忆”等高级业务逻辑的 Prompt 模板与组装过程仍然硬编码在 `src-tauri/src/main.rs` 中。这违背了将智能体（Agent）逻辑集中于 Python 的初衷，导致 Rust 代码臃肿且难以维护大模型交互细节。

## 解决方案
1. **Python 端业务路由扩展**：在 `agent/runtime_agent.py` 中新增专门的业务 API 端点：
   - `POST /v1/chat/summarize_title`：接收用户首条消息，返回精简标题。
   - `POST /v1/memory/extract`：接收对话上下文，返回结构化的记忆候选列表。
2. **Prompt 迁移与集中管理**：将 `main.rs` 中硬编码的 System Prompt 移动到 Python 端的专用模块（如 `agent/prompts.py`）中。
3. **Rust 宿主瘦身**：修改 `src-tauri/src/main.rs` 中的 `chat_summarize_title` 和自动记忆提取的后台任务，移除所有提示词组装逻辑，改为向 Python Agent 发起简洁、语义化的 JSON 请求，仅负责接收处理结果并落库（SQLite）和推送 UI 事件。

## 影响范围
- `agent/runtime_agent.py` (新增 API 路由)
- `agent/models.py` (新增业务数据模型)
- `agent/prompts.py` (新增，集中管理 Prompt)
- `apps/desktop/src-tauri/src/main.rs` (移除硬编码 Prompt，简化请求)
- `apps/desktop/src-tauri/src/chat_provider.rs` (增加发送 JSON 请求到新路由的能力)