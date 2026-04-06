# Design: desktop-runtime-core

- UI 仅通过 Tauri command 调用宿主。
- Rust 宿主管理 Python Agent 生命周期。
- 通信采用结构化 JSON 消息，统一 `ok/error/meta` 返回。
- 启动时初始化 `~/.dragon-li/{data,memory,config,logs,backups}`。
