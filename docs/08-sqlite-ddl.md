# 08. SQLite 初始化 DDL

数据库文件路径：

- `~/.dragon-li/data/dragon_li.db`

初始化 SQL：

```sql
PRAGMA journal_mode = WAL;

CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
  default_provider TEXT,
  default_model TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
  content_md TEXT NOT NULL,
  provider TEXT,
  model TEXT,
  tokens_in INTEGER,
  tokens_out INTEGER,
  latency_ms INTEGER,
  parent_message_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('streaming', 'ok', 'failed')),
  error_code TEXT,
  error_message TEXT,
  retryable INTEGER CHECK (retryable IN (0, 1)),
  created_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS memory_candidates (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  source_message_id TEXT NOT NULL,
  type TEXT NOT NULL CHECK (type IN ('fact', 'preference', 'constraint', 'project', 'task')),
  summary TEXT NOT NULL,
  evidence TEXT,
  confidence REAL NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
  tags_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'rejected', 'conflicted')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS request_logs (
  id TEXT PRIMARY KEY,
  request_id TEXT NOT NULL,
  session_id TEXT,
  provider TEXT,
  model TEXT,
  status TEXT NOT NULL,
  latency_ms INTEGER,
  tokens_in INTEGER,
  tokens_out INTEGER,
  error_code TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS memory_index_docs (
  memory_id TEXT PRIMARY KEY,
  type TEXT NOT NULL,
  tags_json TEXT NOT NULL,
  confidence REAL NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS memory_index_terms (
  term TEXT NOT NULL,
  memory_id TEXT NOT NULL,
  field TEXT NOT NULL CHECK (field IN ('summary', 'tags', 'type', 'evidence')),
  tf REAL NOT NULL,
  weight REAL NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (term, memory_id, field)
);

CREATE TABLE IF NOT EXISTS memory_index_stats (
  term TEXT PRIMARY KEY,
  doc_freq INTEGER NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_updated_at
  ON sessions(updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_messages_session_created_at
  ON messages(session_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_memory_candidates_session_status
  ON memory_candidates(session_id, status);

CREATE INDEX IF NOT EXISTS idx_request_logs_created_at
  ON request_logs(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_request_logs_request_id
  ON request_logs(request_id);

CREATE INDEX IF NOT EXISTS idx_sessions_not_deleted
  ON sessions(deleted_at);

CREATE INDEX IF NOT EXISTS idx_messages_not_deleted
  ON messages(session_id, deleted_at, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_memory_candidates_not_deleted
  ON memory_candidates(session_id, status, deleted_at);

CREATE INDEX IF NOT EXISTS idx_memory_terms_term
  ON memory_index_terms(term);

CREATE INDEX IF NOT EXISTS idx_memory_terms_memory
  ON memory_index_terms(memory_id);
```

说明：

- 时间字段统一使用 ISO 8601 字符串（含时区）。
- `messages.parent_message_id` 为后续分支会话预留字段。
- 禁止使用数据库外键，关联一致性与级联软删除由代码层保证。
