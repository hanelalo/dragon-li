# 05. 数据存储与 Schema（SQLite + Markdown）

## 总体策略

- 对话、候选、日志、索引数据：SQLite。
- 长短期记忆正文：Markdown。
- 禁止使用数据库外键，关联一致性由代码层保证。

## SQLite

- 文件路径：`~/.dragon-li/data/dragon_li.db`
- 模式：启用 WAL

### 表：sessions

- `id` (TEXT PK)
- `title` (TEXT)
- `status` (TEXT: active/archived)
- `default_provider` (TEXT)
- `default_model` (TEXT)
- `created_at` (TEXT)
- `updated_at` (TEXT)
- `deleted_at` (TEXT, NULL) 软删除标记

### 表：messages

- `id` (TEXT PK)
- `session_id` (TEXT)
- `role` (TEXT)
- `content_md` (TEXT)
- `provider` (TEXT)
- `model` (TEXT)
- `tokens_in` (INTEGER)
- `tokens_out` (INTEGER)
- `latency_ms` (INTEGER)
- `parent_message_id` (TEXT, NULL)
- `status` (TEXT: streaming/ok/failed)
- `error_code` (TEXT, NULL)
- `error_message` (TEXT, NULL)
- `retryable` (INTEGER, NULL)
- `created_at` (TEXT)
- `deleted_at` (TEXT, NULL) 软删除标记

### 表：memory_candidates

- `id` (TEXT PK)
- `session_id` (TEXT)
- `source_message_id` (TEXT)
- `type` (TEXT)
- `summary` (TEXT)
- `evidence` (TEXT)
- `confidence` (REAL)
- `tags_json` (TEXT)
- `status` (TEXT: pending/approved/rejected/conflicted)
- `created_at` (TEXT)
- `updated_at` (TEXT)
- `deleted_at` (TEXT, NULL) 软删除标记

### 表：request_logs

- `id` (TEXT PK)
- `request_id` (TEXT)
- `session_id` (TEXT)
- `provider` (TEXT)
- `model` (TEXT)
- `status` (TEXT)
- `latency_ms` (INTEGER)
- `tokens_in` (INTEGER)
- `tokens_out` (INTEGER)
- `error_code` (TEXT, NULL)
- `created_at` (TEXT)

### 表：memory_index_docs（关键词索引文档表）

- `memory_id` (TEXT PK)
- `type` (TEXT)
- `tags_json` (TEXT)
- `confidence` (REAL)
- `updated_at` (TEXT)
- `deleted_at` (TEXT, NULL)

### 表：memory_index_terms（倒排词项表）

- `term` (TEXT)
- `memory_id` (TEXT)
- `field` (TEXT: summary/tags/type/evidence)
- `tf` (REAL) 词频或归一化词频
- `weight` (REAL) 字段权重
- `updated_at` (TEXT)
- 复合主键：`(term, memory_id, field)`

### 表：memory_index_stats（词项统计）

- `term` (TEXT PK)
- `doc_freq` (INTEGER)
- `updated_at` (TEXT)

## Markdown 记忆目录

- `~/.dragon-li/memory/short_term/`
- `~/.dragon-li/memory/long_term/`

## 对话内容存储位置（最终）

- 对话消息以 SQLite `messages` 表为准。
- Markdown 主要用于记忆资产，不再作为对话主存储。

## 软删除与关联删除（代码层）

- 用户触发删除操作时，默认执行软删除：写入 `deleted_at`，不做物理删除。
- 数据读取默认过滤 `deleted_at IS NULL`。
- 因禁止外键，级联逻辑必须在代码中显式处理：
  - 软删除 `sessions` 时，同步软删除该 `session_id` 下 `messages` 与 `memory_candidates`。
  - 恢复 `sessions` 时，默认同步恢复该 `session_id` 下 `messages` 与 `memory_candidates`。
- 删除与恢复记忆时，必须同步维护 `memory_index_terms`（增删对应 posting）。

## 关键词索引设计与原因

设计：

- 不使用 SQLite FTS5，采用自维护倒排索引三表：`memory_index_docs`、`memory_index_terms`、`memory_index_stats`。
- 分词、停用词、中文切词与英文归一化在代码层完成，再写入索引表。
- 查询时按 term 召回 posting list，在代码层计算综合分：
  - 文本相关性（TF-IDF/BM25-lite）
  - 置信度加权
  - 新鲜度加权

原因：

- 可控性高：可直接按 `summary/tags/type/evidence` 设置字段权重。
- 中英文统一：避免 FTS tokenizer 对中文支持不足带来的不一致。
- 可扩展：后续接入向量检索时，可并存为混合召回，不破坏当前接口。
- 与“禁用外键、代码治理一致性”原则一致，索引更新可由统一服务层事务化控制。

## 参考 DDL

- 见 `/Users/hanelalo/develop/dragon-li/docs/08-sqlite-ddl.md`
