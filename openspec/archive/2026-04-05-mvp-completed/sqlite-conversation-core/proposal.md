# Proposal: sqlite-conversation-core

## Why

会话与日志主存储是对话系统核心基础，需先稳定。

## What

- 建立 SQLite 表结构（sessions/messages/request_logs/memory_candidates/index）。
- 落地软删除模型（`deleted_at`）。
- 明确禁外键与代码层关联维护。

## Dependencies

- `desktop-runtime-core`
