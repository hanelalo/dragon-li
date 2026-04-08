# Tasks: change-19-search-ui-toggle

- [x] **模型层**：在 Rust 和 Python 的 `ChatRequestInput` 结构体中新增 `enable_web_search` 字段。
- [x] **UI 层**：修改 `Composer.vue`，加入网络图标 Toggle 按钮及点击拦截逻辑。
- [x] **数据流**：修改 `Composer.vue` 的 `send` emit 格式，带上状态；修改 `ChatPage.vue` 解析该状态并放入 `chat_send` payload。
- [x] **联调验证**：发送消息时，确认 Python 端打印的 request 包含正确的 `enable_web_search` 值。