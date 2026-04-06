# Proposal: memory-pipeline-core

## Why

候选提取、Markdown 持久化、关键词索引是单一事务链路，需整体交付。

## What

- 回复后提取候选记忆。
- 人工审核通过后写入 Markdown。
- 同步维护 `memory_index_docs/terms/stats`。

## Dependencies

- `sqlite-conversation-core`
- `chat-provider-core`
