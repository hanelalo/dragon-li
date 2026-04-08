# Tasks: change-16-python-business-logic

## Python 业务端点建设
- [x] 创建 `agent/prompts.py`：将 `src-tauri/src/main.rs` 中硬编码的 "Title Generation" 和 "Auto Memory Extraction" 系统提示词抽离至 Python 端。
- [x] 扩展 `agent/models.py`：新增与业务逻辑对应的输入输出 Pydantic 模型（如 `TitleGenerateRequest`, `TitleGenerateResponse`, `MemoryExtractRequest`, `MemoryExtractResponse`）。
- [x] 在 `agent/runtime_agent.py` 新增专门的 API 路由：
  - `POST /v1/chat/summarize_title`：接收用户消息，调用内部大模型能力生成 1-6 个字的标题。
  - `POST /v1/memory/extract`：接收对话记录与用户/助手的新一轮交互，执行自动记忆提取逻辑并返回结构化的 JSON。

## Rust 宿主层 (Main & ChatProvider) 重构与瘦身
- [x] 修改 `src-tauri/src/chat_provider.rs`，提供通用的向 Python 代理发送 JSON RPC 风格请求的能力（比如一个通用的 `post_uds_json` 方法）。
- [x] 重构 `src-tauri/src/main.rs` 中的 `chat_summarize_title`：
  - 移除庞大的系统提示词，改为构造包含 `profile_id`, `user_text` 等必要参数的请求。
  - 将请求发往 Python Agent 的 `/v1/chat/summarize_title` 接口，并解析返回结果直接更新 UI 或返回给前端。
- [x] 重构 `src-tauri/src/main.rs` 中的后台任务（自动提取记忆）：
  - 移除所有的提示词与 JSON Schema 约束。
  - 构造仅包含 `session_id`, `user_text`, `assistant_text`, `history` 的干净请求，发送到 Python Agent 的 `/v1/memory/extract` 接口。
  - 将提取的结果直接传递给 `memory_pipeline.save_extracted_candidates` 进行 SQLite 落库并发出事件。

## 验收清单
- [x] `main.rs` 中不再包含任何 "You are a strict title generator" 或 "You are an intelligent memory extraction engine" 等硬编码提示词。
- [x] Python 代理能够独立完成标题生成与记忆提取，Rust 仅负责数据持久化和状态分发。
- [x] 测试发起一段新对话，应用依然能够迅速生成精准的标题（在左侧会话列表中更新），且后台能成功提取记忆并推送到 "Memory Candidates" 徽标。