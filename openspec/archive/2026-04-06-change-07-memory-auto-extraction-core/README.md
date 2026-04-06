# change-07-memory-auto-extraction-core

## Status
- **Completed on**: 2026-04-06
- **Components**: `memory_pipeline.rs`, `sqlite_store.rs`, `main.rs`

## Summary
实现了基于大模型的对话记忆后台自动提取机制：
1. **数据层**: 在 `memory_pipeline.rs` 定义了强类型的 `AutoExtractedMemory` 结构，并增加了带去重逻辑的 `save_extracted_candidates` 存储方法。
2. **Context 构建**: 在 `sqlite_store.rs` 新增了 `get_latest_dialogue_pair` 获取指定 Session 下最后一次对话对。
3. **后台任务**: 在 `main.rs` 的 `chat_send` 中，当回复写入库后，派发了 `tauri::async_runtime::spawn_blocking` 后台线程。
4. **LLM 集成**: 组合 Prompt 要求模型强制返回结构化的 JSON 数据数组，如果提取到新内容，则交由存储层入库。
5. **日志覆盖**: 在整个异步流程中补充了充足的 `info!` 和 `error!` 日志，用来追踪状态或排查提取失效原因。
6. **测试**: 补充了 `save_extracted_candidates_works_and_deduplicates` 测试。