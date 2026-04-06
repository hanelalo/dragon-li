# Tasks: change-08-memory-notification-ui

- [ ] 后端在 `main.rs` 的自动提取异步任务执行结束后，查询未处理的记忆候选总数，通过 `app_handle.emit` 向前端发通知。
- [ ] 编写一个用于查询未审核数量的 `memory_count_pending` Tauri Command，并在 `main.rs` 注册，供前端初始加载使用。
- [ ] 在前端构建全局状态（Vue `ref` / composables）以接收 `memory_candidates_updated` 事件。
- [ ] 在左侧边栏图标上添加红点或数字角标。
- [ ] 测试后台提取完成时前端的响应，并测试用户点击 Approve/Reject 后，数字角标的自动递减刷新逻辑。

## 验收清单
- [ ] 启动应用时，如果有待审核的记忆，侧边栏能够立即显示数量。
- [ ] 聊天结束后，不需要手动刷新，侧边栏角标数字能自动增加。
- [ ] 进入 Memory Center 并审核后，角标数字实时减少。