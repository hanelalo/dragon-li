# Design: sqlite-conversation-core

- DB: `~/.dragon-li/data/dragon_li.db`，WAL。
- Repository 层封装 CRUD 与默认过滤 `deleted_at IS NULL`。
- 会话删除/恢复时显式级联处理关联数据。
