use std::sync::Arc;
use tokio_rusqlite::Connection;
use rusqlite::OptionalExtension;
use zerocopy::IntoBytes;
use crate::embedding::EmbeddingEngine;
use crate::error::ApiError;

pub struct MemoryService {
    pub db: Arc<Connection>,
    pub embedding: Arc<dyn EmbeddingEngine>,
    pub embedding_model: String,
}

impl MemoryService {
    pub fn new(
        db: Arc<Connection>,
        embedding: Arc<dyn EmbeddingEngine>,
        embedding_model: String,
    ) -> Self {
        Self { db, embedding, embedding_model }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateMemoryRequest {
    pub content: String,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub tag: Option<String>,
    pub limit: Option<u32>,
    pub threshold: Option<f32>,
    pub after: Option<String>,
    pub before: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ListParams {
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub tag: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub agent_id: String,
    pub session_id: String,
    pub tags: Vec<String>,
    pub embedding_model: String,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct SearchResult {
    pub memory: Memory,
    pub distance: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct ListResponse {
    pub memories: Vec<Memory>,
    pub total: u64,
}

#[derive(Debug, serde::Serialize)]
pub struct SearchResponse {
    pub memories: Vec<SearchResultItem>,
}

#[derive(Debug, serde::Serialize)]
pub struct SearchResultItem {
    #[serde(flatten)]
    pub memory: Memory,
    pub distance: f64,
}

impl MemoryService {
    pub async fn create_memory(&self, req: CreateMemoryRequest) -> Result<Memory, ApiError> {
        // 1. Validate
        if req.content.trim().is_empty() {
            return Err(ApiError::BadRequest("content must not be empty".to_string()));
        }

        // 2. Embed
        let embedding = self.embedding.embed(&req.content).await?;
        let embedding_bytes: Vec<u8> = embedding.as_bytes().to_vec();

        // 3. Prepare values
        let id = uuid::Uuid::now_v7().to_string();
        let agent_id = req.agent_id.unwrap_or_default();
        let session_id = req.session_id.unwrap_or_default();
        let tags = req.tags.unwrap_or_default();
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
        let embedding_model = self.embedding_model.clone();

        // 4. Atomic dual-table insert
        let id_clone = id.clone();
        let content_clone = req.content.clone();
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
            content: req.content,
            agent_id,
            session_id,
            tags,
            embedding_model,
            created_at,
            updated_at: None,
        })
    }

    pub async fn search_memories(&self, params: SearchParams) -> Result<SearchResponse, ApiError> {
        let q = params.q
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_string();
        if q.is_empty() {
            return Err(ApiError::BadRequest("q parameter is required".to_string()));
        }

        let limit = params.limit.unwrap_or(10).min(100) as i64;
        let has_filters = params.agent_id.is_some() || params.session_id.is_some();
        let k = if has_filters { (limit * 10).min(1000) } else { limit };

        // Embed the query
        let query_embedding = self.embedding.embed(&q).await?;
        let query_bytes: Vec<u8> = query_embedding.as_bytes().to_vec();

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

    pub async fn list_memories(&self, params: ListParams) -> Result<ListResponse, ApiError> {
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

    pub async fn delete_memory(&self, id: String) -> Result<Memory, ApiError> {
        let id_clone = id.clone();
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
}
