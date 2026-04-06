# Proposal: change-07-memory-auto-extraction-core

## Why
MVP 阶段的记忆提取是依靠本地 TF-IDF 分词与固定关键词匹配（启发式规则）来完成的，且必须由用户手动触发。
随着 `change-06` 结构化输出能力的补齐，系统应该能够在每次对话结束后，利用后台异步任务，自动将历史对话发送给大模型，让大模型分析并输出高质量的长期记忆候选。

## What
- 移除（或保留作为降级备用）基于纯文本规则的 `extract_candidates` 逻辑。
- 在 `memory_pipeline.rs` 新增 `save_extracted_candidates` 方法，用于保存 LLM 提取后的结构化数据。
- 当 `chat_send` 的主流程收到 `ChatStreamEvent::Done` 并且保存完消息后，派生一个后台任务 (`tokio::spawn`) 去调用大模型提取逻辑。
- 提取的结果转为 `MemoryCandidate` 存入 SQLite 的 `memory_candidates` 表中。

## Dependencies
- `change-06-chat-provider-structured-output`