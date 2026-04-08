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

-- Request Logs Table
CREATE TABLE IF NOT EXISTS request_logs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    request_payload TEXT NOT NULL,
    response_payload TEXT NOT NULL,
    status TEXT NOT NULL, -- 'success', 'error'
    error_message TEXT,
    duration_ms INTEGER NOT NULL,
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    total_tokens INTEGER,
    created_at TEXT NOT NULL, -- ISO 8601 string
    updated_at TEXT NOT NULL, -- ISO 8601 string
    deleted_at TEXT -- ISO 8601 string, null if not deleted
);

-- Capability Table
CREATE TABLE IF NOT EXISTS capabilities (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL, -- 'native', 'skill', 'mcp'
    name TEXT NOT NULL,
    description TEXT,
    input_schema_json TEXT,
    risk_level TEXT NOT NULL, -- 'low', 'medium', 'high'
    enabled BOOLEAN NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

-- Capability Permissions Table
CREATE TABLE IF NOT EXISTS capability_permissions (
    id TEXT PRIMARY KEY,
    capability_id TEXT NOT NULL,
    permission_type TEXT NOT NULL, -- 'file_read', 'file_write', 'network', 'command'
    resource_pattern TEXT NOT NULL,
    granted BOOLEAN NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

-- Capability Invocations Table
CREATE TABLE IF NOT EXISTS capability_invocations (
    id TEXT PRIMARY KEY,
    capability_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    input_payload TEXT,
    output_payload TEXT,
    status TEXT NOT NULL, -- 'success', 'error', 'pending'
    error_message TEXT,
    duration_ms INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- MCP Connectors Table
CREATE TABLE IF NOT EXISTS mcp_connectors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    mcp_type TEXT NOT NULL, -- 'stdio', 'sse'
    status TEXT NOT NULL, -- 'configured', 'healthy', 'error', 'disabled'
    config_content TEXT NOT NULL, -- JSON string containing type-specific config (e.g., command, args, env, url) and enabled flag
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
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
