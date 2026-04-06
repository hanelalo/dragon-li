# 03. 会话与记忆设计

## 会话能力

- 多会话管理：新建、切换、归档、搜索。
- 对话流程：发送消息、流式回复、失败重试。
- 上下文面板：展示本轮注入记忆和摘要信息。
- 会话结构：MVP 采用线性会话，预留 `parent_message_id` 支持后续分支。

## 记忆模型

- 短期记忆：会话内摘要，控制上下文长度。
- 长期记忆：跨会话事实与偏好，支持人工审核写入。
- 记忆条目载体：Markdown（含 frontmatter 元数据）。

## 记忆写入与冲突

- 写入策略：以人工确认为主。
- 冲突策略：不直接覆盖，保留版本并标记冲突。
- 注入策略：每轮限制注入 5 条，按相关性与置信度筛选。

## 记忆提取规则（MVP）

- 触发时机：每次 assistant 回复完成后触发；支持手动触发。
- 可提取类型：`fact`、`preference`、`constraint`、`project`、`task`。
- 不提取内容：一次性闲聊、低确定性猜测、无任务价值敏感信息。
- 候选门槛：明确陈述或跨两轮重复之一成立。
- 置信度：
  - `0.8-1.0` 明确且可追溯
  - `0.6-0.79` 语义清晰但单轮推断
  - `<0.6` 默认不入长期记忆

## 检索与排序（关键词首版）

- 索引对象：仅长期记忆。
- 字段权重：结论/summary、tags 高；type 中；evidence 中低。
- 排序：关键词相关性 + 置信度 + 新鲜度。
- 过滤：`type`、`tags`、`status`、`min_confidence`。
- 去重：同 ID 留新版本；高相似条目留高分。
- 扩展：通过统一 `Indexer` 接口预留向量检索。

## 数据目录规范

统一使用 `~/.dragon-li/`：

- `~/.dragon-li/data/`
- `~/.dragon-li/memory/short_term/`
- `~/.dragon-li/memory/long_term/`
- `~/.dragon-li/config/`
- `~/.dragon-li/logs/`
- `~/.dragon-li/backups/`

补充说明：

- 会话与消息仅存储在 SQLite（`~/.dragon-li/data/dragon_li.db`）。
