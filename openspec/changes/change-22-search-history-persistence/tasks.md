# Tasks: change-22-search-history-persistence

- [ ] **数据传输设计**：确定如何将 Python 中的 tool_call 数据传递给 Rust 端。修改 Python 的 `ChatStreamEventDone` 和 Rust 的 `ChatStreamEvent::Done` 变体，增加 `tool_invocations` 字段。
- [ ] **Rust 存储层**：实现将提取到的 tool 数据插入到 `capability_invocations` 表的逻辑。
- [ ] **历史上下文验证与净化**：检查 `session.rs` 中获取历史记录的逻辑，或者在 Python 构建 Context 的逻辑中，**必须**将 `assistant` 消息中的 `tool_calls` 剔除，降级为纯文本消息，避免因缺少对应的 `tool` 消息触发大模型 API 的 400 校验错误。
- [ ] **端到端测试**：通过数据库查看工具调用记录是否正确落盘。进行多轮对话测试，确认不会触发 API 校验报错。