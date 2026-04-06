# Proposal: desktop-runtime-core

## Why

建立桌面运行时地基，统一 UI、宿主与 Agent 的进程通信模型。

## What

- 初始化 Tauri + Vue + Python Agent 基础工程。
- 定义 Rust <-> Python IPC 协议骨架（命令/事件）。
- 初始化 `~/.dragon-li/` 目录结构。

## Scope

### In

- 进程启动、健康检查、退出清理。
- 基础 IPC ping/pong 与错误包装。

### Out

- 业务能力（会话、记忆、Provider）。

## Dependencies

- 无（起始 change）。
