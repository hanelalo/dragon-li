# Tasks: change-00-mvp-consistency-and-streaming-fixes

- [x] 改造 `chat_provider` 传输链路，支持 SSE 增量解析并即时发射 `delta` 事件。
- [x] 保持 `chat_send` 结果结构兼容，同时修正事件发送时机为实时发送。
- [x] 补充流式行为验证（至少覆盖 OpenAI/Anthropic 的增量事件解析）。
- [x] 修订配置变更相关设计文档，统一为“显式检查外部变更”口径。
- [x] 修订软删除策略文案，统一为“用户触发删除时默认软删除”。
- [x] 在 SQLite 错误映射中落地 `DB_BUSY`（busy/locked 场景）。
- [x] 在关键数据库写入路径加入 `DB_BUSY` 的有限重试（2 次，500ms->1500ms）。
- [x] 增加 `DB_BUSY` 对应测试或可复现验证脚本。

## 验收清单

- [x] UI 端可观察到真实流式增量输出。
- [x] 配置变更检测文档与代码实现无冲突描述。
- [x] 软删除策略文档无歧义且口径一致。
- [x] busy 场景返回 `DB_BUSY` 并按预期重试。
