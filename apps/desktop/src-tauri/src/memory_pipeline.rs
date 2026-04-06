use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("MEMORY_CANDIDATE_NOT_FOUND: {0}")]
    CandidateNotFound(String),
    #[error("INVALID_REQUEST: {0}")]
    InvalidRequest(String),
    #[error("DB_WRITE_FAILED: {0}")]
    DbWriteFailed(String),
    #[error("DB_READ_FAILED: {0}")]
    DbReadFailed(String),
    #[error("MEMORY_FILE_WRITE_FAILED: {0}")]
    FileWriteFailed(String),
    #[error("MEMORY_FILE_READ_FAILED: {0}")]
    FileReadFailed(String),
    #[error("SESSION_NOT_FOUND: {0}")]
    SessionNotFound(String),
    #[error("MESSAGE_NOT_FOUND: {0}")]
    MessageNotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidateRecord {
    pub id: String,
    pub session_id: String,
    pub source_message_id: String,
    pub candidate_type: String,
    pub summary: String,
    pub evidence: Option<String>,
    pub confidence: f64,
    pub tags: Vec<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractCandidatesInput {
    pub session_id: String,
    pub source_message_id: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCandidateInput {
    pub candidate_id: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDoc {
    pub memory_id: String,
    pub candidate_type: String,
    pub tags: Vec<String>,
    pub confidence: f64,
    pub session_id: String,
    pub source_message_id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub summary: String,
    pub evidence: Option<String>,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchHit {
    pub memory_id: String,
    pub candidate_type: String,
    pub confidence: f64,
    pub score: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongTermMemoryItem {
    pub memory_id: String,
    pub candidate_type: String,
    pub tags: Vec<String>,
    pub confidence: f64,
    pub status: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDeleteRestoreResult {
    pub memory_id: String,
    pub deleted_at: Option<String>,
    pub index_docs_affected: usize,
    pub index_terms_affected: usize,
}

pub struct MemoryPipeline {
    runtime_root: PathBuf,
    db_path: PathBuf,
}

impl MemoryPipeline {
    pub fn new(runtime_root: PathBuf, db_path: PathBuf) -> Self {
        Self {
            runtime_root,
            db_path,
        }
    }

    pub fn extract_candidates(
        &self,
        input: ExtractCandidatesInput,
    ) -> Result<Vec<MemoryCandidateRecord>, MemoryError> {
        if input.content.trim().is_empty() {
            return Ok(Vec::new());
        }
        self.validate_source_exists(&input.session_id, &input.source_message_id)?;

        let now = now_iso();
        let lines = split_sentences(&input.content);
        let mut out = Vec::new();
        let conn = self.open_conn()?;
        for (idx, sentence) in lines.iter().enumerate() {
            if out.len() >= 5 {
                break;
            }
            if let Some((candidate_type, confidence)) = classify_candidate(sentence) {
                let id = format!("mc_{}_{}", now_unix_nanos(), idx);
                let tags = extract_tags(sentence);
                let tags_json = serde_json::to_string(&tags)
                    .map_err(|e| MemoryError::InvalidRequest(e.to_string()))?;
                conn.execute(
                    "INSERT INTO memory_candidates (
                        id, session_id, source_message_id, type, summary, evidence, confidence,
                        tags_json, status, created_at, updated_at, deleted_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', ?9, ?10, NULL)",
                    params![
                        id,
                        input.session_id,
                        input.source_message_id,
                        candidate_type,
                        sentence,
                        sentence,
                        confidence,
                        tags_json,
                        now,
                        now
                    ],
                )
                .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;

                out.push(MemoryCandidateRecord {
                    id,
                    session_id: input.session_id.clone(),
                    source_message_id: input.source_message_id.clone(),
                    candidate_type: candidate_type.to_string(),
                    summary: sentence.to_string(),
                    evidence: Some(sentence.to_string()),
                    confidence,
                    tags,
                    status: "pending".to_string(),
                    created_at: now.clone(),
                    updated_at: now.clone(),
                    deleted_at: None,
                });
            }
        }
        Ok(out)
    }

    pub fn list_candidates(
        &self,
        session_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<MemoryCandidateRecord>, MemoryError> {
        let conn = self.open_conn()?;
        let sql = if status.is_some() {
            "SELECT id, session_id, source_message_id, type, summary, evidence, confidence, tags_json, status, created_at, updated_at
             FROM memory_candidates WHERE session_id = ?1 AND status = ?2 AND deleted_at IS NULL ORDER BY created_at DESC"
        } else {
            "SELECT id, session_id, source_message_id, type, summary, evidence, confidence, tags_json, status, created_at, updated_at
             FROM memory_candidates WHERE session_id = ?1 AND deleted_at IS NULL ORDER BY created_at DESC"
        };
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| MemoryError::DbReadFailed(e.to_string()))?;

        let rows = if let Some(s) = status {
            stmt.query_map(params![session_id, s], map_candidate_row)
        } else {
            stmt.query_map(params![session_id], map_candidate_row)
        }
        .map_err(|e| MemoryError::DbReadFailed(e.to_string()))?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| MemoryError::DbReadFailed(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn review_candidate(&self, input: ReviewCandidateInput) -> Result<MemoryDoc, MemoryError> {
        let action = input.action.to_lowercase();
        if action != "approve" && action != "reject" && action != "merge" {
            return Err(MemoryError::InvalidRequest(
                "action must be approve/reject/merge".to_string(),
            ));
        }
        let mut candidate = self.get_candidate(&input.candidate_id)?;
        let now = now_iso();

        match action.as_str() {
            "reject" => {
                let conn = self.open_conn()?;
                conn.execute(
                    "UPDATE memory_candidates SET status = 'rejected', updated_at = ?2 WHERE id = ?1",
                    params![candidate.id, now],
                )
                .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
                candidate.status = "rejected".to_string();
                candidate.updated_at = now;
                
                // Return an empty markdown doc for rejected candidate
                Ok(MemoryDoc::from_candidate(candidate, String::new()))
            }
            "approve" => {
                let version = self.next_markdown_version(&candidate.id);
                let markdown = build_markdown(&candidate, version);
                self.write_memory_markdown_atomic(&candidate.id, &markdown)?;

                let mut conn = self.open_conn()?;
                let tx = conn
                    .transaction()
                    .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
                tx.execute(
                    "UPDATE memory_candidates SET status = 'approved', updated_at = ?2 WHERE id = ?1",
                    params![candidate.id, now],
                )
                .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
                candidate.status = "approved".to_string();
                candidate.updated_at = now.clone();
                self.upsert_index_tx(&tx, &candidate, &now)?;
                tx.commit()
                    .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
                Ok(MemoryDoc::from_candidate(candidate, markdown))
            }
            "merge" => {
                let version = self.next_markdown_version(&candidate.id);
                let markdown = build_markdown(&candidate, version);
                self.write_memory_markdown_atomic(&candidate.id, &markdown)?;

                let mut conn = self.open_conn()?;
                let tx = conn
                    .transaction()
                    .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
                tx.execute(
                    "UPDATE memory_candidates SET status = 'conflicted', updated_at = ?2 WHERE id = ?1",
                    params![candidate.id, now],
                )
                .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
                candidate.status = "conflicted".to_string();
                candidate.updated_at = now.clone();
                self.upsert_index_tx(&tx, &candidate, &now)?;
                tx.commit()
                    .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
                Ok(MemoryDoc::from_candidate(candidate, markdown))
            }
            _ => Err(MemoryError::InvalidRequest("unsupported action".to_string())),
        }
    }

    pub fn read_memory_doc(&self, memory_id: &str) -> Result<MemoryDoc, MemoryError> {
        let path = self.memory_file_path(memory_id);
        let markdown =
            fs::read_to_string(&path).map_err(|e| MemoryError::FileReadFailed(e.to_string()))?;
        let mut candidate = self.get_candidate(memory_id)?;
        if candidate.status != "approved" {
            candidate.status = "approved".to_string();
        }
        Ok(MemoryDoc::from_candidate(candidate, markdown))
    }

    pub fn query_index(
        &self,
        query: &str,
        min_confidence: f64,
        limit: usize,
    ) -> Result<Vec<MemorySearchHit>, MemoryError> {
        let terms = tokenize(query);
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.open_conn()?;
        let placeholders = std::iter::repeat_n("?", terms.len()).collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT d.memory_id, d.type, d.confidence, d.updated_at, SUM(t.tf * t.weight) as score
             FROM memory_index_docs d
             JOIN memory_index_terms t ON d.memory_id = t.memory_id
             WHERE d.deleted_at IS NULL AND d.confidence >= ? AND t.term IN ({})
             GROUP BY d.memory_id, d.type, d.confidence, d.updated_at
             ORDER BY score DESC, d.updated_at DESC
             LIMIT ?",
            placeholders
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| MemoryError::DbReadFailed(e.to_string()))?;

        let mut bind_values: Vec<rusqlite::types::Value> = Vec::new();
        bind_values.push(min_confidence.into());
        for t in &terms {
            bind_values.push(t.clone().into());
        }
        bind_values.push((limit as i64).into());

        let params = rusqlite::params_from_iter(bind_values.iter());
        let rows = stmt
            .query_map(params, |r| {
                Ok(MemorySearchHit {
                    memory_id: r.get(0)?,
                    candidate_type: r.get(1)?,
                    confidence: r.get(2)?,
                    updated_at: r.get(3)?,
                    score: r.get(4)?,
                })
            })
            .map_err(|e| MemoryError::DbReadFailed(e.to_string()))?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| MemoryError::DbReadFailed(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn list_long_term(
        &self,
        candidate_type: Option<&str>,
        status: Option<&str>,
        min_confidence: f64,
        tags: &[String],
        limit: usize,
    ) -> Result<Vec<LongTermMemoryItem>, MemoryError> {
        let conn = self.open_conn()?;
        let mut sql = String::from(
            "SELECT d.memory_id, d.type, d.tags_json, d.confidence, c.status, d.updated_at
             FROM memory_index_docs d
             JOIN memory_candidates c ON c.id = d.memory_id
             WHERE d.deleted_at IS NULL AND d.confidence >= ?1",
        );
        let mut bind_values: Vec<rusqlite::types::Value> = vec![min_confidence.into()];
        if let Some(t) = candidate_type {
            sql.push_str(" AND d.type = ?");
            bind_values.push(t.to_string().into());
        }
        if let Some(s) = status {
            sql.push_str(" AND c.status = ?");
            bind_values.push(s.to_string().into());
        }
        sql.push_str(" ORDER BY d.updated_at DESC");

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| MemoryError::DbReadFailed(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(bind_values.iter()), |r| {
                let tags_json: String = r.get(2)?;
                let parsed_tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Ok(LongTermMemoryItem {
                    memory_id: r.get(0)?,
                    candidate_type: r.get(1)?,
                    tags: parsed_tags,
                    confidence: r.get(3)?,
                    status: r.get(4)?,
                    updated_at: r.get(5)?,
                })
            })
            .map_err(|e| MemoryError::DbReadFailed(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| MemoryError::DbReadFailed(e.to_string()))?);
        }
        let filtered = if tags.is_empty() {
            out
        } else {
            let normalized: Vec<String> = tags.iter().map(|t| t.trim().to_lowercase()).collect();
            out.into_iter()
                .filter(|item| {
                    let set: HashSet<String> = item.tags.iter().map(|t| t.to_lowercase()).collect();
                    normalized.iter().all(|t| set.contains(t))
                })
                .collect()
        };
        Ok(filtered.into_iter().take(limit).collect())
    }

    pub fn soft_delete_memory(
        &self,
        memory_id: &str,
        deleted_at: &str,
    ) -> Result<MemoryDeleteRestoreResult, MemoryError> {
        let mut conn = self.open_conn()?;
        let tx = conn
            .transaction()
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        let candidate_count = tx
            .execute(
                "UPDATE memory_candidates SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
                params![memory_id, deleted_at],
            )
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        if candidate_count == 0 {
            return Err(MemoryError::CandidateNotFound(memory_id.to_string()));
        }
        let docs_count = tx
            .execute(
                "UPDATE memory_index_docs SET deleted_at = ?2 WHERE memory_id = ?1",
                params![memory_id, deleted_at],
            )
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        let terms_count = tx
            .execute(
                "DELETE FROM memory_index_terms WHERE memory_id = ?1",
                params![memory_id],
            )
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        self.rebuild_index_stats_tx(&tx, deleted_at)?;
        tx.commit()
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        Ok(MemoryDeleteRestoreResult {
            memory_id: memory_id.to_string(),
            deleted_at: Some(deleted_at.to_string()),
            index_docs_affected: docs_count,
            index_terms_affected: terms_count,
        })
    }

    pub fn restore_memory(&self, memory_id: &str) -> Result<MemoryDeleteRestoreResult, MemoryError> {
        let mut candidate = self.get_candidate_allow_deleted(memory_id)?;
        let now = now_iso();
        let mut conn = self.open_conn()?;
        let tx = conn
            .transaction()
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        tx.execute(
            "UPDATE memory_candidates SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1",
            params![memory_id, now],
        )
        .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        candidate.deleted_at = None;
        candidate.updated_at = now.clone();

        let docs_count = tx
            .execute(
                "UPDATE memory_index_docs SET deleted_at = NULL, updated_at = ?2 WHERE memory_id = ?1",
                params![memory_id, now],
            )
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        let terms_count = self.upsert_terms_tx(&tx, &candidate, &now)?;
        self.rebuild_index_stats_tx(&tx, &now)?;
        tx.commit()
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        Ok(MemoryDeleteRestoreResult {
            memory_id: memory_id.to_string(),
            deleted_at: None,
            index_docs_affected: docs_count,
            index_terms_affected: terms_count,
        })
    }

    fn get_candidate(&self, candidate_id: &str) -> Result<MemoryCandidateRecord, MemoryError> {
        let conn = self.open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, source_message_id, type, summary, evidence, confidence, tags_json, status, created_at, updated_at
                 FROM memory_candidates WHERE id = ?1 AND deleted_at IS NULL",
            )
            .map_err(|e| MemoryError::DbReadFailed(e.to_string()))?;
        stmt.query_row(params![candidate_id], map_candidate_row)
            .map_err(|_| MemoryError::CandidateNotFound(candidate_id.to_string()))
    }

    fn get_candidate_allow_deleted(
        &self,
        candidate_id: &str,
    ) -> Result<MemoryCandidateRecord, MemoryError> {
        let conn = self.open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, source_message_id, type, summary, evidence, confidence, tags_json, status, created_at, updated_at, deleted_at
                 FROM memory_candidates WHERE id = ?1",
            )
            .map_err(|e| MemoryError::DbReadFailed(e.to_string()))?;
        stmt.query_row(params![candidate_id], map_candidate_row_with_deleted)
            .map_err(|_| MemoryError::CandidateNotFound(candidate_id.to_string()))
    }

    fn validate_source_exists(
        &self,
        session_id: &str,
        source_message_id: &str,
    ) -> Result<(), MemoryError> {
        let conn = self.open_conn()?;
        let session_exists: Option<String> = conn
            .query_row(
                "SELECT id FROM sessions WHERE id = ?1 AND deleted_at IS NULL",
                params![session_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| MemoryError::DbReadFailed(e.to_string()))?;
        if session_exists.is_none() {
            return Err(MemoryError::SessionNotFound(session_id.to_string()));
        }
        let msg_exists: Option<String> = conn
            .query_row(
                "SELECT id FROM messages WHERE id = ?1 AND session_id = ?2 AND deleted_at IS NULL",
                params![source_message_id, session_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| MemoryError::DbReadFailed(e.to_string()))?;
        if msg_exists.is_none() {
            return Err(MemoryError::MessageNotFound(source_message_id.to_string()));
        }
        Ok(())
    }

    fn upsert_index_tx(
        &self,
        tx: &rusqlite::Transaction<'_>,
        candidate: &MemoryCandidateRecord,
        updated_at: &str,
    ) -> Result<(), MemoryError> {
        let tags_json = serde_json::to_string(&candidate.tags)
            .map_err(|e| MemoryError::InvalidRequest(e.to_string()))?;
        tx.execute(
            "INSERT INTO memory_index_docs (memory_id, type, tags_json, confidence, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL)
             ON CONFLICT(memory_id) DO UPDATE SET
               type=excluded.type, tags_json=excluded.tags_json, confidence=excluded.confidence,
               updated_at=excluded.updated_at, deleted_at=NULL",
            params![
                candidate.id,
                candidate.candidate_type,
                tags_json,
                candidate.confidence,
                updated_at
            ],
        )
        .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;

        tx.execute(
            "DELETE FROM memory_index_terms WHERE memory_id = ?1",
            params![candidate.id],
        )
        .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        let _ = self.upsert_terms_tx(tx, candidate, updated_at)?;
        self.rebuild_index_stats_tx(tx, updated_at)?;
        Ok(())
    }

    fn upsert_terms_tx(
        &self,
        tx: &rusqlite::Transaction<'_>,
        candidate: &MemoryCandidateRecord,
        updated_at: &str,
    ) -> Result<usize, MemoryError> {
        let mut term_stats: HashMap<(String, String), f64> = HashMap::new();
        for term in tokenize(&candidate.summary) {
            *term_stats.entry((term, "summary".to_string())).or_insert(0.0) += 1.0;
        }
        for tag in &candidate.tags {
            for term in tokenize(tag) {
                *term_stats.entry((term, "tags".to_string())).or_insert(0.0) += 1.0;
            }
        }
        for term in tokenize(&candidate.candidate_type) {
            *term_stats.entry((term, "type".to_string())).or_insert(0.0) += 1.0;
        }
        if let Some(evidence) = &candidate.evidence {
            for term in tokenize(evidence) {
                *term_stats.entry((term, "evidence".to_string())).or_insert(0.0) += 1.0;
            }
        }
        let mut count = 0usize;
        for ((term, field), tf) in term_stats {
            let weight = match field.as_str() {
                "summary" => 1.3,
                "tags" => 1.5,
                "type" => 1.0,
                "evidence" => 0.8,
                _ => 1.0,
            };
            tx.execute(
                "INSERT INTO memory_index_terms (term, memory_id, field, tf, weight, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![term, candidate.id, field, tf, weight, updated_at],
            )
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
            count += 1;
        }
        Ok(count)
    }

    fn rebuild_index_stats_tx(
        &self,
        tx: &rusqlite::Transaction<'_>,
        updated_at: &str,
    ) -> Result<(), MemoryError> {
        tx.execute("DELETE FROM memory_index_stats", [])
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        tx.execute(
            "INSERT INTO memory_index_stats (term, doc_freq, updated_at)
             SELECT term, COUNT(DISTINCT memory_id) as doc_freq, ?1
             FROM memory_index_terms
             GROUP BY term",
            params![updated_at],
        )
        .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        Ok(())
    }

    fn write_memory_markdown_atomic(
        &self,
        memory_id: &str,
        content: &str,
    ) -> Result<(), MemoryError> {
        let path = self.memory_file_path(memory_id);
        let version = parse_markdown_version(content);
        let version_path = self.memory_version_file_path(memory_id, version);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| MemoryError::FileWriteFailed(e.to_string()))?;
        }
        if let Some(parent) = version_path.parent() {
            fs::create_dir_all(parent).map_err(|e| MemoryError::FileWriteFailed(e.to_string()))?;
        }
        let tmp = path.with_extension(format!("tmp-{}", now_unix_nanos()));
        fs::write(&tmp, content).map_err(|e| MemoryError::FileWriteFailed(e.to_string()))?;
        fs::rename(&tmp, &path).map_err(|e| MemoryError::FileWriteFailed(e.to_string()))?;
        fs::write(version_path, content).map_err(|e| MemoryError::FileWriteFailed(e.to_string()))?;
        Ok(())
    }

    fn next_markdown_version(&self, memory_id: &str) -> usize {
        let versions_dir = self
            .runtime_root
            .join("memory")
            .join("long_term")
            .join("versions");
        let mut max_v = 0usize;
        let prefix = format!("{memory_id}.v");
        if let Ok(entries) = fs::read_dir(versions_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if let Some(raw) = name
                        .strip_prefix(&prefix)
                        .and_then(|s| s.strip_suffix(".md"))
                    {
                        if let Ok(v) = raw.parse::<usize>() {
                            if v > max_v {
                                max_v = v;
                            }
                        }
                    }
                }
            }
        }
        if max_v == 0 { 1 } else { max_v + 1 }
    }

    fn memory_file_path(&self, memory_id: &str) -> PathBuf {
        self.runtime_root
            .join("memory")
            .join("long_term")
            .join(format!("{memory_id}.md"))
    }

    fn memory_version_file_path(&self, memory_id: &str, version: usize) -> PathBuf {
        self.runtime_root
            .join("memory")
            .join("long_term")
            .join("versions")
            .join(format!("{memory_id}.v{version}.md"))
    }

    fn open_conn(&self) -> Result<Connection, MemoryError> {
        let conn =
            Connection::open(&self.db_path).map_err(|e| MemoryError::DbReadFailed(e.to_string()))?;
        conn.pragma_update(None, "foreign_keys", "OFF")
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| MemoryError::DbWriteFailed(e.to_string()))?;
        Ok(conn)
    }
}

impl MemoryDoc {
    fn from_candidate(candidate: MemoryCandidateRecord, markdown: String) -> Self {
        Self {
            memory_id: candidate.id,
            candidate_type: candidate.candidate_type,
            tags: candidate.tags,
            confidence: candidate.confidence,
            session_id: candidate.session_id,
            source_message_id: candidate.source_message_id,
            status: candidate.status,
            created_at: candidate.created_at,
            updated_at: candidate.updated_at,
            summary: candidate.summary,
            evidence: candidate.evidence,
            markdown,
        }
    }
}

fn map_candidate_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryCandidateRecord> {
    let tags_json: String = r.get(7)?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    Ok(MemoryCandidateRecord {
        id: r.get(0)?,
        session_id: r.get(1)?,
        source_message_id: r.get(2)?,
        candidate_type: r.get(3)?,
        summary: r.get(4)?,
        evidence: r.get(5)?,
        confidence: r.get(6)?,
        tags,
        status: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
        deleted_at: None,
    })
}

fn map_candidate_row_with_deleted(r: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryCandidateRecord> {
    let tags_json: String = r.get(7)?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    Ok(MemoryCandidateRecord {
        id: r.get(0)?,
        session_id: r.get(1)?,
        source_message_id: r.get(2)?,
        candidate_type: r.get(3)?,
        summary: r.get(4)?,
        evidence: r.get(5)?,
        confidence: r.get(6)?,
        tags,
        status: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
        deleted_at: r.get(11)?,
    })
}

fn build_markdown(candidate: &MemoryCandidateRecord, version: usize) -> String {
    let tags = if candidate.tags.is_empty() {
        "[]".to_string()
    } else {
        format!(
            "[{}]",
            candidate
                .tags
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    format!(
        "---\nid: {}\nversion: {}\ntype: {}\nstatus: {}\nconfidence: {:.2}\nsession_id: {}\nsource_message_id: {}\ntags: {}\ncreated_at: {}\nupdated_at: {}\n---\n\n{}\n\n{}\n",
        candidate.id,
        version,
        candidate.candidate_type,
        candidate.status,
        candidate.confidence,
        candidate.session_id,
        candidate.source_message_id,
        tags,
        candidate.created_at,
        candidate.updated_at,
        candidate.summary,
        candidate.evidence.clone().unwrap_or_default()
    )
}

fn split_sentences(content: &str) -> Vec<String> {
    content
        .split(['\n', '.', '。', '!', '！', '?', '？'])
        .map(str::trim)
        .filter(|s| s.len() >= 8)
        .map(|s| s.to_string())
        .collect()
}

fn classify_candidate(sentence: &str) -> Option<(&'static str, f64)> {
    let s = sentence.to_lowercase();
    if s.contains("喜欢")
        || s.contains("偏好")
        || s.contains("prefer")
        || s.contains("i like")
    {
        return Some(("preference", 0.85));
    }
    if s.contains("必须")
        || s.contains("不要")
        || s.contains("must")
        || s.contains("don't")
        || s.contains("cannot")
    {
        return Some(("constraint", 0.83));
    }
    if s.contains("项目") || s.contains("project") {
        return Some(("project", 0.78));
    }
    if s.contains("任务") || s.contains("todo") || s.contains("next step") {
        return Some(("task", 0.76));
    }
    if s.contains("我是") || s.contains("i am") || s.contains("我在") || s.contains("we use") {
        return Some(("fact", 0.72));
    }
    None
}

fn extract_tags(sentence: &str) -> Vec<String> {
    let mut set = HashSet::new();
    for token in tokenize(sentence) {
        if token.len() >= 2 && token.len() <= 20 {
            set.insert(token);
        }
        if set.len() >= 6 {
            break;
        }
    }
    set.into_iter().collect()
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            if is_cjk(ch) {
                tokens.push(ch.to_string());
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_cjk(ch: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&ch)
}

fn now_iso() -> String {
    chrono::Local::now().to_rfc3339()
}

fn now_unix_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn parse_markdown_version(content: &str) -> usize {
    for line in content.lines() {
        if let Some(raw) = line.strip_prefix("version:") {
            if let Ok(v) = raw.trim().parse::<usize>() {
                return v;
            }
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite_store::{NewMessage, NewSession, SqliteStore};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(1);

    fn unique_test_id() -> String {
        let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
        format!("{}-{}-{}", now_unix_nanos(), std::process::id(), seq)
    }

    fn setup() -> (PathBuf, PathBuf, MemoryPipeline) {
        let uid = unique_test_id();
        let root = std::env::temp_dir().join(format!("dragon-li-memory-{uid}"));
        let runtime_root = root.join(".dragon-li");
        fs::create_dir_all(runtime_root.join("memory/long_term")).expect("create runtime memory dir");
        fs::create_dir_all(runtime_root.join("data")).expect("create runtime data dir");
        let db_path = runtime_root.join("data").join("dragon_li.db");
        let store = SqliteStore::new(db_path.clone());
        store.init_schema().expect("init schema");
        store
            .create_session(&NewSession {
                id: format!("s1-{uid}"),
                title: "test".to_string(),
                status: "active".to_string(),
                default_provider: None,
                default_model: None,
                created_at: now_iso(),
                updated_at: now_iso(),
            })
            .expect("create session");
        store
            .create_message(&NewMessage {
                id: format!("m1-{uid}"),
                session_id: format!("s1-{uid}"),
                role: "assistant".to_string(),
                content_md: "seed".to_string(),
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
                created_at: now_iso(),
            })
            .expect("create message");
        let pipeline = MemoryPipeline::new(runtime_root.clone(), db_path.clone());
        (root, db_path, pipeline)
    }

    #[test]
    fn extract_review_and_query_index() {
        let (root, db_path, pipeline) = setup();
        let uid = root
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or_default()
            .replace("dragon-li-memory-", "");
        let extracted = pipeline
            .extract_candidates(ExtractCandidatesInput {
                session_id: format!("s1-{uid}"),
                source_message_id: format!("m1-{uid}"),
                content: "我喜欢简洁的代码风格。下一个任务是补齐集成测试。".to_string(),
            })
            .expect("extract ok");
        assert!(!extracted.is_empty());

        let approved = pipeline
            .review_candidate(ReviewCandidateInput {
                candidate_id: extracted[0].id.clone(),
                action: "approve".to_string(),
            })
            .expect("approve ok");
        assert_eq!(approved.status, "approved");
        assert!(approved.markdown.contains("---"));
        assert!(approved.markdown.contains("version: 1"));

        let read_back = pipeline
            .read_memory_doc(&approved.memory_id)
            .expect("read markdown ok");
        assert!(read_back.markdown.contains(&approved.memory_id));

        let hits = pipeline
            .query_index("代码 风格", 0.6, 10)
            .expect("query index ok");
        assert!(!hits.is_empty());

        std::fs::remove_file(db_path).ok();
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn reject_flow_updates_status() {
        let (root, db_path, pipeline) = setup();
        let uid = root
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or_default()
            .replace("dragon-li-memory-", "");
        let extracted = pipeline
            .extract_candidates(ExtractCandidatesInput {
                session_id: format!("s1-{uid}"),
                source_message_id: format!("m1-{uid}"),
                content: "项目里我们使用 Rust 和 Tauri。".to_string(),
            })
            .expect("extract ok");
        assert!(!extracted.is_empty());

        let rejected = pipeline
            .review_candidate(ReviewCandidateInput {
                candidate_id: extracted[0].id.clone(),
                action: "reject".to_string(),
            })
            .expect("reject ok");
        assert_eq!(rejected.status, "rejected");

        std::fs::remove_file(db_path).ok();
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn extract_rejects_invalid_source() {
        let (root, db_path, pipeline) = setup();
        let err = pipeline
            .extract_candidates(ExtractCandidatesInput {
                session_id: "missing-session".to_string(),
                source_message_id: "m1-missing".to_string(),
                content: "我喜欢 Rust".to_string(),
            })
            .expect_err("missing session should fail");
        assert!(matches!(err, MemoryError::SessionNotFound(_)));

        std::fs::remove_file(db_path).ok();
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn delete_restore_syncs_index_terms() {
        let (root, db_path, pipeline) = setup();
        let uid = root
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or_default()
            .replace("dragon-li-memory-", "");
        let extracted = pipeline
            .extract_candidates(ExtractCandidatesInput {
                session_id: format!("s1-{uid}"),
                source_message_id: format!("m1-{uid}"),
                content: "我喜欢整洁的代码和自动化测试。".to_string(),
            })
            .expect("extract ok");
        let approved = pipeline
            .review_candidate(ReviewCandidateInput {
                candidate_id: extracted[0].id.clone(),
                action: "approve".to_string(),
            })
            .expect("approve ok");

        let before = pipeline
            .query_index("代码", 0.6, 10)
            .expect("query before delete");
        assert!(!before.is_empty());

        pipeline
            .soft_delete_memory(&approved.memory_id, &now_iso())
            .expect("soft delete memory");
        let after_delete = pipeline
            .query_index("代码", 0.6, 10)
            .expect("query after delete");
        assert!(after_delete.is_empty());

        pipeline
            .restore_memory(&approved.memory_id)
            .expect("restore memory");
        let after_restore = pipeline
            .query_index("代码", 0.6, 10)
            .expect("query after restore");
        assert!(!after_restore.is_empty());

        std::fs::remove_file(db_path).ok();
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn merge_creates_next_markdown_version() {
        let (root, db_path, pipeline) = setup();
        let uid = root
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or_default()
            .replace("dragon-li-memory-", "");
        let extracted = pipeline
            .extract_candidates(ExtractCandidatesInput {
                session_id: format!("s1-{uid}"),
                source_message_id: format!("m1-{uid}"),
                content: "我喜欢稳定的工程流程和版本化记录。".to_string(),
            })
            .expect("extract ok");
        let first = pipeline
            .review_candidate(ReviewCandidateInput {
                candidate_id: extracted[0].id.clone(),
                action: "approve".to_string(),
            })
            .expect("approve ok");
        assert!(first.markdown.contains("version: 1"));

        let merged = pipeline
            .review_candidate(ReviewCandidateInput {
                candidate_id: extracted[0].id.clone(),
                action: "merge".to_string(),
            })
            .expect("merge ok");
        assert_eq!(merged.status, "conflicted");
        assert!(merged.markdown.contains("version: 2"));
        let version_dir = root
            .join(".dragon-li")
            .join("memory")
            .join("long_term")
            .join("versions");
        assert!(version_dir.join(format!("{}.v1.md", merged.memory_id)).exists());
        assert!(version_dir.join(format!("{}.v2.md", merged.memory_id)).exists());

        std::fs::remove_file(db_path).ok();
        std::fs::remove_dir_all(root).ok();
    }
}
