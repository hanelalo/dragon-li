# Tasks: change-07-memory-auto-extraction-core

- [ ] 定义用于接收模型结构化输出的 Rust 数据结构（`AutoExtractedMemory`）。
- [ ] 编写一个用于记忆提取的 `System Prompt`，明确输出格式。
- [ ] 在 `memory_pipeline.rs` 新增 `save_extracted_candidates` 方法。
- [ ] 在 `main.rs` 的 `chat_send` 成功完成流式响应落库后，触发后台异步调用 `chat_with_retry_json` 并在成功后调用 `save_extracted_candidates`。
- [ ] 在 `MemoryPage.vue` 测试并验证新功能。

## 验收清单
- [ ] 与大模型正常聊天后，控制台出现提取记忆的请求日志。
- [ ] 去 Memory 页面能看到刚才聊天产生的新记忆卡片。
- [ ] 卡片字段（summary, type, tags, evidence）完整且符合语义。