# System Architecture Specification

**Status**: Active (MVP Phase)
**Last Updated**: 2026-04-06

## 1. Overview
Dragon Li 是一款基于本地优先（Local-First）架构构建的桌面 AI 助手应用。它利用 Tauri 将系统级的高性能 Rust 后端与现代化的 Vue 3 组合式 API 前端相结合。系统的核心目标是提供低延迟的聊天流式体验、可靠的本地数据持久化，以及基于本地文件系统和轻量级 SQLite 的长效记忆流水线。

## 2. Tech Stack
- **Desktop Runtime**: Tauri v2
- **Backend (Core System)**: Rust (Tokio, Serde, Reqwest)
- **Frontend (UI/UX)**: Vue 3 (Composition API), Vite, Tailwind/CSS
- **Storage Layer**: SQLite (Rusqlite), Local File System (`~/.dragon-li/`)

## 3. Core Components

### 3.1 App Shell & Global State
- 前端通过 Vue 组合式函数 (`useStore`, `ref`, `computed`) 维护全局状态。
- Tauri 后端通过 `AppState` 结构体（由 `Mutex` 或 `RwLock` 包装）管理全局数据库连接 (`sqlite_store`)、配置服务以及运行时的聊天客户端等。

### 3.2 Storage Layer (SQLite + FS)
- 所有的配置信息、聊天记录（Sessions/Messages）、API 请求日志（Request Logs）均保存在 `~/.dragon-li/db/dragon_li.db`。
- **文件系统**：长效记忆作为具体的实体 Markdown 文件保存在 `~/.dragon-li/memory/long_term/` 中，便于用户直观管理或被第三方工具索引。

### 3.3 Communication Bridge
- 前后端通过 Tauri 的 IPC (Inter-Process Communication) 进行通信：
  - **Commands (Invoke)**: 前端主动调用后端的同步/异步任务（如 `chat_send`, `memory_extract`）。
  - **Events (Emit)**: 后端主动向前端推送流式数据和状态变更（如 `chat_stream_event`，下发 Delta、Usage 统计及完成信号）。