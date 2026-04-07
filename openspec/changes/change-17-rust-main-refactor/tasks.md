# Tasks: change-17-rust-main-refactor

## 1. 基础架构重组
- [x] 在 `src-tauri/src/` 下创建 `commands/` 目录和 `commands/mod.rs` 文件，用于导出所有的 Tauri Command。
- [x] 创建 `src-tauri/src/utils.rs`：
  - 将 `main.rs` 中的基础辅助函数提取过来，包括：`iso_now`, `next_log_id`, `map_store_error`, `map_memory_error`, `config_error_parts`。
  - 在 `main.rs` 和其他需要的地方导入这些公共函数。

## 2. 领域拆分 (按文件迁移 Command)
- [x] **Config 模块 (`commands/config.rs`)**
  - 迁移 `config_get`, `config_save`, `config_check_external_change`, `config_apply_external_change`, `config_update_profile`, `config_remove_profile`。
- [x] **Session & Message 模块 (`commands/session.rs`)**
  - 迁移 `session_create`, `session_list`, `session_update_title`, `session_delete`。
  - 迁移 `message_list`, `message_clear_all`。
  - 迁移 `message_append_user`, `message_update_user`, `message_truncate_at`。
- [x] **Memory 模块 (`commands/memory.rs`)**
  - 迁移 `memory_extract_candidates`, `memory_count_pending`, `memory_list_candidates`, `memory_review_candidate`。
  - 迁移 `memory_soft_delete`, `memory_restore`, `memory_read`, `memory_search`, `memory_list_long_term`。
- [x] **Chat 模块 (`commands/chat.rs`)**
  - 迁移核心的 `chat_send`, `chat_summarize_title`，以及相关辅助函数 `create_chat_failure_log`, `resolve_chat_log_context`, `inject_memory_context`。
- [x] **Agent 模块 (`commands/agent.rs`)**
  - 迁移 `start_agent`, `stop_agent`, `status_agent`。

## 3. main.rs 瘦身
- [x] 在 `main.rs` 的头部导入 `commands::*` 中的所有 handler。
- [x] 修改 `tauri::Builder::default().invoke_handler(tauri::generate_handler![...])`：
  - 确保所有的被移动的 Command 都能被正确地传递给 `generate_handler!` 宏。
- [x] 移除 `main.rs` 中被拆分掉的所有业务逻辑和相关的结构体定义（例如：`TitleGenerateRequest`, `MemoryExtractRequest` 等如果只在 `chat.rs` 使用，则也移动过去）。

## 4. 编译与验收
- [x] 执行 `cargo check`，修复可能出现的生命周期、所有权、或者不可见（private/public visibility）的编译报错。
- [x] 执行 `cargo build`。
- [x] 确保前端调用所有的 Command（配置、聊天、会话列表、记忆）依然畅通无阻，功能无任何退化。
- [x] `main.rs` 缩减到 200 行以内，仅保留程序的入口逻辑、状态（`AppState`）注册和 Tauri 系统初始化。