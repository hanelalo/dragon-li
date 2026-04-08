# Tasks: change-21-search-status-stream

- [ ] **模型层**：在 Python 和 Rust 中新增 `tool_call` 类型的 Stream 事件。
- [ ] **Python 端**：在 `chat_stream_generator` 执行工具的前后，分别 yield `started` 和 `finished` 事件。
- [ ] **前端数据流**：解析该事件，更新对应 Message 的局部状态。
- [ ] **前端 UI**：在对话气泡组件中添加 Loading 状态的展示。