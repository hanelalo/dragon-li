# Proposal: change-17-rust-main-refactor

## 核心目标
重构并拆分 `src-tauri/src/main.rs`。将超过 1000 行的臃肿文件按领域逻辑（Domain Logic）拆分为多个独立的命令模块，使 `main.rs` 回归其作为应用程序入口（Entry Point）的本质，提升 Rust 宿主层的可维护性。

## 背景与痛点
虽然在 `change-16` 中我们成功将 LLM 的提示词和核心业务逻辑下沉到了 Python，但 `main.rs` 仍然包含了大量用于响应前端调用的 Tauri Command。目前，所有的配置管理（Config）、本地数据库操作（SQLite Session/Message）、记忆管理（Memory Pipeline）和聊天转发（Chat Provider）都被堆砌在同一个文件中。这导致：
1. 文件过长（超过 1000 行），阅读和维护极其困难。
2. 职责不清晰，Tauri Command 与底层工具函数混杂。
3. 团队协作时容易产生 Merge Conflict。

## 解决方案
1. **创建 `commands` 目录**：在 `src-tauri/src/` 下新建 `commands` 模块，用于分类存放所有的 Tauri Command。
2. **按领域拆分 Command**：
   - `commands/config.rs`: 负责配置的读取、保存、检查外部变更。
   - `commands/session.rs`: 负责会话的创建、列表查询、消息流读取、删除等。
   - `commands/memory.rs`: 负责记忆候选的提取、审核、列表查询、软删除、搜索等。
   - `commands/chat.rs`: 负责与大模型相关的请求（对话生成、标题总结）。
   - `commands/agent.rs`: 负责后台 Python Agent 进程的管理（启动、状态检查）。
3. **公共工具与错误处理**：将 `error_to_parts`, `map_store_error`, `map_memory_error`, `iso_now` 等共享的辅助函数提取到 `utils.rs` 或相关的基础模块中。
4. **`main.rs` 瘦身**：将 `main.rs` 精简到只包含 `main()` 函数、`AppState` 声明、以及 `tauri::Builder` 的配置和命令注册逻辑。

## 影响范围
- `apps/desktop/src-tauri/src/main.rs` (大规模重构)
- `apps/desktop/src-tauri/src/commands/mod.rs` (新增)
- `apps/desktop/src-tauri/src/commands/config.rs` (新增)
- `apps/desktop/src-tauri/src/commands/session.rs` (新增)
- `apps/desktop/src-tauri/src/commands/memory.rs` (新增)
- `apps/desktop/src-tauri/src/commands/chat.rs` (新增)
- `apps/desktop/src-tauri/src/commands/agent.rs` (新增)
- `apps/desktop/src-tauri/src/utils.rs` (新增，用于存放共享的工具函数)