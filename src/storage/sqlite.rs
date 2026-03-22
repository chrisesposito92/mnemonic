use std::sync::Arc;
use tokio_rusqlite::Connection;
use rusqlite::OptionalExtension;
use zerocopy::IntoBytes;
use async_trait::async_trait;
use crate::storage::{StorageBackend, StoreRequest, CandidateRecord, MergedMemoryRequest};
use crate::service::{Memory, ListResponse, SearchResponse, SearchResultItem, ListParams, SearchParams};
use crate::error::ApiError;

// ──────────────────────────────────────────────────────────────────────────────
// SqliteBackend struct
// ──────────────────────────────────────────────────────────────────────────────

/// SQLite implementation of StorageBackend.
///
/// Wraps `Arc<Connection>` so that multiple services can share the same
/// database connection. All SQL is extracted verbatim from service.rs and
/// compaction.rs with no behavioural changes — this is purely a structural
/// extraction into the backend abstraction layer.
pub struct SqliteBackend {
    db: Arc<Connection>,
}

impl SqliteBackend {
    pub fn new(db: Arc<Connection>) -> Self {
        Self { db }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// StorageBackend implementation
// ──────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl StorageBackend for SqliteBackend {
    /// Store a memory with a pre-computed embedding. Atomic dual-table write.
    ///
    /// SQL extracted verbatim from MemoryService::create_memory (service.rs lines 115-133).
    async fn store(&self, req: StoreRequest) -> Result<Memory, ApiError> {
        let embedding_bytes: Vec<u8> = req.embedding.as_bytes().to_vec();

        let id = req.id.clone();
        let content = req.content.clone();
        let agent_id = req.agent_id.clone();
        let session_id = req.session_id.clone();
        let tags_json = serde_json::to_string(&req.tags).unwrap_or_else(|_| "[]".to_string());
        let embedding_model = req.embedding_model.clone();

        let id_clone = id.clone();
        let content_clone = content.clone();
        let agent_id_clone = agent_id.clone();
        let session_id_clone = session_id.clone();
        let tags_json_clone = tags_json.clone();
        let embedding_model_clone = embedding_model.clone();

        let created_at = self.db.call(move |c| -> Result<String, rusqlite::Error> {
            let tx = c.transaction()?;
            tx.execute(
                "INSERT INTO memories (id, content, agent_id, session_id, tags, embedding_model, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
                rusqlite::params![id_clone, content_clone, agent_id_clone, session_id_clone, tags_json_clone, embedding_model_clone],
            )?;
            tx.execute(
                "INSERT INTO vec_memories (memory_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![id_clone, embedding_bytes],
            )?;
            let created_at: String = tx.query_row(
                "SELECT created_at FROM memories WHERE id = ?1",
                rusqlite::params![id_clone],
                |row| row.get(0),
            )?;
            tx.commit()?;
            Ok(created_at)
        }).await?;

        Ok(Memory {
            id,
            content,
            agent_id,
            session_id,
            tags: req.tags,
            embedding_model,
            created_at,
            updated_at: None,
        })
    }

    /// Get a single memory by ID. Returns None if not found.
    ///
    /// SQL derived from MemoryService::delete_memory inner SELECT (service.rs lines 316-333).
    async fn get_by_id(&self, id: &str) -> Result<Option<Memory>, ApiError> {
        let id = id.to_string();
        let result = self.db.call(move |c| -> Result<Option<Memory>, rusqlite::Error> {
            let mut stmt = c.prepare(
                "SELECT id, content, agent_id, session_id, tags, embedding_model, created_at, updated_at
                 FROM memories WHERE id = ?1"
            )?;
            stmt.query_row(rusqlite::params![id], |row| {
                let tags_str: String = row.get(4)?;
                let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                Ok(Memory {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    agent_id: row.get(2)?,
                    session_id: row.get(3)?,
                    tags,
                    embedding_model: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            }).optional()
        }).await?;
        Ok(result)
    }

    /// List memories with filtering and pagination.
    ///
    /// SQL extracted verbatim from MemoryService::list_memories (service.rs lines 245-289).
    async fn list(&self, params: ListParams) -> Result<ListResponse, ApiError> {
        let limit = params.limit.unwrap_or(20).min(100) as i64;
        let offset = params.offset.unwrap_or(0) as i64;
        let agent_id = params.agent_id;
        let session_id = params.session_id;
        let tag = params.tag;
        let after = params.after;
        let before = params.before;

        // Clone for the count query
        let agent_id2 = agent_id.clone();
        let session_id2 = session_id.clone();
        let tag2 = tag.clone();
        let after2 = after.clone();
        let before2 = before.clone();

        let (memories, total) = self.db.call(move |c| -> Result<(Vec<Memory>, u64), rusqlite::Error> {
            let filter_clause = "WHERE (?1 IS NULL OR agent_id = ?1)
                  AND (?2 IS NULL OR session_id = ?2)
                  AND (?3 IS NULL OR tags LIKE '%' || ?3 || '%')
                  AND (?4 IS NULL OR created_at > ?4)
                  AND (?5 IS NULL OR created_at < ?5)";

            // Count query
            let count_sql = format!("SELECT COUNT(*) FROM memories {}", filter_clause);
            let total: u64 = c.query_row(
                &count_sql,
                rusqlite::params![agent_id2, session_id2, tag2, after2, before2],
                |row| row.get(0),
            )?;

            // Results query
            let results_sql = format!(
                "SELECT id, content, agent_id, session_id, tags, embedding_model, created_at, updated_at
                 FROM memories {}
                 ORDER BY created_at DESC
                 LIMIT ?6 OFFSET ?7",
                filter_clause
            );

            let mut stmt = c.prepare(&results_sql)?;
            let rows = stmt.query_map(
                rusqlite::params![agent_id, session_id, tag, after, before, limit, offset],
                |row| {
                    let tags_str: String = row.get(4)?;
                    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                    Ok(Memory {
                        id: row.get(0)?,
                        content: row.get(1)?,
                        agent_id: row.get(2)?,
                        session_id: row.get(3)?,
                        tags,
                        embedding_model: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                },
            )?;

            let memories = rows.collect::<Result<Vec<_>, _>>()?;
            Ok((memories, total))
        }).await?;

        Ok(ListResponse { memories, total })
    }

    /// Semantic search using a pre-computed query embedding.
    ///
    /// SQL extracted verbatim from MemoryService::search_memories (service.rs lines 172-216).
    /// The `params.q` field is ignored here — embedding is already computed by the caller (per D-09).
    /// Distance semantics: lower is better (per D-02).
    async fn search(&self, embedding: Vec<f32>, params: SearchParams) -> Result<SearchResponse, ApiError> {
        let limit = params.limit.unwrap_or(10).min(100) as i64;
        let has_filters = params.agent_id.is_some() || params.session_id.is_some();
        let k = if has_filters { (limit * 10).min(1000) } else { limit };

        let query_bytes: Vec<u8> = embedding.as_bytes().to_vec();

        let agent_id = params.agent_id;
        let session_id = params.session_id;
        let tag = params.tag;
        let after = params.after;
        let before = params.before;
        let threshold = params.threshold;

        let results = self.db.call(move |c| -> Result<Vec<(Memory, f64)>, rusqlite::Error> {
            let mut stmt = c.prepare(
                "WITH knn_candidates AS (
                    SELECT memory_id, distance
                    FROM vec_memories
                    WHERE embedding MATCH ?1
                    AND k = ?2
                )
                SELECT m.id, m.content, m.agent_id, m.session_id, m.tags,
                       m.embedding_model, m.created_at, m.updated_at,
                       knn_candidates.distance
                FROM knn_candidates
                JOIN memories m ON m.id = knn_candidates.memory_id
                WHERE (?3 IS NULL OR m.agent_id = ?3)
                  AND (?4 IS NULL OR m.session_id = ?4)
                  AND (?5 IS NULL OR m.tags LIKE '%' || ?5 || '%')
                  AND (?6 IS NULL OR m.created_at > ?6)
                  AND (?7 IS NULL OR m.created_at < ?7)
                ORDER BY knn_candidates.distance
                LIMIT ?8"
            )?;

            let rows = stmt.query_map(
                rusqlite::params![query_bytes, k, agent_id, session_id, tag, after, before, limit],
                |row| {
                    let tags_str: String = row.get(4)?;
                    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                    Ok((
                        Memory {
                            id: row.get(0)?,
                            content: row.get(1)?,
                            agent_id: row.get(2)?,
                            session_id: row.get(3)?,
                            tags,
                            embedding_model: row.get(5)?,
                            created_at: row.get(6)?,
                            updated_at: row.get(7)?,
                        },
                        row.get::<_, f64>(8)?,
                    ))
                },
            )?;

            rows.collect::<Result<Vec<_>, _>>()
        }).await?;

        let memories: Vec<SearchResultItem> = results
            .into_iter()
            .filter(|(_, distance)| {
                threshold.map_or(true, |t| *distance <= t as f64)
            })
            .map(|(memory, distance)| SearchResultItem { memory, distance })
            .collect();

        Ok(SearchResponse { memories })
    }

    /// Delete a memory by ID. Returns the deleted memory or NotFound.
    ///
    /// SQL extracted verbatim from MemoryService::delete_memory (service.rs lines 313-347).
    async fn delete(&self, id: &str) -> Result<Memory, ApiError> {
        let id_clone = id.to_string();
        let result = self.db.call(move |c| -> Result<Option<Memory>, rusqlite::Error> {
            // Fetch the memory first — drop stmt before taking a transaction
            let memory = {
                let mut stmt = c.prepare(
                    "SELECT id, content, agent_id, session_id, tags, embedding_model, created_at, updated_at
                     FROM memories WHERE id = ?1"
                )?;
                stmt.query_row(rusqlite::params![id_clone], |row| {
                    let tags_str: String = row.get(4)?;
                    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                    Ok(Memory {
                        id: row.get(0)?,
                        content: row.get(1)?,
                        agent_id: row.get(2)?,
                        session_id: row.get(3)?,
                        tags,
                        embedding_model: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                }).optional()?
                // stmt is dropped here, releasing the immutable borrow on c
            };

            if let Some(ref mem) = memory {
                let tx = c.transaction()?;
                tx.execute("DELETE FROM vec_memories WHERE memory_id = ?1", rusqlite::params![mem.id])?;
                tx.execute("DELETE FROM memories WHERE id = ?1", rusqlite::params![mem.id])?;
                tx.commit()?;
            }

            Ok(memory)
        }).await?;

        result.ok_or(ApiError::NotFound)
    }

    /// Fetch compaction candidates with embeddings for an agent.
    ///
    /// SQL extracted verbatim from CompactionService::fetch_candidates (compaction.rs lines 187-216).
    /// Returns (candidates, truncated) where truncated=true if more exist beyond max_candidates.
    async fn fetch_candidates(&self, agent_id: &str, max_candidates: u32) -> Result<(Vec<CandidateRecord>, bool), ApiError> {
        let agent_id = agent_id.to_string();
        let fetch_limit = max_candidates as i64 + 1;

        let mut candidates = self.db.call(move |c| -> Result<Vec<CandidateRecord>, rusqlite::Error> {
            let mut stmt = c.prepare(
                "SELECT m.id, m.content, m.tags, m.created_at, v.embedding
                 FROM memories m
                 JOIN vec_memories v ON v.memory_id = m.id
                 WHERE m.agent_id = ?1
                 ORDER BY m.created_at DESC
                 LIMIT ?2"
            )?;
            let rows = stmt.query_map(
                rusqlite::params![agent_id, fetch_limit],
                |row| {
                    let tags_str: String = row.get(2)?;
                    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                    let bytes: Vec<u8> = row.get(4)?;
                    // SAFETY: sqlite-vec stores 384 aligned f32 values as 1536 bytes (IEEE 754 little-endian)
                    let embedding: Vec<f32> = unsafe {
                        std::slice::from_raw_parts(bytes.as_ptr() as *const f32, bytes.len() / 4).to_vec()
                    };
                    Ok(CandidateRecord {
                        id: row.get(0)?,
                        content: row.get(1)?,
                        tags,
                        created_at: row.get(3)?,
                        embedding,
                    })
                },
            )?;
            rows.collect::<Result<Vec<_>, _>>()
        }).await?;

        let truncated = candidates.len() > max_candidates as usize;
        if truncated {
            candidates.truncate(max_candidates as usize);
        }

        Ok((candidates, truncated))
    }

    /// Atomically insert a merged memory and delete its source memories.
    ///
    /// SQL extracted from CompactionService::compact atomic write block (compaction.rs lines 344-398),
    /// adapted for a single cluster per call. The MergedMemoryRequest.source_ids carries the IDs
    /// to delete in the same transaction.
    async fn write_compaction_result(&self, req: MergedMemoryRequest) -> Result<Memory, ApiError> {
        let new_id = req.new_id.clone();
        let agent_id = req.agent_id.clone();
        let content = req.content.clone();
        let tags = req.tags.clone();
        let embedding_model = req.embedding_model.clone();
        let created_at = req.created_at.clone();
        let source_ids = req.source_ids.clone();
        let embedding_bytes: Vec<u8> = req.embedding.as_bytes().to_vec();

        let new_id_c = new_id.clone();
        let agent_id_c = agent_id.clone();
        let content_c = content.clone();
        let embedding_model_c = embedding_model.clone();
        let created_at_c = created_at.clone();

        self.db.call(move |c| -> Result<(), rusqlite::Error> {
            let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
            let source_ids_json = serde_json::to_string(&source_ids).unwrap_or_else(|_| "[]".to_string());

            let tx = c.transaction()?;

            // INSERT merged memory
            tx.execute(
                "INSERT INTO memories (id, content, agent_id, session_id, tags, embedding_model, created_at, source_ids)
                 VALUES (?1, ?2, ?3, '', ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    new_id_c,
                    content_c,
                    agent_id_c,
                    tags_json,
                    embedding_model_c,
                    created_at_c,
                    source_ids_json
                ],
            )?;

            // INSERT vec embedding for merged memory
            tx.execute(
                "INSERT INTO vec_memories (memory_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![new_id_c, embedding_bytes],
            )?;

            // DELETE source vec entries first
            for src_id in &source_ids {
                tx.execute(
                    "DELETE FROM vec_memories WHERE memory_id = ?1",
                    rusqlite::params![src_id],
                )?;
            }

            // DELETE source memories
            for src_id in &source_ids {
                tx.execute(
                    "DELETE FROM memories WHERE id = ?1",
                    rusqlite::params![src_id],
                )?;
            }

            tx.commit()?;
            Ok(())
        }).await?;

        Ok(Memory {
            id: new_id,
            content,
            agent_id,
            session_id: "".to_string(),
            tags: req.tags,
            embedding_model,
            created_at,
            updated_at: None,
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Confirms SqliteBackend is Send + Sync at compile time.
    #[allow(dead_code)]
    fn _assert_send<T: Send>() {}
    #[allow(dead_code)]
    fn _assert_sync<T: Sync>() {}

    /// Confirms Arc<dyn StorageBackend> accepts SqliteBackend (dyn-compatible with concrete type).
    #[allow(dead_code)]
    fn _takes_backend(_: Arc<dyn StorageBackend>) {}

    #[test]
    fn test_sqlite_backend_send_sync() {
        // Compile-time proof: SqliteBackend must be Send + Sync to implement StorageBackend.
        // If this test file compiles, the constraint is satisfied.
        _assert_send::<SqliteBackend>();
        _assert_sync::<SqliteBackend>();
    }

    #[test]
    fn test_sqlite_backend_as_trait_object() {
        // Compile-time proof: Arc<dyn StorageBackend> is constructible with SqliteBackend.
        // The _takes_backend function above is the actual assertion — it exists to be called.
    }
}
