use rusqlite::{params, Connection, Error as SqliteError, ErrorCode, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use thiserror::Error;

const INIT_DDL: &str = r#"
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
  reasoning_md TEXT,
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

    CREATE TABLE IF NOT EXISTS capabilities (
        id TEXT PRIMARY KEY,
        type TEXT NOT NULL,
        name TEXT NOT NULL,
        description TEXT,
        input_schema_json TEXT,
        risk_level TEXT NOT NULL,
        enabled BOOLEAN NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        deleted_at TEXT
    );

    CREATE TABLE IF NOT EXISTS capability_permissions (
        id TEXT PRIMARY KEY,
        capability_id TEXT NOT NULL,
        permission_type TEXT NOT NULL,
        resource_pattern TEXT NOT NULL,
        granted BOOLEAN NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        deleted_at TEXT
    );

    CREATE TABLE IF NOT EXISTS capability_invocations (
        id TEXT PRIMARY KEY,
        capability_id TEXT NOT NULL,
        session_id TEXT NOT NULL,
        message_id TEXT NOT NULL,
        input_payload TEXT,
        output_payload TEXT,
        status TEXT NOT NULL,
        error_message TEXT,
        duration_ms INTEGER,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS mcp_connectors (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        mcp_type TEXT NOT NULL,
        status TEXT NOT NULL,
        config_content TEXT NOT NULL,
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
"#;

const MAX_DB_BUSY_RETRIES: usize = 2;
const DB_BUSY_RETRY_BACKOFF_MS: [u64; 2] = [500, 1500];

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("DB_INIT_FAILED: {0}")]
    DbInitFailed(String),
    #[error("DB_BUSY: {0}")]
    DbBusy(String),
    #[error("DB_WRITE_FAILED: {0}")]
    DbWriteFailed(String),
    #[error("DB_READ_FAILED: {0}")]
    DbReadFailed(String),
    #[error("SESSION_NOT_FOUND: {0}")]
    SessionNotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    pub status: String,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSession {
    pub id: String,
    pub title: String,
    pub status: String,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content_md: String,
    pub reasoning_md: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub latency_ms: Option<i64>,
    pub parent_message_id: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retryable: Option<i64>,
    pub created_at: String,
    pub deleted_at: Option<String>,
    pub explicit_skill_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content_md: String,
    pub reasoning_md: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub latency_ms: Option<i64>,
    pub parent_message_id: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retryable: Option<i64>,
    pub created_at: String,
    pub explicit_skill_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLogRecord {
    pub id: String,
    pub request_id: String,
    pub session_id: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: String,
    pub latency_ms: Option<i64>,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub error_code: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewRequestLog {
    pub id: String,
    pub request_id: String,
    pub session_id: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: String,
    pub latency_ms: Option<i64>,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub error_code: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRestoreResult {
    pub sessions_affected: usize,
    pub messages_affected: usize,
    pub memory_candidates_affected: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConnectorRecord {
    pub id: String,
    pub name: String,
    pub mcp_type: String,
    pub status: String,
    pub config_content: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMcpConnector {
    pub id: String,
    pub name: String,
    pub mcp_type: String,
    pub status: String,
    pub config_content: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRecord {
    pub id: String,
    pub r#type: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema_json: Option<String>,
    pub risk_level: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Clone)]
pub struct SqliteStore {
    db_path: PathBuf,
}

impl SqliteStore {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    pub fn init_schema(&self) -> Result<(), StoreError> {
        let conn = self.open_conn().map_err(|e| StoreError::DbInitFailed(e.to_string()))?;
        
        // Migration: Check if the old 'endpoint' column exists in mcp_connectors.
        // If it does, we drop the table so INIT_DDL can recreate it with the correct schema.
        let mut should_drop_mcp = false;
        if let Ok(mut stmt) = conn.prepare("PRAGMA table_info(mcp_connectors)") {
            if let Ok(mut rows) = stmt.query([]) {
                while let Ok(Some(row)) = rows.next() {
                    let name: String = row.get(1).unwrap_or_default();
                    if name == "endpoint" {
                        should_drop_mcp = true;
                        break;
                    }
                }
            }
        }
        if should_drop_mcp {
            let _ = conn.execute("DROP TABLE IF EXISTS mcp_connectors", []);
        }

        conn.execute_batch(INIT_DDL)
            .map_err(|e| StoreError::DbInitFailed(e.to_string()))?;
        // Migration for existing databases
        let _ = conn.execute("ALTER TABLE messages ADD COLUMN reasoning_md TEXT", []);
        let _ = conn.execute("ALTER TABLE mcp_connectors ADD COLUMN mcp_type TEXT NOT NULL DEFAULT 'stdio'", []);
        let _ = conn.execute("ALTER TABLE mcp_connectors ADD COLUMN config_content TEXT NOT NULL DEFAULT '{}'", []);
        let _ = conn.execute("ALTER TABLE messages ADD COLUMN explicit_skill_id TEXT", []);
        Ok(())
    }

    pub fn create_session(&self, item: &NewSession) -> Result<(), StoreError> {
        with_busy_retry(|| {
            let conn = self.open_conn()?;
            conn.execute(
                "INSERT INTO sessions (id, title, status, default_provider, default_model, created_at, updated_at, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
                params![
                    item.id,
                    item.title,
                    item.status,
                    item.default_provider,
                    item.default_model,
                    item.created_at,
                    item.updated_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionRecord>, StoreError> {
        let conn = self.open_conn().map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, status, default_provider, default_model, created_at, updated_at, deleted_at
                 FROM sessions
                 WHERE deleted_at IS NULL
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(SessionRecord {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    status: r.get(2)?,
                    default_provider: r.get(3)?,
                    default_model: r.get(4)?,
                    created_at: r.get(5)?,
                    updated_at: r.get(6)?,
                    deleted_at: r.get(7)?,
                })
            })
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        collect_rows(rows).map_err(|e| StoreError::DbReadFailed(e.to_string()))
    }

    pub fn get_latest_user_message(
        &self,
        session_id: &str,
    ) -> Result<MessageRecord, StoreError> {
        let conn = self.open_conn().map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, role, content_md, reasoning_md, provider, model, tokens_in, tokens_out, latency_ms,
                        parent_message_id, status, error_code, error_message, retryable, created_at, deleted_at, explicit_skill_id
                 FROM messages
                 WHERE session_id = ?1 AND role = 'user' AND deleted_at IS NULL
                 ORDER BY created_at DESC
                 LIMIT 1",
            )
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let msg = stmt
            .query_row([session_id], |r| {
                Ok(MessageRecord {
                    id: r.get(0)?,
                    session_id: r.get(1)?,
                    role: r.get(2)?,
                    content_md: r.get(3)?,
                    reasoning_md: r.get(4)?,
                    provider: r.get(5)?,
                    model: r.get(6)?,
                    tokens_in: r.get(7)?,
                    tokens_out: r.get(8)?,
                    latency_ms: r.get(9)?,
                    parent_message_id: r.get(10)?,
                    status: r.get(11)?,
                    error_code: r.get(12)?,
                    error_message: r.get(13)?,
                    retryable: r.get(14)?,
                    created_at: r.get(15)?,
                    deleted_at: r.get(16)?,
                    explicit_skill_id: r.get(17)?,
                })
            })
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        Ok(msg)
    }

    pub fn update_session_title(
        &self,
        session_id: &str,
        title: &str,
        updated_at: &str,
    ) -> Result<usize, StoreError> {
        with_busy_retry(|| {
            let conn = self.open_conn()?;
            let count = conn.execute(
                "UPDATE sessions SET title = ?2, updated_at = ?3 WHERE id = ?1 AND deleted_at IS NULL",
                params![session_id, title, updated_at],
            )?;
            if count == 0 {
                return Err(SqliteError::QueryReturnedNoRows); // Map to SessionNotFound later if needed, or handle here. Actually with_busy_retry expects SqliteError.
            }
            Ok(())
        }).map(|_| 1).map_err(|e| match e {
            StoreError::DbWriteFailed(msg) if msg.contains("QueryReturnedNoRows") => {
                StoreError::SessionNotFound(session_id.to_string())
            }
            other => other,
        })
    }

    pub fn create_message(&self, item: &NewMessage) -> Result<(), StoreError> {
        let conn = self.open_conn().map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;
        let session_exists: Option<String> = conn
            .query_row(
                "SELECT id FROM sessions WHERE id = ?1 AND deleted_at IS NULL",
                [item.session_id.as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        if session_exists.is_none() {
            return Err(StoreError::SessionNotFound(item.session_id.clone()));
        }

        conn.execute(
            "INSERT OR REPLACE INTO messages (
                id, session_id, role, content_md, reasoning_md, provider, model,
                tokens_in, tokens_out, latency_ms, parent_message_id, status,
                error_code, error_message, retryable, created_at, deleted_at, explicit_skill_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, NULL, ?17)",
            params![
                item.id,
                item.session_id,
                item.role,
                item.content_md,
                item.reasoning_md,
                item.provider,
                item.model,
                item.tokens_in,
                item.tokens_out,
                item.latency_ms,
                item.parent_message_id,
                item.status,
                item.error_code,
                item.error_message,
                item.retryable,
                item.created_at,
                item.explicit_skill_id
            ],
        )
        .map_err(map_write_error)?;
        Ok(())
    }

    pub fn update_message_completion(
        &self,
        msg_id: &str,
        content_md: &str,
        reasoning_md: &str,
        status: &str,
        tokens_in: Option<i64>,
        tokens_out: Option<i64>,
        latency_ms: Option<i64>,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), StoreError> {
        with_busy_retry(|| {
            let conn = self.open_conn()?;
            conn.execute(
                "UPDATE messages 
                 SET content_md = ?1, reasoning_md = ?2, status = ?3, tokens_in = ?4, tokens_out = ?5, latency_ms = ?6, error_code = ?7, error_message = ?8
                 WHERE id = ?9 AND deleted_at IS NULL",
                params![content_md, reasoning_md, status, tokens_in, tokens_out, latency_ms, error_code, error_message, msg_id],
            )?;
            Ok(())
        })
    }

    pub fn list_messages(&self, session_id: &str) -> Result<Vec<MessageRecord>, StoreError> {
        let conn = self.open_conn().map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, role, content_md, reasoning_md, provider, model, tokens_in, tokens_out, latency_ms,
                        parent_message_id, status, error_code, error_message, retryable, created_at, deleted_at, explicit_skill_id
                 FROM messages
                 WHERE session_id = ?1 AND deleted_at IS NULL
                 ORDER BY created_at ASC",
            )
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let rows = stmt
            .query_map([session_id], |r| {
                Ok(MessageRecord {
                    id: r.get(0)?,
                    session_id: r.get(1)?,
                    role: r.get(2)?,
                    content_md: r.get(3)?,
                    reasoning_md: r.get(4)?,
                    provider: r.get(5)?,
                    model: r.get(6)?,
                    tokens_in: r.get(7)?,
                    tokens_out: r.get(8)?,
                    latency_ms: r.get(9)?,
                    parent_message_id: r.get(10)?,
                    status: r.get(11)?,
                    error_code: r.get(12)?,
                    error_message: r.get(13)?,
                    retryable: r.get(14)?,
                    created_at: r.get(15)?,
                    deleted_at: r.get(16)?,
                    explicit_skill_id: r.get(17)?,
                })
            })
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        collect_rows(rows).map_err(|e| StoreError::DbReadFailed(e.to_string()))
    }

    pub fn create_request_log(&self, item: &NewRequestLog) -> Result<(), StoreError> {
        with_busy_retry(|| {
            let conn = self.open_conn()?;
            conn.execute(
                "INSERT INTO request_logs (
                    id, request_id, session_id, provider, model, status,
                    latency_ms, tokens_in, tokens_out, error_code, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    item.id,
                    item.request_id,
                    item.session_id,
                    item.provider,
                    item.model,
                    item.status,
                    item.latency_ms,
                    item.tokens_in,
                    item.tokens_out,
                    item.error_code,
                    item.created_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_request_logs_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Vec<RequestLogRecord>, StoreError> {
        let conn = self.open_conn().map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, request_id, session_id, provider, model, status, latency_ms,
                        tokens_in, tokens_out, error_code, created_at
                 FROM request_logs
                 WHERE request_id = ?1
                 ORDER BY created_at DESC",
            )
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let rows = stmt
            .query_map([request_id], |r| {
                Ok(RequestLogRecord {
                    id: r.get(0)?,
                    request_id: r.get(1)?,
                    session_id: r.get(2)?,
                    provider: r.get(3)?,
                    model: r.get(4)?,
                    status: r.get(5)?,
                    latency_ms: r.get(6)?,
                    tokens_in: r.get(7)?,
                    tokens_out: r.get(8)?,
                    error_code: r.get(9)?,
                    created_at: r.get(10)?,
                })
            })
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        collect_rows(rows).map_err(|e| StoreError::DbReadFailed(e.to_string()))
    }

    pub fn soft_delete_session(
        &self,
        session_id: &str,
        deleted_at: &str,
    ) -> Result<DeleteRestoreResult, StoreError> {
        let mut conn = self.open_conn().map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;
        let tx = conn
            .transaction()
            .map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;

        let session_count = tx
            .execute(
                "UPDATE sessions SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
                params![session_id, deleted_at],
            )
            .map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;
        if session_count == 0 {
            return Err(StoreError::SessionNotFound(session_id.to_string()));
        }

        let msg_count = tx
            .execute(
                "UPDATE messages SET deleted_at = ?2 WHERE session_id = ?1 AND deleted_at IS NULL",
                params![session_id, deleted_at],
            )
            .map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;

        let memory_count = tx
            .execute(
                "UPDATE memory_candidates SET deleted_at = ?2 WHERE session_id = ?1 AND deleted_at IS NULL",
                params![session_id, deleted_at],
            )
            .map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;

        tx.commit()
            .map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;

        Ok(DeleteRestoreResult {
            sessions_affected: session_count,
            messages_affected: msg_count,
            memory_candidates_affected: memory_count,
        })
    }

    pub fn restore_session(&self, session_id: &str) -> Result<DeleteRestoreResult, StoreError> {
        let mut conn = self.open_conn().map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;
        let tx = conn
            .transaction()
            .map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;

        let exists: Option<String> = tx
            .query_row(
                "SELECT id FROM sessions WHERE id = ?1",
                [session_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        if exists.is_none() {
            return Err(StoreError::SessionNotFound(session_id.to_string()));
        }

        let session_count = tx
            .execute(
                "UPDATE sessions SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL",
                [session_id],
            )
            .map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;

        let msg_count = tx
            .execute(
                "UPDATE messages SET deleted_at = NULL WHERE session_id = ?1 AND deleted_at IS NOT NULL",
                [session_id],
            )
            .map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;

        let memory_count = tx
            .execute(
                "UPDATE memory_candidates SET deleted_at = NULL WHERE session_id = ?1 AND deleted_at IS NOT NULL",
                [session_id],
            )
            .map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;

        tx.commit()
            .map_err(|e| StoreError::DbWriteFailed(e.to_string()))?;

        Ok(DeleteRestoreResult {
            sessions_affected: session_count,
            messages_affected: msg_count,
            memory_candidates_affected: memory_count,
        })
    }

    fn open_conn(&self) -> Result<Connection, rusqlite::Error> {
        if let Some(parent) = self.db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&self.db_path)?;
        conn.pragma_update(None, "busy_timeout", 0)?;
        conn.pragma_update(None, "foreign_keys", "OFF")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Ok(conn)
    }

    pub fn create_mcp_connector(&self, item: &NewMcpConnector) -> Result<(), StoreError> {
        with_busy_retry(|| {
            let conn = self.open_conn()?;
            conn.execute(
                "INSERT INTO mcp_connectors (id, name, mcp_type, status, config_content, created_at, updated_at, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
                params![
                    item.id,
                    item.name,
                    item.mcp_type,
                    item.status,
                    item.config_content,
                    item.created_at,
                    item.updated_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_mcp_connectors(&self) -> Result<Vec<McpConnectorRecord>, StoreError> {
        let conn = self.open_conn().map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, mcp_type, status, config_content, created_at, updated_at, deleted_at
                 FROM mcp_connectors
                 WHERE deleted_at IS NULL
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(McpConnectorRecord {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    mcp_type: r.get(2)?,
                    status: r.get(3)?,
                    config_content: r.get(4)?,
                    created_at: r.get(5)?,
                    updated_at: r.get(6)?,
                    deleted_at: r.get(7)?,
                })
            })
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| StoreError::DbReadFailed(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn update_mcp_connector(
        &self,
        id: &str,
        name: &str,
        mcp_type: &str,
        status: &str,
        config_content: &str,
        updated_at: &str,
    ) -> Result<(), StoreError> {
        with_busy_retry(|| {
            let conn = self.open_conn()?;
            let rows = conn.execute(
                "UPDATE mcp_connectors
                 SET name = ?1, mcp_type = ?2, status = ?3, config_content = ?4, updated_at = ?5
                 WHERE id = ?6 AND deleted_at IS NULL",
                params![
                    name,
                    mcp_type,
                    status,
                    config_content,
                    updated_at,
                    id
                ],
            )?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
    }

    pub fn delete_mcp_connector(&self, id: &str, deleted_at: &str) -> Result<(), StoreError> {
        with_busy_retry(|| {
            let conn = self.open_conn()?;
            conn.execute(
                "UPDATE mcp_connectors
                 SET deleted_at = ?1
                 WHERE id = ?2 AND deleted_at IS NULL",
                params![deleted_at, id],
            )?;
            Ok(())
        })
    }

    pub fn list_skills(&self) -> Result<Vec<CapabilityRecord>, StoreError> {
        let conn = self.open_conn().map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, type, name, description, input_schema_json, risk_level, enabled, created_at, updated_at, deleted_at
                 FROM capabilities
                 WHERE type = 'skill' AND deleted_at IS NULL
                 ORDER BY name ASC",
            )
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(CapabilityRecord {
                    id: r.get(0)?,
                    r#type: r.get(1)?,
                    name: r.get(2)?,
                    description: r.get(3)?,
                    input_schema_json: r.get(4)?,
                    risk_level: r.get(5)?,
                    enabled: r.get(6)?,
                    created_at: r.get(7)?,
                    updated_at: r.get(8)?,
                    deleted_at: r.get(9)?,
                })
            })
            .map_err(|e| StoreError::DbReadFailed(e.to_string()))?;
        collect_rows(rows).map_err(|e| StoreError::DbReadFailed(e.to_string()))
    }

    pub fn update_skill_enabled(
        &self,
        id: &str,
        enabled: bool,
        updated_at: &str,
    ) -> Result<(), StoreError> {
        with_busy_retry(|| {
            let conn = self.open_conn()?;
            let rows = conn.execute(
                "UPDATE capabilities
                 SET enabled = ?1, updated_at = ?2
                 WHERE id = ?3 AND type = 'skill' AND deleted_at IS NULL",
                params![enabled, updated_at, id],
            )?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
    }
}

fn is_busy_error(err: &SqliteError) -> bool {
    match err {
        SqliteError::SqliteFailure(inner, _) => {
            inner.code == ErrorCode::DatabaseBusy || inner.code == ErrorCode::DatabaseLocked
        }
        _ => false,
    }
}

fn map_write_error(err: SqliteError) -> StoreError {
    if is_busy_error(&err) {
        return StoreError::DbBusy(err.to_string());
    }
    StoreError::DbWriteFailed(err.to_string())
}

fn with_busy_retry<F>(mut op: F) -> Result<(), StoreError>
where
    F: FnMut() -> Result<(), SqliteError>,
{
    let mut retries = 0usize;
    loop {
        match op() {
            Ok(()) => return Ok(()),
            Err(err) => {
                if is_busy_error(&err) && retries < MAX_DB_BUSY_RETRIES {
                    let backoff = DB_BUSY_RETRY_BACKOFF_MS
                        .get(retries)
                        .copied()
                        .unwrap_or_else(|| *DB_BUSY_RETRY_BACKOFF_MS.last().unwrap_or(&1500));
                    retries += 1;
                    thread::sleep(Duration::from_millis(backoff));
                    continue;
                }
                return Err(map_write_error(err));
            }
        }
    }
}

fn collect_rows<T, I>(rows: I) -> rusqlite::Result<Vec<T>>
where
    I: IntoIterator<Item = rusqlite::Result<T>>,
{
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn default_db_path(runtime_root: &Path) -> PathBuf {
    runtime_root.join("data").join("dragon_li.db")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id();
        std::env::temp_dir().join(format!("dragon-li-db-{pid}-{nanos}.db"))
    }

    #[test]
    fn init_and_repo_flow() {
        let db = temp_db_path();
        let store = SqliteStore::new(db.clone());
        store.init_schema().expect("schema init");

        store
            .create_session(&NewSession {
                id: "s1".to_string(),
                title: "Test Session".to_string(),
                status: "active".to_string(),
                default_provider: Some("openai".to_string()),
                default_model: Some("gpt-4o-mini".to_string()),
                created_at: "2026-04-05T10:00:00+08:00".to_string(),
                updated_at: "2026-04-05T10:00:00+08:00".to_string(),
            })
            .expect("create session");

        store
            .create_message(&NewMessage {
                id: "m1".to_string(),
                session_id: "s1".to_string(),
                role: "user".to_string(),
                content_md: "hello".to_string(),
                reasoning_md: None,
                provider: Some("openai".to_string()),
                model: Some("gpt-4o-mini".to_string()),
                tokens_in: Some(10),
                tokens_out: Some(20),
                latency_ms: Some(500),
                parent_message_id: None,
                status: "ok".to_string(),
                error_code: None,
                error_message: None,
                retryable: None,
                created_at: "2026-04-05T10:01:00+08:00".to_string(),
                explicit_skill_id: None,
            })
            .expect("create message");

        store
            .create_request_log(&NewRequestLog {
                id: "r1".to_string(),
                request_id: "req_abc".to_string(),
                session_id: Some("s1".to_string()),
                provider: Some("openai".to_string()),
                model: Some("gpt-4o-mini".to_string()),
                status: "ok".to_string(),
                latency_ms: Some(500),
                tokens_in: Some(10),
                tokens_out: Some(20),
                error_code: None,
                created_at: "2026-04-05T10:01:02+08:00".to_string(),
            })
            .expect("create request log");

        let sessions = store.list_sessions().expect("list sessions");
        assert_eq!(sessions.len(), 1);

        let messages = store.list_messages("s1").expect("list messages");
        assert_eq!(messages.len(), 1);

        let logs = store
            .list_request_logs_by_request_id("req_abc")
            .expect("request logs");
        assert_eq!(logs.len(), 1);

        std::fs::remove_file(db).ok();
    }

    #[test]
    fn soft_delete_and_restore_cascade() {
        let db = temp_db_path();
        let store = SqliteStore::new(db.clone());
        store.init_schema().expect("schema init");

        store
            .create_session(&NewSession {
                id: "s1".to_string(),
                title: "Test".to_string(),
                status: "active".to_string(),
                default_provider: None,
                default_model: None,
                created_at: "2026-04-05T10:00:00+08:00".to_string(),
                updated_at: "2026-04-05T10:00:00+08:00".to_string(),
            })
            .expect("create session");
        store
            .create_message(&NewMessage {
                id: "m1".to_string(),
                session_id: "s1".to_string(),
                role: "assistant".to_string(),
                content_md: "reply".to_string(),
                reasoning_md: None,
                provider: None,
                model: None,
                tokens_in: None,
                tokens_out: None,
                latency_ms: None,
                parent_message_id: None,
                status: "ok".to_string(),
                error_code: None,
                error_message: None,
                retryable: None,
                created_at: "2026-04-05T10:01:00+08:00".to_string(),
                explicit_skill_id: None,
            })
            .expect("create message");

        let deleted = store
            .soft_delete_session("s1", "2026-04-05T11:00:00+08:00")
            .expect("soft delete");
        assert_eq!(deleted.sessions_affected, 1);

        let messages_after_delete = store.list_messages("s1").expect("list messages");
        assert_eq!(messages_after_delete.len(), 0);

        let restored = store.restore_session("s1").expect("restore");
        assert_eq!(restored.sessions_affected, 1);

        let messages_after_restore = store.list_messages("s1").expect("list messages");
        assert_eq!(messages_after_restore.len(), 1);

        std::fs::remove_file(db).ok();
    }

    #[test]
    fn create_message_requires_existing_session() {
        let db = temp_db_path();
        let store = SqliteStore::new(db.clone());
        store.init_schema().expect("schema init");

        let err = store
            .create_message(&NewMessage {
                id: "m-orphan".to_string(),
                session_id: "missing".to_string(),
                role: "user".to_string(),
                content_md: "hello".to_string(),
                reasoning_md: None,
                provider: None,
                model: None,
                tokens_in: None,
                tokens_out: None,
                latency_ms: None,
                parent_message_id: None,
                status: "ok".to_string(),
                error_code: None,
                error_message: None,
                retryable: None,
                created_at: "2026-04-05T10:01:00+08:00".to_string(),
                explicit_skill_id: None,
            })
            .expect_err("orphan message should fail");
        assert!(matches!(err, StoreError::SessionNotFound(_)));

        std::fs::remove_file(db).ok();
    }

    #[test]
    fn request_log_returns_db_busy_when_locked() {
        let db = temp_db_path();
        let store = SqliteStore::new(db.clone());
        store.init_schema().expect("schema init");

        let lock_conn = Connection::open(&db).expect("open lock connection");
        lock_conn
            .execute_batch("PRAGMA journal_mode = WAL; BEGIN IMMEDIATE;")
            .expect("hold write lock");

        let err = store
            .create_request_log(&NewRequestLog {
                id: "busy_log".to_string(),
                request_id: "req_busy".to_string(),
                session_id: None,
                provider: None,
                model: None,
                status: "failed".to_string(),
                latency_ms: None,
                tokens_in: None,
                tokens_out: None,
                error_code: Some("DB_BUSY".to_string()),
                created_at: "2026-04-05T10:01:02+08:00".to_string(),
            })
            .expect_err("should return busy error");

        assert!(matches!(err, StoreError::DbBusy(_)));

        lock_conn.execute_batch("ROLLBACK;").ok();
        std::fs::remove_file(db).ok();
    }
}
