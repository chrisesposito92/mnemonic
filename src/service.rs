use std::sync::Arc;
use crate::embedding::EmbeddingEngine;
use crate::error::ApiError;
use crate::storage::{StorageBackend, StoreRequest};

pub struct MemoryService {
    pub backend: Arc<dyn StorageBackend>,
    pub embedding: Arc<dyn EmbeddingEngine>,
    pub embedding_model: String,
}

impl MemoryService {
    pub fn new(
        backend: Arc<dyn StorageBackend>,
        embedding: Arc<dyn EmbeddingEngine>,
        embedding_model: String,
    ) -> Self {
        Self { backend, embedding, embedding_model }
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

#[derive(Debug, serde::Serialize)]
pub struct AgentStats {
    pub agent_id: String,
    pub memory_count: u64,
    pub last_active: String,  // ISO 8601 UTC, max created_at for that agent
}

#[derive(Debug, serde::Serialize)]
pub struct StatsResponse {
    pub agents: Vec<AgentStats>,
}

impl MemoryService {
    pub async fn create_memory(&self, req: CreateMemoryRequest) -> Result<Memory, ApiError> {
        // 1. Validate
        if req.content.trim().is_empty() {
            return Err(ApiError::BadRequest("content must not be empty".to_string()));
        }

        // 2. Embed
        let embedding = self.embedding.embed(&req.content).await?;

        // 3. Prepare values
        let id = uuid::Uuid::now_v7().to_string();
        let agent_id = req.agent_id.unwrap_or_default();
        let session_id = req.session_id.unwrap_or_default();
        let tags = req.tags.unwrap_or_default();
        let embedding_model = self.embedding_model.clone();

        // 4. Delegate to backend
        self.backend.store(StoreRequest {
            id,
            content: req.content,
            agent_id,
            session_id,
            tags,
            embedding_model,
            embedding,
        }).await
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

        // Embed the query
        let query_embedding = self.embedding.embed(&q).await?;

        self.backend.search(query_embedding, params).await
    }

    pub async fn list_memories(&self, params: ListParams) -> Result<ListResponse, ApiError> {
        self.backend.list(params).await
    }

    /// Fetches only the agent_id for a memory by ID.
    /// Returns Ok(None) if the memory does not exist.
    /// Used by delete_memory_handler for scope ownership verification (D-12).
    pub async fn get_memory_agent_id(&self, id: &str) -> Result<Option<String>, ApiError> {
        let memory = self.backend.get_by_id(id).await?;
        Ok(memory.map(|m| m.agent_id))
    }

    pub async fn delete_memory(&self, id: String) -> Result<Memory, ApiError> {
        self.backend.delete(&id).await
    }

    /// Returns per-agent memory counts. Used by GET /stats.
    pub async fn stats(&self) -> Result<StatsResponse, ApiError> {
        let agents = self.backend.stats().await?;
        Ok(StatsResponse { agents })
    }

    /// Returns stats filtered to a single agent. Used by GET /stats with scoped keys.
    pub async fn stats_for_agent(&self, agent_id: &str) -> Result<StatsResponse, ApiError> {
        let all = self.backend.stats().await?;
        let agents = all.into_iter().filter(|a| a.agent_id == agent_id).collect();
        Ok(StatsResponse { agents })
    }
}
