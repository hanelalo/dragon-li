# Proposal: change-22-search-history-persistence

## 核心目标
将网络搜索的工具调用记录持久化到 `capability_invocations` 表中，并在构建多轮对话历史时剔除冗长的工具返回结果，以节省 Token 并保持数据库整洁。

## 背景与痛点
当前工具调用只在内存中流转，未落库，不便于后续的回溯和 Debug。同时，如果多轮对话中直接把带有长篇搜索结果的 `tool` 消息传给大模型，会导致 Token 迅速耗尽。

## 解决方案
1. **状态落库**：Rust 端拦截或在请求结束后统一保存，将工具的入参和出参记录写入现有的 `capability_invocations` 表，关联 `message_id`。
2. **上下文瘦身**：构建下一轮对话历史时，只提取 `messages` 表中的 `user` 和 `assistant` 消息，不拼接原始的 `tool` 消息（因为最终生成的 `assistant` 消息已经包含了摘要后的事实）。

## 验收标准
- 发生搜索后，SQLite 数据库的 `capability_invocations` 表有对应记录。
- 进行多轮对话，抓包或查看日志确认传给 Python Agent 的 `history` 数组中，不包含冗余的搜索原始 JSON。