use async_trait::async_trait;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, VectorParamsBuilder, Distance,
    CreateFieldIndexCollectionBuilder, FieldType,
    PointStruct, UpsertPointsBuilder,
    GetPointsBuilder,
    DeletePointsBuilder, PointsIdsList,
    ScrollPointsBuilder,
    QueryPointsBuilder,
    CountPointsBuilder,
    Filter, Condition,
    DatetimeRange,
    RetrievedPoint,
};
use qdrant_client::qdrant::vector_output::Vector;
use qdrant_client::Payload;
use std::collections::HashMap;
use crate::config::Config;
use crate::error::{ApiError, MnemonicError, DbError};
use crate::storage::{StorageBackend, StoreRequest, CandidateRecord, MergedMemoryRequest};
use crate::service::{Memory, ListResponse, SearchResponse, SearchResultItem, ListParams, SearchParams};

// ──────────────────────────────────────────────────────────────────────────────
// QdrantBackend struct
// ──────────────────────────────────────────────────────────────────────────────

/// Qdrant implementation of StorageBackend.
///
/// Uses the qdrant-client gRPC crate to store and retrieve memories from a
/// Qdrant vector database. The client is stored directly in the struct — it
/// is already Send + Sync and handles connection pooling internally.
///
/// Collection `mnemonic_memories` is auto-created on first startup with:
/// - 384-dimension cosine distance vectors (matching all-MiniLM-L6-v2)
/// - Payload indexes on agent_id, session_id, and tags for efficient filtering
///
/// Distance semantics: Qdrant returns cosine similarity scores (higher=better).
/// All scores are converted via `1.0 - score` to match the trait's lower-is-better
/// distance contract (per D-08).
pub struct QdrantBackend {
    client: Qdrant,
    collection: String,
}

impl QdrantBackend {
    /// Create a new QdrantBackend by connecting to Qdrant at `config.qdrant_url`
    /// and ensuring the `mnemonic_memories` collection exists with the correct schema.
    pub async fn new(config: &Config) -> Result<Self, ApiError> {
        let url = config.qdrant_url.as_deref()
            .ok_or_else(|| ApiError::Internal(MnemonicError::Config(
                crate::error::ConfigError::Load(
                    "qdrant_url is required when storage_provider is \"qdrant\"".to_string()
                )
            )))?;

        let mut builder = Qdrant::from_url(url);
        if let Some(key) = &config.qdrant_api_key {
            builder = builder.api_key(key.as_str());
        }
        let client = builder.build()
            .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Open(e.to_string()))))?;

        let backend = Self {
            client,
            collection: "mnemonic_memories".to_string(),
        };
        backend.ensure_collection().await?;
        Ok(backend)
    }

    /// Ensure the `mnemonic_memories` collection exists with the correct schema.
    ///
    /// Idempotent — safe to call on every startup. If the collection already exists,
    /// this is a no-op. If it does not exist, creates it with:
    /// - 384-dimension cosine distance vectors
    /// - Payload indexes on agent_id, session_id, and tags (D-04)
    async fn ensure_collection(&self) -> Result<(), ApiError> {
        let exists = self.client.collection_exists(&self.collection).await
            .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

        if !exists {
            self.client.create_collection(
                CreateCollectionBuilder::new(&self.collection)
                    .vectors_config(VectorParamsBuilder::new(384, Distance::Cosine))
            ).await.map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

            // Create payload indexes for efficient filtering (D-04)
            for (field, ftype) in [
                ("agent_id", FieldType::Keyword),
                ("session_id", FieldType::Keyword),
                ("tags", FieldType::Keyword),
            ] {
                self.client.create_field_index(
                    CreateFieldIndexCollectionBuilder::new(&self.collection, field, ftype)
                ).await.map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;
            }
        }

        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Helper functions
// ──────────────────────────────────────────────────────────────────────────────

/// Convert a Qdrant cosine similarity score (higher=better) to a distance value
/// (lower=better) matching the StorageBackend trait contract (per D-08).
///
/// score=1.0 (identical vectors)  -> distance=0.0
/// score=0.0 (orthogonal vectors) -> distance=1.0
/// score=-1.0 (opposite vectors)  -> distance=2.0
fn score_to_distance(score: f32) -> f64 {
    1.0_f64 - score as f64
}

/// Build a Qdrant payload filter from optional filter parameters.
///
/// Returns None if all parameters are None (no filtering). Returns Some(Filter::must(...))
/// with all provided conditions combined as a logical AND.
///
/// The `tag` condition uses Qdrant's native array containment check — for keyword-indexed
/// array fields, `Condition::matches("tags", value)` checks if the array contains that value.
///
/// For `after`/`before` date range filtering, ISO 8601 UTC strings are parsed and converted
/// to prost_types::Timestamp for use with Qdrant's native datetime_range filter.
fn build_filter(
    agent_id: Option<&str>,
    session_id: Option<&str>,
    tag: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
) -> Option<Filter> {
    let mut conditions: Vec<Condition> = Vec::new();

    if let Some(id) = agent_id {
        conditions.push(Condition::matches("agent_id", id.to_string()));
    }
    if let Some(sid) = session_id {
        conditions.push(Condition::matches("session_id", sid.to_string()));
    }
    if let Some(t) = tag {
        conditions.push(Condition::matches("tags", t.to_string()));
    }
    // Date range filtering using Qdrant's native datetime_range with prost_types::Timestamp
    if after.is_some() || before.is_some() {
        fn parse_ts(s: &str) -> Option<prost_types::Timestamp> {
            let epoch = iso8601_to_epoch(s)?;
            Some(prost_types::Timestamp {
                seconds: epoch,
                nanos: 0,
            })
        }

        let range = DatetimeRange {
            gte: after.and_then(parse_ts),
            lt: before.and_then(parse_ts),
            ..Default::default()
        };
        conditions.push(Condition::datetime_range("created_at", range));
    }

    if conditions.is_empty() {
        None
    } else {
        Some(Filter::must(conditions))
    }
}

/// Parse an ISO 8601 UTC timestamp string to Unix epoch seconds.
/// Supports formats: "YYYY-MM-DDTHH:MM:SSZ", "YYYY-MM-DDTHH:MM:SS+00:00", "YYYY-MM-DD HH:MM:SS"
/// Returns None if the string cannot be parsed.
fn iso8601_to_epoch(s: &str) -> Option<i64> {
    let s = s.trim();

    let (date_part, time_part) = if let Some(idx) = s.find('T') {
        let time_raw = &s[idx + 1..];
        // Strip timezone suffix (Z, +00:00, -00:00)
        let time = time_raw
            .trim_end_matches('Z')
            .trim_end_matches("+00:00")
            .trim_end_matches("-00:00");
        (&s[..idx], time)
    } else if let Some(idx) = s.find(' ') {
        (&s[..idx], &s[idx + 1..])
    } else {
        return None;
    };

    let date_parts: Vec<&str> = date_part.splitn(3, '-').collect();
    if date_parts.len() != 3 {
        return None;
    }
    let year: i64 = date_parts[0].parse().ok()?;
    let month: i64 = date_parts[1].parse().ok()?;
    let day: i64 = date_parts[2].parse().ok()?;

    let time_parts: Vec<&str> = time_part.splitn(3, ':').collect();
    if time_parts.len() < 2 {
        return None;
    }
    let hour: i64 = time_parts[0].parse().ok()?;
    let min: i64 = time_parts[1].parse().ok()?;
    let sec: i64 = if time_parts.len() > 2 {
        // Handle fractional seconds by truncating
        let sec_str = time_parts[2].split('.').next().unwrap_or("0");
        sec_str.parse().ok()?
    } else {
        0
    };

    // Convert (year, month, day) to Julian Day Number, then to Unix epoch
    let a = (14 - month) / 12;
    let y = year + 4800 - a;
    let m = month + 12 * a - 3;
    let jdn = day + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32045;
    let unix_days = jdn - 2440588; // JDN for 1970-01-01
    Some(unix_days * 86400 + hour * 3600 + min * 60 + sec)
}

/// Format current time as ISO 8601 UTC string ("YYYY-MM-DDTHH:MM:SSZ").
fn now_iso8601() -> String {
    use std::time::SystemTime;
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs() as i64;

    // Convert Unix epoch seconds to calendar date and time
    let time_of_day = secs % 86400;
    let days = secs / 86400;
    let hour = time_of_day / 3600;
    let min = (time_of_day % 3600) / 60;
    let sec = time_of_day % 60;

    // Convert days-since-epoch to calendar date using Julian Day Number algorithm
    let jdn = days + 2440588;
    let a = jdn + 32044;
    let b = (4 * a + 3) / 146097;
    let c = a - (146097 * b) / 4;
    let d2 = (4 * c + 3) / 1461;
    let e = c - (1461 * d2) / 4;
    let m = (5 * e + 2) / 153;
    let day = e - (153 * m + 2) / 5 + 1;
    let month = m + 3 - 12 * (m / 10);
    let year = 100 * b + d2 - 4800 + m / 10;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, min, sec
    )
}

/// Extract a string value from a Qdrant point payload.
///
/// `as_str()` on a `Value` returns `Option<&String>` via the extract! macro in qdrant-client.
fn get_payload_string(
    payload: &HashMap<String, qdrant_client::qdrant::Value>,
    key: &str,
) -> Option<String> {
    payload.get(key)?.as_str().map(|s| s.clone())
}

/// Extract a list of strings from a Qdrant point payload.
///
/// `as_list()` on a `Value` returns `Option<&[Value]>` — a slice of Value items.
fn get_payload_string_list(
    payload: &HashMap<String, qdrant_client::qdrant::Value>,
    key: &str,
) -> Vec<String> {
    let Some(value) = payload.get(key) else {
        return Vec::new();
    };
    let Some(items) = value.as_list() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|v| v.as_str().map(|s: &String| s.clone()))
        .collect()
}

/// Extract the default (unnamed) dense vector from a RetrievedPoint as Vec<f32>.
///
/// RetrievedPoint.vectors is Option<VectorsOutput>. With with_vectors(true), Qdrant
/// returns the default vector as VectorsOutput::VectorsOptions::Vector(VectorOutput).
/// VectorOutput.into_vector() converts to the Vector enum; we match Dense to get Vec<f32>.
///
/// Returns ApiError::Internal if vectors are missing (should not happen when
/// with_vectors(true) was specified on the scroll request).
fn extract_vector_from_point(pt: &RetrievedPoint) -> Result<Vec<f32>, ApiError> {
    let vectors_output = pt.vectors.as_ref()
        .ok_or_else(|| ApiError::Internal(MnemonicError::Db(DbError::Query(
            "missing vectors in Qdrant point (with_vectors was true)".to_string()
        ))))?;

    let vector = vectors_output.get_vector()
        .ok_or_else(|| ApiError::Internal(MnemonicError::Db(DbError::Query(
            "could not extract default vector from VectorsOutput".to_string()
        ))))?;

    match vector {
        Vector::Dense(dense) => Ok(dense.data),
        _ => Err(ApiError::Internal(MnemonicError::Db(DbError::Query(
            "expected dense vector but got sparse or multi-dense".to_string()
        )))),
    }
}

/// Convert a Qdrant point payload to a Memory struct.
///
/// All required fields are extracted from the payload. Missing required fields
/// return an ApiError::Internal.
fn point_to_memory(
    payload: &HashMap<String, qdrant_client::qdrant::Value>,
) -> Result<Memory, ApiError> {
    let id = get_payload_string(payload, "id")
        .ok_or_else(|| ApiError::Internal(MnemonicError::Db(DbError::Query(
            "missing field 'id' in Qdrant payload".to_string()
        ))))?;
    let content = get_payload_string(payload, "content")
        .ok_or_else(|| ApiError::Internal(MnemonicError::Db(DbError::Query(
            "missing field 'content' in Qdrant payload".to_string()
        ))))?;
    let agent_id = get_payload_string(payload, "agent_id")
        .ok_or_else(|| ApiError::Internal(MnemonicError::Db(DbError::Query(
            "missing field 'agent_id' in Qdrant payload".to_string()
        ))))?;
    let session_id = get_payload_string(payload, "session_id")
        .ok_or_else(|| ApiError::Internal(MnemonicError::Db(DbError::Query(
            "missing field 'session_id' in Qdrant payload".to_string()
        ))))?;
    let embedding_model = get_payload_string(payload, "embedding_model")
        .ok_or_else(|| ApiError::Internal(MnemonicError::Db(DbError::Query(
            "missing field 'embedding_model' in Qdrant payload".to_string()
        ))))?;
    let created_at = get_payload_string(payload, "created_at")
        .ok_or_else(|| ApiError::Internal(MnemonicError::Db(DbError::Query(
            "missing field 'created_at' in Qdrant payload".to_string()
        ))))?;
    let tags = get_payload_string_list(payload, "tags");
    let updated_at = get_payload_string(payload, "updated_at"); // None if null/missing

    Ok(Memory {
        id,
        content,
        agent_id,
        session_id,
        tags,
        embedding_model,
        created_at,
        updated_at,
    })
}

// ──────────────────────────────────────────────────────────────────────────────
// StorageBackend implementation
// ──────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl StorageBackend for QdrantBackend {
    /// Store a memory with a pre-computed embedding.
    ///
    /// Upserts a Qdrant point with the UUID string ID, embedding vector,
    /// and all memory fields as payload. Returns the stored Memory.
    async fn store(&self, req: StoreRequest) -> Result<Memory, ApiError> {
        let created_at = now_iso8601();

        let payload: Payload = serde_json::json!({
            "id": req.id,
            "content": req.content,
            "agent_id": req.agent_id,
            "session_id": req.session_id,
            "tags": req.tags,
            "embedding_model": req.embedding_model,
            "created_at": created_at,
            "updated_at": serde_json::Value::Null,
        })
        .try_into()
        .map_err(|e: qdrant_client::QdrantError| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

        let point = PointStruct::new(req.id.clone(), req.embedding, payload);

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection, vec![point]))
            .await
            .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

        Ok(Memory {
            id: req.id,
            content: req.content,
            agent_id: req.agent_id,
            session_id: req.session_id,
            tags: req.tags,
            embedding_model: req.embedding_model,
            created_at,
            updated_at: None,
        })
    }

    /// Get a single memory by ID. Returns None if not found.
    async fn get_by_id(&self, id: &str) -> Result<Option<Memory>, ApiError> {
        let response = self.client
            .get_points(
                GetPointsBuilder::new(&self.collection, vec![id.to_string().into()])
                    .with_payload(true)
                    .with_vectors(false),
            )
            .await
            .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

        match response.result.into_iter().next() {
            Some(point) => Ok(Some(point_to_memory(&point.payload)?)),
            None => Ok(None),
        }
    }

    /// List memories with filtering and pagination.
    ///
    /// Uses Qdrant's scroll API with payload filters, then sorts client-side by
    /// `created_at DESC` (per D-15). Qdrant scroll does not natively sort by payload,
    /// so sorting is done after retrieval. For typical memory counts per agent (<10K)
    /// this is acceptable performance.
    ///
    /// Pagination: Qdrant scroll is cursor-based, not integer-offset. For integer
    /// offset/limit pagination, we fetch (offset + limit + 1) points, sort, then
    /// slice. A separate count query provides accurate total (per D-14).
    async fn list(&self, params: ListParams) -> Result<ListResponse, ApiError> {
        let limit = params.limit.unwrap_or(20).min(100) as usize;
        let offset = params.offset.unwrap_or(0) as usize;

        // Build filter for scroll and count queries
        let filter = build_filter(
            params.agent_id.as_deref(),
            params.session_id.as_deref(),
            params.tag.as_deref(),
            params.after.as_deref(),
            params.before.as_deref(),
        );
        let filter_for_count = build_filter(
            params.agent_id.as_deref(),
            params.session_id.as_deref(),
            params.tag.as_deref(),
            params.after.as_deref(),
            params.before.as_deref(),
        );

        // Fetch offset+limit+1 to detect if more exist (used for heuristic total)
        let fetch_limit = (offset + limit + 1) as u32;

        let mut scroll_builder = ScrollPointsBuilder::new(&self.collection)
            .limit(fetch_limit)
            .with_payload(true)
            .with_vectors(false);

        if let Some(f) = filter {
            scroll_builder = scroll_builder.filter(f);
        }

        let scroll_result = self.client.scroll(scroll_builder)
            .await
            .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

        let mut points = scroll_result.result;

        // Client-side sort by created_at DESC (D-15)
        // ISO 8601 strings sort lexicographically correctly
        points.sort_by(|a, b| {
            let ta = get_payload_string(&a.payload, "created_at").unwrap_or_default();
            let tb = get_payload_string(&b.payload, "created_at").unwrap_or_default();
            tb.cmp(&ta)
        });

        // Apply offset/limit pagination
        let paged: Vec<RetrievedPoint> = points.into_iter()
            .skip(offset)
            .take(limit)
            .collect();

        let memories: Vec<Memory> = paged.into_iter()
            .map(|pt| point_to_memory(&pt.payload))
            .collect::<Result<Vec<_>, _>>()?;

        // Accurate total count via Qdrant count API (exact=true)
        let total = {
            let mut count_builder = CountPointsBuilder::new(&self.collection)
                .exact(true);
            if let Some(f) = filter_for_count {
                count_builder = count_builder.filter(f);
            }
            let count_response = self.client.count(count_builder)
                .await
                .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;
            count_response.result.map(|r| r.count).unwrap_or(0)
        };

        Ok(ListResponse { memories, total })
    }

    /// Semantic search using a pre-computed query embedding.
    ///
    /// Uses Qdrant's query API with the embedding as a nearest-neighbour query.
    /// Filters are applied natively by Qdrant during search — no over-fetch needed
    /// (per D-17). Score-to-distance conversion is applied after results are returned,
    /// then threshold filtering is applied on the lower-is-better distance (per D-18).
    async fn search(
        &self,
        embedding: Vec<f32>,
        params: SearchParams,
    ) -> Result<SearchResponse, ApiError> {
        let limit = params.limit.unwrap_or(10).min(100) as u64;

        let filter = build_filter(
            params.agent_id.as_deref(),
            params.session_id.as_deref(),
            params.tag.as_deref(),
            params.after.as_deref(),
            params.before.as_deref(),
        );

        // D-17: Qdrant applies filters natively during search -- no over-fetch needed
        let mut query_builder = QueryPointsBuilder::new(&self.collection)
            .query(embedding)
            .limit(limit)
            .with_payload(true);

        if let Some(f) = filter {
            query_builder = query_builder.filter(f);
        }

        let response = self.client.query(query_builder)
            .await
            .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

        // D-08, D-09: Convert score to lower-is-better distance
        // D-18: Apply threshold filter after conversion
        let threshold = params.threshold;
        let memories: Vec<SearchResultItem> = response.result
            .into_iter()
            .map(|pt| {
                let distance = score_to_distance(pt.score);
                let memory = point_to_memory(&pt.payload)?;
                Ok(SearchResultItem { memory, distance })
            })
            .collect::<Result<Vec<_>, ApiError>>()?
            .into_iter()
            .filter(|item| {
                threshold.map_or(true, |t| item.distance <= t as f64)
            })
            .collect();

        Ok(SearchResponse { memories })
    }

    /// Delete a memory by ID. Returns the deleted memory or NotFound.
    ///
    /// Fetches the memory first to return it, then deletes the point by UUID string ID.
    async fn delete(&self, id: &str) -> Result<Memory, ApiError> {
        // Fetch first to return the deleted memory (per D-07)
        let memory = self
            .get_by_id(id)
            .await?
            .ok_or(ApiError::NotFound)?;

        // Delete the point by UUID string ID
        self.client
            .delete_points(
                DeletePointsBuilder::new(&self.collection)
                    .points(PointsIdsList {
                        ids: vec![id.to_string().into()],
                    }),
            )
            .await
            .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

        Ok(memory)
    }

    /// Fetch compaction candidates with embeddings for an agent.
    ///
    /// Uses Qdrant scroll with agent_id filter and with_vectors(true) to retrieve
    /// points with their embeddings for compaction (per D-19). Over-fetches by 1
    /// to detect truncation (per D-20). Sorts client-side by created_at DESC (per D-21).
    async fn fetch_candidates(
        &self,
        agent_id: &str,
        max_candidates: u32,
    ) -> Result<(Vec<CandidateRecord>, bool), ApiError> {
        let fetch_limit = max_candidates + 1; // D-20: over-fetch by 1 to detect truncation

        let filter = Filter::must(vec![
            Condition::matches("agent_id", agent_id.to_string()),
        ]);

        // D-19: scroll with agent_id filter and with_vectors(true) — embeddings required
        let scroll_result = self.client.scroll(
            ScrollPointsBuilder::new(&self.collection)
                .filter(filter)
                .limit(fetch_limit)
                .with_payload(true)
                .with_vectors(true) // CRITICAL: need embeddings for compaction
        ).await.map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

        let mut points = scroll_result.result;

        // D-21: Sort by created_at DESC client-side (matching SqliteBackend ORDER BY created_at DESC)
        points.sort_by(|a, b| {
            let ta = get_payload_string(&a.payload, "created_at").unwrap_or_default();
            let tb = get_payload_string(&b.payload, "created_at").unwrap_or_default();
            tb.cmp(&ta)
        });

        let truncated = points.len() > max_candidates as usize;

        // Extract CandidateRecords — take at most max_candidates
        let candidates: Vec<CandidateRecord> = points.into_iter()
            .take(max_candidates as usize)
            .map(|pt| {
                let id = get_payload_string(&pt.payload, "id")
                    .ok_or_else(|| ApiError::Internal(MnemonicError::Db(
                        DbError::Query("missing id in Qdrant payload".to_string())
                    )))?;
                let content = get_payload_string(&pt.payload, "content")
                    .ok_or_else(|| ApiError::Internal(MnemonicError::Db(
                        DbError::Query("missing content in Qdrant payload".to_string())
                    )))?;
                let tags = get_payload_string_list(&pt.payload, "tags");
                let created_at = get_payload_string(&pt.payload, "created_at")
                    .ok_or_else(|| ApiError::Internal(MnemonicError::Db(
                        DbError::Query("missing created_at in Qdrant payload".to_string())
                    )))?;
                let embedding = extract_vector_from_point(&pt)?;

                Ok(CandidateRecord { id, content, tags, created_at, embedding })
            })
            .collect::<Result<Vec<_>, ApiError>>()?;

        Ok((candidates, truncated))
    }

    /// Write compaction result: upsert merged memory, then delete source memories.
    ///
    /// IMPORTANT: This is NOT atomic. Qdrant has no multi-operation transactions.
    /// Each step (upsert, delete) is a separate API call (per D-11).
    ///
    /// Order: upsert first, delete second (per D-10).
    /// Rationale: if deletion fails after upsert, we have a duplicate (recoverable
    /// via next compaction run) rather than data loss (irrecoverable).
    ///
    /// On partial failure (upsert succeeds, delete fails): return the error. The merged
    /// memory exists alongside sources. Next compaction run will detect duplicates
    /// but no data will be lost. A warning is logged (per D-12).
    async fn write_compaction_result(
        &self,
        req: MergedMemoryRequest,
    ) -> Result<Memory, ApiError> {
        // Step 1: Upsert the merged memory point (D-10: upsert first)
        let payload: Payload = serde_json::json!({
            "id": req.new_id,
            "content": req.content,
            "agent_id": req.agent_id,
            "session_id": "",
            "tags": req.tags,
            "embedding_model": req.embedding_model,
            "created_at": req.created_at,
            "updated_at": serde_json::Value::Null,
        }).try_into().map_err(|e: qdrant_client::QdrantError|
            ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string())))
        )?;

        let point = PointStruct::new(req.new_id.clone(), req.embedding, payload);

        self.client.upsert_points(
            UpsertPointsBuilder::new(&self.collection, vec![point])
        ).await.map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(
            format!("compaction upsert failed: {}", e)
        ))))?;

        // Step 2: Delete source points (D-10: delete second, D-11: separate API call)
        if !req.source_ids.is_empty() {
            let ids_to_delete: Vec<qdrant_client::qdrant::PointId> = req.source_ids.iter()
                .map(|id| id.to_string().into())
                .collect();

            self.client.delete_points(
                DeletePointsBuilder::new(&self.collection)
                    .points(PointsIdsList { ids: ids_to_delete })
            ).await.map_err(|e| {
                // D-12: Log warning on partial failure (upsert succeeded, delete failed)
                tracing::warn!(
                    merged_id = %req.new_id,
                    source_count = req.source_ids.len(),
                    error = %e,
                    "compaction delete failed after successful upsert -- merged memory exists but sources remain"
                );
                ApiError::Internal(MnemonicError::Db(DbError::Query(
                    format!("compaction delete failed after upsert: {}", e)
                )))
            })?;
        }

        Ok(Memory {
            id: req.new_id,
            content: req.content,
            agent_id: req.agent_id,
            session_id: "".to_string(),
            tags: req.tags,
            embedding_model: req.embedding_model,
            created_at: req.created_at,
            updated_at: None,
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Unit tests — no live Qdrant instance required
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // score_to_distance tests

    #[test]
    fn test_score_to_distance_identical() {
        assert!((score_to_distance(1.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_score_to_distance_opposite() {
        assert!((score_to_distance(-1.0) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_score_to_distance_midpoint() {
        assert!((score_to_distance(0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_score_to_distance_typical_similar() {
        // score=0.85 -> distance=0.15
        assert!((score_to_distance(0.85) - 0.15).abs() < 1e-6);
    }

    // build_filter tests

    #[test]
    fn test_build_filter_none_when_no_params() {
        let filter = build_filter(None, None, None, None, None);
        assert!(filter.is_none());
    }

    #[test]
    fn test_build_filter_agent_id_only() {
        let filter = build_filter(Some("agent-1"), None, None, None, None);
        assert!(filter.is_some());
    }

    #[test]
    fn test_build_filter_all_params() {
        let filter = build_filter(
            Some("agent-1"),
            Some("session-1"),
            Some("tag-1"),
            Some("2026-01-01T00:00:00Z"),
            Some("2026-12-31T23:59:59Z"),
        );
        assert!(filter.is_some());
    }

    // now_iso8601 tests

    #[test]
    fn test_now_iso8601_format() {
        let ts = now_iso8601();
        // Should match YYYY-MM-DDTHH:MM:SSZ format (length = 20)
        assert_eq!(ts.len(), 20, "Expected format YYYY-MM-DDTHH:MM:SSZ, got: {}", ts);
        assert!(ts.ends_with('Z'), "Should end with Z, got: {}", ts);
        assert!(ts.contains('T'), "Should contain T separator, got: {}", ts);
    }

    // iso8601_to_epoch tests

    #[test]
    fn test_iso8601_epoch_origin() {
        // 1970-01-01T00:00:00Z should be 0
        let epoch = iso8601_to_epoch("1970-01-01T00:00:00Z");
        assert_eq!(epoch, Some(0), "Unix epoch start should be 0");
    }

    #[test]
    fn test_iso8601_epoch_known_date() {
        // 2026-01-01T00:00:00Z — epoch should be in a reasonable range for 2026
        let epoch = iso8601_to_epoch("2026-01-01T00:00:00Z");
        assert!(epoch.is_some(), "Should parse valid ISO 8601 date");
        let e = epoch.unwrap();
        // 2026 is between 2020 (1577836800) and 2030 (1893456000)
        assert!(
            e > 1_577_836_800 && e < 1_893_456_000,
            "Epoch {} not in expected range for 2026",
            e
        );
    }

    // ── Nyquist gap tests (QDRT-01, QDRT-04) ──────────────────────────────────

    /// Helper: construct a qdrant Value with a string kind.
    fn string_value(s: &str) -> qdrant_client::qdrant::Value {
        use qdrant_client::qdrant::value::Kind;
        qdrant_client::qdrant::Value {
            kind: Some(Kind::StringValue(s.to_string())),
        }
    }

    /// Helper: construct a qdrant Value with a list of string values.
    fn list_value(items: &[&str]) -> qdrant_client::qdrant::Value {
        use qdrant_client::qdrant::{ListValue, value::Kind};
        qdrant_client::qdrant::Value {
            kind: Some(Kind::ListValue(ListValue {
                values: items
                    .iter()
                    .map(|s| qdrant_client::qdrant::Value {
                        kind: Some(Kind::StringValue(s.to_string())),
                    })
                    .collect(),
            })),
        }
    }

    /// Helper: build a complete valid payload HashMap for point_to_memory.
    fn valid_payload(tags: &[&str]) -> HashMap<String, qdrant_client::qdrant::Value> {
        let mut map = HashMap::new();
        map.insert("id".to_string(), string_value("test-id-001"));
        map.insert("content".to_string(), string_value("Hello, world!"));
        map.insert("agent_id".to_string(), string_value("agent-abc"));
        map.insert("session_id".to_string(), string_value("session-xyz"));
        map.insert("embedding_model".to_string(), string_value("all-MiniLM-L6-v2"));
        map.insert("created_at".to_string(), string_value("2026-01-15T10:00:00Z"));
        map.insert("tags".to_string(), list_value(tags));
        map
    }

    // QDRT-01: point_to_memory extracts all fields from a valid payload
    #[test]
    fn test_point_to_memory_valid_payload() {
        let payload = valid_payload(&["tag1", "tag2"]);
        let result = point_to_memory(&payload);
        assert!(result.is_ok(), "Expected Ok but got: {:?}", result);
        let memory = result.unwrap();
        assert_eq!(memory.id, "test-id-001");
        assert_eq!(memory.content, "Hello, world!");
        assert_eq!(memory.agent_id, "agent-abc");
        assert_eq!(memory.session_id, "session-xyz");
        assert_eq!(memory.embedding_model, "all-MiniLM-L6-v2");
        assert_eq!(memory.created_at, "2026-01-15T10:00:00Z");
        assert_eq!(memory.tags, vec!["tag1".to_string(), "tag2".to_string()]);
    }

    // QDRT-01: point_to_memory returns Err when a required field is missing
    #[test]
    fn test_point_to_memory_missing_required_field() {
        let mut payload = valid_payload(&[]);
        // Remove the required "content" field
        payload.remove("content");
        let result = point_to_memory(&payload);
        assert!(
            result.is_err(),
            "Expected Err for missing 'content' field, got Ok"
        );
    }

    // QDRT-01: get_payload_string_list returns correct Vec<String> for a list payload
    #[test]
    fn test_get_payload_string_list_with_values() {
        let mut payload: HashMap<String, qdrant_client::qdrant::Value> = HashMap::new();
        payload.insert("tags".to_string(), list_value(&["alpha", "beta", "gamma"]));
        let result = get_payload_string_list(&payload, "tags");
        assert_eq!(
            result,
            vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
            "Expected three tag strings from list value"
        );
    }

    // QDRT-04: build_filter with session_id only produces a Some filter
    #[test]
    fn test_build_filter_session_id_only() {
        let filter = build_filter(None, Some("session-1"), None, None, None);
        assert!(
            filter.is_some(),
            "Expected Some filter when session_id is provided"
        );
    }

    // QDRT-04: build_filter with tag only produces a Some filter
    #[test]
    fn test_build_filter_tag_only() {
        let filter = build_filter(None, None, Some("important"), None, None);
        assert!(
            filter.is_some(),
            "Expected Some filter when tag is provided"
        );
    }

    // QDRT-04: build_filter with date range only produces a Some filter
    #[test]
    fn test_build_filter_date_range_only() {
        let filter = build_filter(
            None,
            None,
            None,
            Some("2026-01-01T00:00:00Z"),
            Some("2026-12-31T23:59:59Z"),
        );
        assert!(
            filter.is_some(),
            "Expected Some filter when date range is provided"
        );
    }
}
