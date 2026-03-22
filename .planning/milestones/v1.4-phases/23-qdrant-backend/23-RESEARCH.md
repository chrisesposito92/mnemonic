# Phase 23: Qdrant Backend - Research

**Researched:** 2026-03-21
**Domain:** qdrant-client 1.x Rust gRPC crate — collection management, vector search, payload filtering, scroll API
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Single collection named `mnemonic_memories` — all agents share one collection, isolated by `agent_id` payload filter
- **D-02:** Vector config: dimension 384 (all-MiniLM-L6-v2), cosine distance metric. User switching to OpenAI (1536-dim) must recreate collection.
- **D-03:** Payload fields per point: `id` (String), `content` (String), `agent_id` (String, indexed), `session_id` (String, indexed), `tags` (String[], keyword-indexed), `embedding_model` (String), `created_at` (String, ISO 8601), `updated_at` (String or null)
- **D-04:** Payload indexes on `agent_id`, `session_id`, `tags` at collection creation time
- **D-05:** Collection auto-creation: `QdrantBackend::new()` checks existence and creates if absent (idempotent, safe on every startup)
- **D-06:** Qdrant point IDs use UUID string format directly — UUID v7 IDs are valid UUIDs, map 1:1
- **D-07:** `get_by_id` and `delete` use point ID lookup by string ID. No secondary index needed.
- **D-08:** Score-to-distance: `distance = 1.0 - score` (Qdrant cosine similarity is higher-is-better; trait contract is lower-is-better)
- **D-09:** Threshold comparison applied after score-to-distance conversion in `search()`, same as SqliteBackend
- **D-10:** `write_compaction_result` order: upsert merged point first, then delete source points
- **D-11:** Each step (upsert, delete) is a separate Qdrant API call — no multi-operation transaction
- **D-12:** On partial failure: return error (merged exists but sources remain). Next compaction handles duplicates. Log warning.
- **D-13:** `list()` uses Qdrant scroll API with payload filters for agent_id, session_id, tag, date range
- **D-14:** Pagination via scroll offset (PointId-based). Integer offset/limit: scroll and skip. Acceptable for typical page sizes.
- **D-15:** Ordering: `list()` results ordered by `created_at DESC`. Qdrant scroll doesn't natively sort by payload — sort client-side. Acceptable for <10K memories/agent.
- **D-16:** `search()` uses Qdrant query API with pre-computed embedding vector and payload filters
- **D-17:** SQLite 10x over-fetch CTE pattern NOT needed — Qdrant applies filters during search natively
- **D-18:** `threshold` filtering applied after score-to-distance conversion, same as SqliteBackend
- **D-19:** `fetch_candidates` uses scroll with `agent_id` filter and `with_vectors: true`
- **D-20:** Limit to `max_candidates + 1` (over-fetch-by-one pattern) to detect truncation
- **D-21:** Sort by `created_at DESC` client-side after scroll retrieval
- **D-22:** New file `src/storage/qdrant.rs` behind `#[cfg(feature = "backend-qdrant")]`
- **D-23:** `src/storage/mod.rs` conditionally declares module and re-exports
- **D-24:** `Cargo.toml` feature `backend-qdrant` adds `qdrant-client = { version = "1", optional = true }`
- **D-25:** Wire `QdrantBackend::new(&config).await?` into `create_backend()` "qdrant" arm, replacing `todo!()`
- **D-26:** `QdrantBackend::new(config: &Config)` reads `qdrant_url`, optional `qdrant_api_key`, constructs client, verifies connectivity, ensures collection exists
- **D-27:** Store `QdrantClient` (`Qdrant` struct) directly — already `Send + Sync`, handles connection pooling internally
- **D-28:** Unit tests in `qdrant.rs` test helpers (score conversion, payload construction, filter building) without live instance
- **D-29:** Integration tests behind `#[cfg(all(test, feature = "backend-qdrant"))]` require live Qdrant. Run: `docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant && cargo test --features backend-qdrant`
- **D-30:** Existing 273 tests must pass unchanged without `--features backend-qdrant`

### Claude's Discretion

- Exact qdrant-client version (latest stable 1.x)
- Internal helper function decomposition within QdrantBackend
- Error message wording for Qdrant connection failures
- Whether to add a `collection_name` config field or hardcode `mnemonic_memories`
- Payload serialization approach (json_value vs typed fields)
- Whether scroll-based list() needs a practical limit warning in docs

### Deferred Ideas (OUT OF SCOPE)

- Collection name as config field (`qdrant_collection`) — hardcode `mnemonic_memories`
- Vector dimension auto-detection from first stored memory
- Qdrant API key rotation/refresh
- Qdrant cluster mode / distributed deployment docs
- Backend health ping in `/health` endpoint
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| QDRT-01 | QdrantBackend implements StorageBackend using qdrant-client gRPC, feature-gated behind backend-qdrant | qdrant-client 1.17.0 API verified; all 7 trait methods have direct mappings to Qdrant operations |
| QDRT-02 | Qdrant score (higher=better) normalized to distance (lower=better) matching StorageBackend contract | Confirmed: Qdrant cosine returns f32 score in [−1.0, 1.0]; `1.0 - score` converts to lower-is-better distance |
| QDRT-03 | Compaction works on Qdrant with documented non-transactional semantics (separate delete+upsert) | Confirmed: upsert_points then delete_points as separate calls; Qdrant has no cross-operation transactions |
| QDRT-04 | Multi-agent namespace isolation via Qdrant payload filtering on agent_id | Confirmed: `Filter::must([Condition::matches("agent_id", agent_id)])` applied to all operations |
</phase_requirements>

---

## Summary

Phase 23 implements `QdrantBackend` — a `StorageBackend` trait implementation that delegates all 7 memory operations to a Qdrant instance via the `qdrant-client` 1.17.0 gRPC crate. The implementation is entirely new code in `src/storage/qdrant.rs`, conditionally compiled behind `#[cfg(feature = "backend-qdrant")]`.

The `qdrant-client` crate (latest stable: 1.17.0, published 2026-02-20) provides a clean async builder-pattern API. All required operations — collection management, point upsert/get/delete, vector search via `query()`, scroll for pagination and candidate fetching, and payload index creation — are directly available and well-documented. The crate is already tokio-async, uses `prost-types` for timestamps, and the `Qdrant` struct is `Send + Sync`, satisfying the `Arc<dyn StorageBackend>` requirement without additional wrapping.

Key implementation concerns: (1) UUID string to `PointId` conversion (confirmed: UUID strings are valid PointId via Qdrant's type system), (2) score-to-distance normalization (`distance = 1.0 - score` for cosine), (3) client-side sorting for `list()` since Qdrant scroll has no native payload sort, and (4) non-transactional compaction semantics (upsert-first, delete-second, document clearly in code).

**Primary recommendation:** Use `qdrant-client = { version = "1", optional = true }` with the builder-pattern API throughout. Use `serde_json::json!(...).try_into()` for `Payload` construction. Use `Condition::datetime_range` with `prost_types::Timestamp` for date range filtering.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| qdrant-client | 1.17.0 | gRPC client for all Qdrant operations | Official Qdrant Rust client, actively maintained, builder-pattern API |

**Version verified:** `cargo search qdrant-client` and crates.io API confirmed 1.17.0 as latest stable, published 2026-02-20.

### Supporting (transitive — no extra Cargo.toml entries needed)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| prost-types | ^0.13.3 | `Timestamp` type for datetime range filtering | Pulled in by qdrant-client; use `prost_types::Timestamp` directly |
| tonic | ^0.12.3 | gRPC transport (TLS, compression) | Transparent via qdrant-client; no direct usage needed |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| qdrant-client gRPC | qdrant REST via reqwest | REST is simpler but gRPC is more efficient and is the official client |
| `query()` API | `search_points()` API | `query()` is the modern unified API; `search_points()` is the older method — both work, prefer `query()` |
| `Condition::datetime_range` | String comparison in payload filter | datetime_range is native Qdrant; string comparison would require exact ISO format ordering which is fragile |

**Installation:**
```toml
# In Cargo.toml [features] section — backend-qdrant already declared:
backend-qdrant = ["dep:qdrant-client"]  # Change from backend-qdrant = []

# In [dependencies]:
qdrant-client = { version = "1", optional = true }
```

**Note:** The current `backend-qdrant = []` feature declaration must be updated to `backend-qdrant = ["dep:qdrant-client"]` to link the optional dependency to the feature. The optional dependency line goes in `[dependencies]` without `features = [...]` gating.

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── storage/
│   ├── mod.rs          # StorageBackend trait + create_backend() factory (existing)
│   ├── sqlite.rs       # SqliteBackend (existing — reference implementation)
│   └── qdrant.rs       # QdrantBackend (new — entire implementation here)
```

### Pattern 1: QdrantBackend Struct and Construction

**What:** Store the `Qdrant` client directly in the struct (no Arc needed — it's already cheaply cloneable and `Send + Sync` via internal Arc). Implement `new()` as async to allow collection creation at startup.

**When to use:** All Qdrant backends follow this pattern.

**Example:**
```rust
// Source: qdrant-client 1.17.0 docs + CONTEXT.md D-26, D-27
#[cfg(feature = "backend-qdrant")]
use qdrant_client::{Qdrant, QdrantError};
use qdrant_client::qdrant::{
    CreateCollectionBuilder, VectorParamsBuilder, Distance,
    CreateFieldIndexCollectionBuilder, FieldType,
};

pub struct QdrantBackend {
    client: Qdrant,
    collection: String,  // "mnemonic_memories"
}

impl QdrantBackend {
    pub async fn new(config: &Config) -> Result<Self, ApiError> {
        let url = config.qdrant_url.as_deref()
            .ok_or_else(|| ApiError::Internal(/* ConfigError::Load(...) */))?;

        let mut builder = Qdrant::from_url(url);
        if let Some(key) = &config.qdrant_api_key {
            builder = builder.api_key(key);
        }
        let client = builder.build()
            .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

        let backend = Self {
            client,
            collection: "mnemonic_memories".to_string(),
        };
        backend.ensure_collection().await?;
        Ok(backend)
    }
}
```

### Pattern 2: Collection Auto-Creation (Idempotent)

**What:** Check collection existence with `collection_exists()` before creating. Create with 384-dim cosine vectors and payload indexes.

**Example:**
```rust
// Source: qdrant-client 1.17.0 API — collection_exists, create_collection, create_field_index
async fn ensure_collection(&self) -> Result<(), ApiError> {
    let exists = self.client.collection_exists(&self.collection).await
        .map_err(|e| /* map to ApiError::Internal */)?;

    if !exists {
        self.client.create_collection(
            CreateCollectionBuilder::new(&self.collection)
                .vectors_config(VectorParamsBuilder::new(384, Distance::Cosine))
        ).await.map_err(|e| /* ApiError::Internal */)?;

        // Create payload indexes for efficient filtering (D-04)
        for (field, ftype) in [
            ("agent_id", FieldType::Keyword),
            ("session_id", FieldType::Keyword),
            ("tags", FieldType::Keyword),
        ] {
            self.client.create_field_index(
                CreateFieldIndexCollectionBuilder::new(&self.collection, field, ftype)
            ).await.map_err(|e| /* ApiError::Internal */)?;
        }
    }
    Ok(())
}
```

### Pattern 3: Point Upsert with Payload

**What:** Construct a `PointStruct` with UUID string as ID, payload from `serde_json::json!`, and upsert.

**Example:**
```rust
// Source: qdrant-client examples/query.rs + payload.rs TryFrom<serde_json::Value>
use qdrant_client::qdrant::{PointStruct, UpsertPointsBuilder};
use qdrant_client::Payload;

let payload: Payload = serde_json::json!({
    "id": req.id,
    "content": req.content,
    "agent_id": req.agent_id,
    "session_id": req.session_id,
    "tags": req.tags,
    "embedding_model": req.embedding_model,
    "created_at": created_at,
    "updated_at": serde_json::Value::Null,
}).try_into().expect("valid JSON always converts to Payload");

// UUID string converts to PointId via Into<PointId> — UUID strings are natively supported
let point = PointStruct::new(req.id.clone(), req.embedding.clone(), payload);

self.client.upsert_points(
    UpsertPointsBuilder::new(&self.collection, vec![point])
).await.map_err(|e| /* ApiError::Internal */)?;
```

**Critical note:** `PointStruct::new()` accepts `impl Into<PointId>`. UUID strings (which our IDs are) are valid Qdrant point IDs. The Into<PointId> conversion parses the UUID string into the internal UUID representation.

### Pattern 4: Score-to-Distance Conversion (QDRT-02)

**What:** Qdrant cosine similarity returns `f32` scores where higher is more similar. The trait contract requires lower-is-better distance.

**Example:**
```rust
// Source: CONTEXT.md D-08, D-09
fn score_to_distance(score: f32) -> f64 {
    (1.0 - score) as f64
}

// In search(), after getting ScoredPoint results:
let memories: Vec<SearchResultItem> = results
    .into_iter()
    .map(|pt| {
        let distance = score_to_distance(pt.score);
        // ... extract Memory from pt.payload ...
        SearchResultItem { memory, distance }
    })
    .filter(|item| params.threshold.map_or(true, |t| item.distance <= t as f64))
    .collect();
```

**Qdrant cosine range:** Qdrant's cosine similarity score is in [-1.0, 1.0] range internally, but typically [0.0, 1.0] after normalization for unit vectors. After `1.0 - score`, distance is in [0.0, 2.0] where 0.0 = identical, 2.0 = opposite.

### Pattern 5: Payload Filtering (QDRT-04)

**What:** Build `Filter::must()` with multiple `Condition::matches()` conditions for namespace isolation.

**Example:**
```rust
// Source: qdrant-client filters.rs API
use qdrant_client::qdrant::{Filter, Condition};

fn build_filter(
    agent_id: Option<&str>,
    session_id: Option<&str>,
    tag: Option<&str>,
    after: Option<&str>,   // ISO 8601 string
    before: Option<&str>,  // ISO 8601 string
) -> Option<Filter> {
    let mut conditions: Vec<Condition> = Vec::new();

    if let Some(id) = agent_id {
        conditions.push(Condition::matches("agent_id", id.to_string()));
    }
    if let Some(sid) = session_id {
        conditions.push(Condition::matches("session_id", sid.to_string()));
    }
    if let Some(t) = tag {
        // Tags is a String array — matching a single value checks if array contains it
        conditions.push(Condition::matches("tags", t.to_string()));
    }
    if after.is_some() || before.is_some() {
        use prost_types::Timestamp;
        let range = qdrant_client::qdrant::DatetimeRange {
            gte: after.and_then(|s| s.parse::<Timestamp>().ok()),
            lt:  before.and_then(|s| s.parse::<Timestamp>().ok()),
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
```

**Important:** Tags stored as String array in payload. `Condition::matches("tags", "single_value".to_string())` checks if the array contains that value — this is Qdrant's native array containment check for keyword fields.

### Pattern 6: Scroll for list() and fetch_candidates()

**What:** Use `scroll()` with filter, limit, and `with_vectors` for candidate fetching. Sort client-side for `created_at DESC`.

**Example:**
```rust
// Source: qdrant-client API — ScrollPointsBuilder
use qdrant_client::qdrant::ScrollPointsBuilder;

// For list() — no vectors needed, has payload filter
let scroll_result = self.client.scroll(
    ScrollPointsBuilder::new(&self.collection)
        .filter(filter)          // Option<Filter> from build_filter()
        .limit(fetch_limit)      // u32
        .with_payload(true)
        .with_vectors(false)
).await.map_err(|e| /* ApiError::Internal */)?;

let mut results = scroll_result.result; // Vec<RetrievedPoint>

// Client-side sort by created_at DESC (D-15, D-21)
results.sort_by(|a, b| {
    let ta = get_payload_str(&a.payload, "created_at").unwrap_or_default();
    let tb = get_payload_str(&b.payload, "created_at").unwrap_or_default();
    tb.cmp(&ta)  // DESC — ISO 8601 strings sort lexicographically
});
```

**Scroll pagination:** For `list()` with integer offset/limit, scroll all results up to `offset + limit`, then take the slice. For typical agent workloads (<10K memories), this is acceptable.

**Scroll for fetch_candidates:**
```rust
let scroll_result = self.client.scroll(
    ScrollPointsBuilder::new(&self.collection)
        .filter(Filter::must([Condition::matches("agent_id", agent_id.to_string())]))
        .limit(max_candidates as u32 + 1)  // over-fetch by 1 to detect truncation
        .with_payload(true)
        .with_vectors(true)  // D-19: need embeddings for compaction
).await.map_err(|e| /* ApiError::Internal */)?;
```

### Pattern 7: Query/Search

**What:** Use `query()` API (modern unified API, preferred over `search_points()`) with vector and filter.

**Example:**
```rust
// Source: qdrant-client examples/query.rs — QueryPointsBuilder
use qdrant_client::qdrant::QueryPointsBuilder;

let response = self.client.query(
    QueryPointsBuilder::new(&self.collection)
        .query(embedding)    // Vec<f32> — directly accepted
        .limit(limit as u64)
        .with_payload(true)
        .filter(filter)      // None or Some(Filter) — builder accepts Option<Filter>
).await.map_err(|e| /* ApiError::Internal */)?;

// response.result is Vec<ScoredPoint>
// pt.score is f32, higher is more similar (cosine)
```

### Pattern 8: Delete Points

**What:** Delete by point IDs using `delete_points()` with `PointsIdsList`.

**Example:**
```rust
// Source: qdrant-client points.rs
use qdrant_client::qdrant::{DeletePointsBuilder, PointsIdsList};

// Delete single point (for delete() method)
self.client.delete_points(
    DeletePointsBuilder::new(&self.collection)
        .points(PointsIdsList {
            ids: vec![id.to_string().into()],
        })
).await.map_err(|e| /* ApiError::Internal */)?;

// Delete multiple source points (for write_compaction_result)
self.client.delete_points(
    DeletePointsBuilder::new(&self.collection)
        .points(PointsIdsList {
            ids: req.source_ids.iter().map(|id| id.to_string().into()).collect(),
        })
).await.map_err(|e| /* ApiError::Internal */)?;
```

### Pattern 9: Get Point by ID

**What:** Use `get_points()` to retrieve a single point by its UUID string ID.

**Example:**
```rust
// Source: qdrant-client API — GetPointsBuilder
use qdrant_client::qdrant::GetPointsBuilder;

let response = self.client.get_points(
    GetPointsBuilder::new(&self.collection, vec![id.to_string().into()])
        .with_payload(true)
        .with_vectors(false)
).await.map_err(|e| /* ApiError::Internal */)?;

match response.result.into_iter().next() {
    Some(point) => Ok(Some(point_to_memory(&point.payload)?)),
    None => Ok(None),
}
```

### Pattern 10: Payload Extraction Helper

**What:** Extract typed values from Qdrant's `HashMap<String, qdrant_client::qdrant::Value>` payload.

**Example:**
```rust
// Source: qdrant-client qdrant::Value type — dynamically typed union
use qdrant_client::qdrant::Value;

fn get_str(payload: &HashMap<String, Value>, key: &str) -> Option<String> {
    payload.get(key)?.as_str().map(|s| s.to_string())
    // or use .kind field: Value.kind == Some(Kind::StringValue(s))
}

fn get_str_array(payload: &HashMap<String, Value>, key: &str) -> Vec<String> {
    payload.get(key)
        .and_then(|v| v.as_list())
        .map(|list| list.values.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default()
}
```

### Anti-Patterns to Avoid

- **Using `search_points()` instead of `query()`:** `search_points()` is the older API. Use `query()` (QueryPointsBuilder) for new code.
- **Storing embeddings only in payload:** Embeddings must be stored as the point's vector, not in payload — the vector field enables semantic search.
- **Assuming Qdrant transactions:** There are none. Document the non-transactional nature explicitly in `write_compaction_result`.
- **Forgetting `with_vectors(true)` in fetch_candidates:** Without this flag, the scroll response omits vector data — compaction will fail.
- **Integer offset for scroll pagination:** Qdrant scroll uses PointId-based offsets, not integer offsets. For integer offset/limit, scroll and skip.
- **Not indexing `agent_id` before use:** Payload filters without indexes work but are slow for large collections. Create indexes at collection creation time (D-04).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| gRPC connection management | Custom gRPC client | `Qdrant::from_url().build()` | Connection pooling, TLS, compression handled internally |
| Payload type conversion | Custom serde::Serialize impl | `serde_json::json!(...).try_into::<Payload>()` | Handles all Value types correctly |
| Filter construction | Manual protobuf construction | `Filter::must()`, `Condition::matches()` | Type-safe filter builder, handles all condition types |
| Timestamp parsing | Custom date parser | `prost_types::Timestamp` with `.parse::<Timestamp>()` | RFC3339/ISO8601 parsing via `FromStr` implementation |
| Score normalization logic | Complex formula | `(1.0 - score) as f64` | Single-line, verified by CONTEXT.md D-08 |
| Point ID conversion | UUID library + manual struct | `uuid_string.to_string().into()` for `impl Into<PointId>` | UUID strings are directly accepted by PointId's Into impl |

**Key insight:** The qdrant-client builder-pattern API handles all serialization, transport, and type conversion. Implementing any of these manually introduces subtle bugs with protobuf encoding.

---

## Common Pitfalls

### Pitfall 1: Feature Flag Dependency Syntax
**What goes wrong:** `backend-qdrant = []` does not enable the qdrant-client optional dependency. Tests compiled with `--features backend-qdrant` will fail with "unresolved import" at `use qdrant_client::...`.
**Why it happens:** In Cargo, optional dependencies must be explicitly listed under the feature or declared with `dep:` syntax.
**How to avoid:** Change feature declaration to `backend-qdrant = ["dep:qdrant-client"]` AND add `qdrant-client = { version = "1", optional = true }` in `[dependencies]`.
**Warning signs:** `error[E0432]: unresolved import 'qdrant_client'` when building with `--features backend-qdrant`.

### Pitfall 2: `with_vectors` Omitted in fetch_candidates
**What goes wrong:** Scroll without `with_vectors(true)` returns `None` for the vectors field on `RetrievedPoint`. Attempting to access embeddings panics or produces empty vectors.
**Why it happens:** Qdrant omits vectors from responses by default to save bandwidth.
**How to avoid:** Always set `.with_vectors(true)` in the `fetch_candidates` scroll call (D-19).
**Warning signs:** `RetrievedPoint.vectors` is `None`; compaction produces zero candidates.

### Pitfall 3: Non-Transactional Compaction Silent Data Loss
**What goes wrong:** If the process crashes between upsert and delete, source memories remain alongside the merged memory. If the process crashes before upsert, the operation is lost entirely.
**Why it happens:** Qdrant has no cross-operation transactions. Each API call is its own atomic unit.
**How to avoid:** Use upsert-first, delete-second order (D-10). Document the semantic explicitly in `write_compaction_result` doc comment. Log a warning on partial failure (D-12).
**Warning signs:** Compaction runs repeatedly but memory count doesn't decrease.

### Pitfall 4: Score Range Misunderstanding
**What goes wrong:** Developer assumes Qdrant cosine returns scores in [0, 1] and converts via `1.0 - score`. For normalized vectors this is usually fine, but raw unnormalized embeddings can produce scores outside [0, 1].
**Why it happens:** Cosine similarity is mathematically in [-1.0, 1.0]. Qdrant normalizes vectors before storage but the returned score may still be slightly outside [0, 1] due to float precision.
**How to avoid:** The `1.0 - score` conversion is still correct — it produces distances in [0.0, 2.0]. Don't clamp to [0, 1].
**Warning signs:** Negative distances in search results (scores slightly above 1.0 due to float precision).

### Pitfall 5: Client-Side Sort Memory for Large Agents
**What goes wrong:** `list()` fetches all memories for an agent up to `offset + limit`, sorts them, then returns a page. For agents with 50K+ memories, this OOMs or is very slow.
**Why it happens:** Qdrant scroll API does not natively sort by payload field (only by ID or score).
**How to avoid:** Document this limitation clearly in the `list()` method comment. Add a practical limit note as per Claude's Discretion item. This is acceptable for v1.4.
**Warning signs:** High memory usage or slow responses for agents with large memory counts.

### Pitfall 6: Payload Extraction from `qdrant_client::qdrant::Value`
**What goes wrong:** `Value` is a protobuf-generated type with a `kind` field (enum), not a `serde_json::Value`. Calling `.as_str()` directly on `Value` may not be available.
**Why it happens:** The qdrant-client `Value` type wraps protobuf's well-known `Value` type, not serde_json.
**How to avoid:** Access payload values via `Value.kind` matching or the helper methods provided on the type. Alternatively, use the `serde_json` feature of qdrant-client if available to get JSON interop.
**Warning signs:** Compile error "method `as_str` not found for `qdrant_client::qdrant::Value`".

### Pitfall 7: `collection_exists()` Type Mismatch
**What goes wrong:** `collection_exists()` accepts `impl Into<CollectionExistsRequest>`, not a bare `&str`. Passing `&self.collection` directly may require `.as_str()` or direct string reference.
**Why it happens:** The Into<CollectionExistsRequest> impl accepts `&str` and `String` but the exact bound requires checking.
**How to avoid:** Use `self.client.collection_exists(&self.collection)` — `&String` auto-derefs to `&str` which implements `Into<CollectionExistsRequest>`.

---

## Code Examples

### Complete store() method skeleton
```rust
// Source: CONTEXT.md patterns + qdrant-client 1.17.0 API
async fn store(&self, req: StoreRequest) -> Result<Memory, ApiError> {
    let created_at = chrono::Utc::now().to_rfc3339();  // or uuid_v7 timestamp extraction
    // NOTE: SqliteBackend uses SQLite's datetime('now') — QdrantBackend must generate this server-side

    let payload: Payload = serde_json::json!({
        "id": req.id,
        "content": req.content,
        "agent_id": req.agent_id,
        "session_id": req.session_id,
        "tags": req.tags,
        "embedding_model": req.embedding_model,
        "created_at": created_at,
        "updated_at": serde_json::Value::Null,
    }).try_into().map_err(|e| ApiError::Internal(/* ... */))?;

    let point = PointStruct::new(req.id.clone(), req.embedding, payload);

    self.client.upsert_points(
        UpsertPointsBuilder::new(&self.collection, vec![point])
    ).await.map_err(|e| /* ApiError::Internal */)?;

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
```

### Unit-testable helpers (D-28)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_to_distance_identical() {
        // Perfect match: score=1.0 → distance=0.0
        assert_eq!(score_to_distance(1.0), 0.0);
    }

    #[test]
    fn test_score_to_distance_opposite() {
        // Opposite: score=-1.0 → distance=2.0
        assert_eq!(score_to_distance(-1.0), 2.0);
    }

    #[test]
    fn test_score_to_distance_midpoint() {
        // Orthogonal: score=0.0 → distance=1.0
        assert!((score_to_distance(0.0) - 1.0).abs() < 1e-10);
    }

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
}
```

### Cargo.toml feature declaration (D-24)
```toml
[features]
backend-qdrant = ["dep:qdrant-client"]
backend-postgres = []

[dependencies]
qdrant-client = { version = "1", optional = true }
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `search_points()` API | `query()` API (QueryPointsBuilder) | qdrant-client ~1.10 | Use `query()` — unified, supports hybrid search and re-ranking |
| Numeric point IDs only | UUID string point IDs | Qdrant server 1.x | Our UUID v7 IDs map directly to Qdrant's UUID point ID type |
| Manual payload construction | `serde_json::json!(...).try_into::<Payload>()` | qdrant-client 1.x | Clean JSON-to-Payload conversion |
| Static quantization config required | Quantization optional | qdrant-client 1.x | `CreateCollectionBuilder` without quantization works fine |

**Deprecated/outdated:**
- `search_points()`: Still works but superseded by `query()` for new code
- `client.upsert_points_blocking()`: Sync API removed; use async `upsert_points()` with `.await`

---

## Open Questions

1. **`created_at` timestamp generation in `store()`**
   - What we know: SqliteBackend uses `datetime('now')` in SQL and reads it back from the DB. QdrantBackend has no server-side timestamp injection.
   - What's unclear: Should QdrantBackend generate the ISO 8601 timestamp client-side (using `chrono::Utc::now()`) or derive it from the UUID v7 ID's embedded timestamp?
   - Recommendation: Generate client-side with `chrono::Utc::now().to_rfc3339()` — simple, consistent, already a dependency via uuid crate's chrono feature or use `std::time::SystemTime`. Alternatively, extract from UUID v7 timestamp bits. Either is fine; pick the simpler option.

2. **`Value.as_str()` availability on `qdrant_client::qdrant::Value`**
   - What we know: `Value` is a protobuf type with `kind: Option<Kind>` where `Kind` is an enum. Helper methods may or may not be available.
   - What's unclear: Exact method names for extracting string, list, null values from `qdrant_client::qdrant::Value`.
   - Recommendation: During implementation, check `docs.rs/qdrant-client/1.17.0/qdrant_client/qdrant/struct.Value.html` for helper methods. If not available, pattern-match on `value.kind` directly.

3. **`QueryPointsBuilder` accepting `Option<Filter>` vs requiring a filter**
   - What we know: The builder pattern uses `.filter(filter)` — unclear if it accepts `Option<Filter>` or requires `Filter`.
   - What's unclear: How to make the filter optional (no filter = return all results for agent).
   - Recommendation: Wrap in conditional — only call `.filter(f)` when `filter.is_some()`. Use `if let Some(f) = filter { builder = builder.filter(f) }`.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + tokio::test, cargo test |
| Config file | none (uses Cargo.toml [dev-dependencies]) |
| Quick run command | `cargo test --lib 2>&1` (unit tests only, no live Qdrant) |
| Full suite command | `cargo test --features backend-qdrant 2>&1` (requires live Qdrant on localhost:6334) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| QDRT-01 | QdrantBackend struct compiles behind feature flag | compile-time | `cargo build --features backend-qdrant` | ❌ Wave 0 — new file `src/storage/qdrant.rs` |
| QDRT-01 | `create_backend("qdrant")` returns QdrantBackend when feature enabled | integration | `cargo test --features backend-qdrant test_create_backend_qdrant` | ❌ Wave 0 (in `src/storage/mod.rs` tests) |
| QDRT-01 | Default binary (no feature) has zero qdrant-client deps | compile-time | `cargo build 2>&1 \| grep -v qdrant` | ✅ existing (cargo build passes without feature) |
| QDRT-02 | `score_to_distance(1.0) == 0.0` | unit | `cargo test --features backend-qdrant test_score_to_distance` | ❌ Wave 0 — `src/storage/qdrant.rs` |
| QDRT-02 | Search results ordered lowest-distance first | integration | `cargo test --features backend-qdrant test_search_distance_order` | ❌ Wave 0 — `src/storage/qdrant.rs` |
| QDRT-03 | `write_compaction_result` upserts merged, deletes sources | integration | `cargo test --features backend-qdrant test_compaction_upsert_delete_order` | ❌ Wave 0 — `src/storage/qdrant.rs` |
| QDRT-03 | Existing 273 tests pass without backend-qdrant feature | regression | `cargo test` | ✅ existing |
| QDRT-04 | Memories under agent-A not visible to agent-B | integration | `cargo test --features backend-qdrant test_agent_isolation` | ❌ Wave 0 — `src/storage/qdrant.rs` |

### Sampling Rate
- **Per task commit:** `cargo test --lib` (unit tests, no live Qdrant required, ~30 seconds)
- **Per wave merge:** `cargo test` (existing 273 tests must remain green)
- **Phase gate:** `cargo build --features backend-qdrant && cargo test --features backend-qdrant` with live Qdrant running before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `src/storage/qdrant.rs` — entire QdrantBackend implementation (new file)
- [ ] Unit tests in `src/storage/qdrant.rs`: `test_score_to_distance_*`, `test_build_filter_*`, `test_payload_construction`
- [ ] Integration tests in `src/storage/qdrant.rs` (behind `#[cfg(all(test, feature = "backend-qdrant"))]`): `test_store_and_get`, `test_search_distance_order`, `test_agent_isolation`, `test_compaction_upsert_delete_order`
- [ ] Updated `src/storage/mod.rs`: conditional `pub mod qdrant;` declaration + re-export + `create_backend()` wiring
- [ ] Updated `Cargo.toml`: `backend-qdrant = ["dep:qdrant-client"]` and `qdrant-client = { version = "1", optional = true }`

---

## Sources

### Primary (HIGH confidence)
- `docs.rs/qdrant-client/1.17.0` — Qdrant struct methods (create_collection, upsert_points, query, scroll, get_points, delete_points, create_field_index, collection_exists, health_check), builder types, ScoredPoint fields
- `docs.rs/qdrant-client/1.9.0/src/qdrant_client/filters.rs` — Filter::must/should/must_not, Condition::matches/datetime_range/range, MatchValue conversions
- `crates.io API` — qdrant-client 1.17.0 confirmed as latest stable (published 2026-02-20)
- `crates.io dependencies API` — prost-types ^0.13.3 and tonic ^0.12.3 confirmed as transitive deps
- `docs.rs/prost-types` — Timestamp::from_str (RFC3339 parsing), Timestamp::date/date_time constructors

### Secondary (MEDIUM confidence)
- `github.com/qdrant/rust-client examples/query.rs` — CreateCollectionBuilder, VectorParamsBuilder, QueryPointsBuilder usage patterns
- `github.com/qdrant/rust-client src/qdrant_client/payload.rs` — serde_json::json!(...).try_into::<Payload>() pattern
- `github.com/qdrant/rust-client src/qdrant_client/points.rs` — upsert_points, delete_points with PointsIdsList patterns

### Tertiary (LOW confidence — verify during implementation)
- qdrant-client `Value.as_str()` helper method availability — documented indirectly; exact method names should be verified at `docs.rs` during implementation
- `QueryPointsBuilder` optional filter chaining — behavior when no filter is passed needs runtime verification

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — qdrant-client 1.17.0 verified on crates.io with exact publish date
- Architecture: HIGH — all 7 StorageBackend methods have confirmed qdrant-client API mappings
- Pitfalls: HIGH for pitfalls 1-4 (verified against docs/CONTEXT.md), MEDIUM for 5-7 (known patterns, implementation details may vary)

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable library — 30 days)
