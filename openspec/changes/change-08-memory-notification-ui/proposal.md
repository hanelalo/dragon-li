# Proposal: change-08-memory-notification-ui

## Why
在 `change-07` 实现自动后台记忆提取后，前端界面在静默状态下并不知道有新记忆产生。
为了形成良好的闭环用户体验，需要在有新的候选记忆生成时，给前端发送通知（Event Emit），以便 UI 上出现提示（如侧边栏红点、Toast 提示），引导用户前往 Memory Center 进行审核（Review）。

## What
- 后端在成功提取并写入 `pending` 候选记忆后，向前端 `emit` 一个特定的状态更新事件（例如 `memory_candidates_updated`）。
- 前端使用全局状态管理（Vue `ref` 注入在根组件或 Pinia Store）监听该事件。
- 当接收到事件时，更新未审核候选（`pending`）的数量。
- 在应用侧边栏的 "Memory" 导航项上展示一个红点或数字角标（Badge）。
- 可选：在右下角弹出一个轻量级的 Toast 提示。

## Dependencies
- `change-07-memory-auto-extraction-core`