# Phase 3: Service and API - Research

**Researched:** 2026-03-19
**Domain:** Rust axum HTTP API, sqlite-vec KNN + metadata filtering, MemoryService orchestration
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Search query design**
- `GET /memories/search` with query parameters: `q` (required), `agent_id`, `session_id`, `tag`, `limit`, `after`, `before`
- Default top-K: 10 results, max 100 via `limit` parameter
- No distance threshold by default; optional `threshold` param
- Agent pre-filter: JOIN between `vec_memories` KNN results and `memories` table filtered by `agent_id`
- Over-fetch from KNN when `agent_id` filter is present; filter down after JOIN

**Response format**
- Omit embedding vectors from all responses
- `POST /memories` returns 201 with full memory object (id, created_at, embedding_model)
- `GET /memories` returns `{ "memories": [...], "total": N }` with `offset`/`limit` pagination (default offset=0, limit=20, max 100)
- `GET /memories/search` returns `{ "memories": [...] }` where each memory includes a `distance` float field
- `DELETE /memories/:id` returns 200 with the deleted memory object
- Memory object shape: `{ id, content, agent_id, session_id, tags, embedding_model, created_at, updated_at }`
- Tags returned as a JSON array

**Error responses**
- JSON error body: `{ "error": "Human-readable message" }` — no error codes in v1
- Status codes: 201 (create), 200 (read/search/delete), 400 (validation), 404 (not found), 500 (internal)
- Embedding failures during POST: 400 if empty content, 500 if API/model failure
- axum `IntoResponse` implementation on a unified API error type

**Service layer architecture**
- New `src/service.rs` with `MemoryService` struct
- MemoryService holds `Arc<Connection>` and `Arc<dyn EmbeddingEngine>` (not full AppState)
- Orchestration flow for POST: validate → embed → generate UUID v7 → insert both tables in one `conn.call()` → return created memory
- Thin axum handlers in `server.rs`: extract params, call MemoryService, format response
- MemoryService methods return `Result<T, MnemonicError>`
- `server.rs` gains new routes: POST /memories, GET /memories, GET /memories/search, DELETE /memories/:id
- UUID v7 via `uuid` crate with `v7` feature (already in Cargo.toml as `uuid = { version = "1", features = ["v7"] }`)

### Claude's Discretion
- Exact sqlite-vec KNN query syntax and JOIN pattern (researched below — see Critical Finding)
- Over-fetch multiplier for agent_id-filtered KNN queries
- Request/response serde struct naming and field ordering
- Whether to use axum extractors (Query, Json, Path) or manual extraction
- Test structure for API integration tests (in-memory DB vs temp file)
- Whether MemoryService methods are async or use `conn.call` internally

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| API-01 | `POST /memories` stores memory with content, optional agent_id, session_id, tags | JSON body extraction, MemoryService.create_memory(), dual-table insert in single conn.call() |
| API-02 | `GET /memories/search` semantic search with optional filters | KNN JOIN pattern, over-fetch multiplier, zerocopy f32 binding |
| API-03 | `GET /memories` list with filtering by agent_id, session_id, tags, time range | SQL WHERE clause builder, offset/limit pagination |
| API-04 | `DELETE /memories/:id` delete a specific memory | Path extractor, DELETE from both tables in transaction, 404 if missing |
| API-05 | `GET /health` returns server readiness | Already implemented in server.rs:34-36 — no new work needed |
| API-06 | All endpoints return JSON with appropriate HTTP status codes and error messages | IntoResponse on ApiError, (StatusCode, Json) tuples |
| AGNT-01 | Memories namespaced by agent_id | agent_id column in memories table (already exists), filter in all queries |
| AGNT-02 | Memories grouped by session_id for conversation-scoped retrieval | session_id column (already exists), filter support in list/search |
| AGNT-03 | Semantic search pre-filters by agent_id before KNN | CTE + JOIN pattern with over-fetch; see Critical Finding below |
</phase_requirements>

---

## Summary

Phase 3 builds the HTTP API layer on top of the Phase 1-2 foundation. The codebase already has axum routing scaffolding (`server.rs`), the full DB schema with `memories` and `vec_memories` tables, and the `EmbeddingEngine` trait. The work is primarily: (1) a new `src/service.rs` MemoryService orchestrator, (2) five route handlers in `server.rs`, (3) an API error type with `IntoResponse`, and (4) integration tests using axum's `tower::ServiceExt::oneshot` pattern.

The most critical technical challenge — sqlite-vec KNN with agent_id pre-filter — has a confirmed solution: a CTE-based JOIN pattern where you over-fetch from KNN (e.g., 10x the requested limit), join to `memories` on agent_id, then slice to `limit`. This avoids the sqlite-vec virtual table restriction that prevents WHERE clauses inside the KNN step. The metadata column approach from sqlite-vec v0.1.6+ is not applicable here because our schema keeps vectors in a separate `vec_memories` table.

**Primary recommendation:** Use the CTE + JOIN over-fetch pattern for KNN search, implement a unified `ApiError` enum with `IntoResponse`, use axum extractors `Query<T>`, `Json<T>`, `Path<T>` with serde-derived structs, and test all routes via `tower::ServiceExt::oneshot` without a running server.

---

## Standard Stack

### Core (all already in Cargo.toml)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.8.8 | HTTP framework, routing, extractors | Already in use, idiomatic Rust HTTP |
| serde | 1 | Request/response struct serialization | Already in use; derive Serialize/Deserialize |
| serde_json | 1 | JSON body parsing and JSON error responses | Already in use |
| tokio-rusqlite | 0.7.0 | Async SQLite via conn.call() closures | Already in use; established pattern |
| uuid | 1.22.0 | UUID v7 generation (`Uuid::now_v7()`) | Already in Cargo.toml with `v7` feature |
| zerocopy | 0.8.47 | Convert `Vec<f32>` to `&[u8]` for sqlite binding | Already transitive dep; f32 impl IntoBytes |

### Supporting (add to Cargo.toml)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| http-body-util | 0.1.3 | Read response body in tests | Already transitive dep; needed for test assertions |
| tower | (axum dep) | `ServiceExt::oneshot` for handler tests | Already transitive dep through axum |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| CTE + JOIN over-fetch | sqlite-vec metadata columns | Metadata columns require vec0 schema change; current schema separates tables — JOIN is correct for this architecture |
| zerocopy IntoBytes | bytemuck::cast_slice | bytemuck is also available; both work. zerocopy is already used by sqlite-vec itself — prefer zerocopy |
| tower::ServiceExt::oneshot tests | Running full server with reqwest | oneshot is faster, no port binding, same logic coverage |

**Installation:** No new top-level dependencies needed. `zerocopy` and `http-body-util` are already available as transitive deps but should be added to `[dev-dependencies]` for explicit test use.

**Version verification:** All versions confirmed via `cargo metadata` on 2026-03-19.

---

## Architecture Patterns

### Recommended Project Structure (additions)

```
src/
├── service.rs      # NEW: MemoryService struct, orchestration logic
├── server.rs       # EXTEND: add 5 new route handlers and API error type
├── error.rs        # EXTEND: add ApiError enum with IntoResponse
├── lib.rs          # EXTEND: add pub mod service
└── [existing files unchanged: db.rs, config.rs, embedding.rs, main.rs]

tests/
└── integration.rs  # EXTEND: add API integration tests (oneshot pattern)
```

### Pattern 1: MemoryService Struct

**What:** A service struct that holds `Arc<Connection>` and `Arc<dyn EmbeddingEngine>` and exposes async methods for each API operation.

**When to use:** Separates business logic from HTTP handling. Handlers stay thin — extract → call service → convert to response.

```rust
// src/service.rs
use std::sync::Arc;
use tokio_rusqlite::Connection;
use crate::embedding::EmbeddingEngine;
use crate::error::MnemonicError;

pub struct MemoryService {
    pub db: Arc<Connection>,
    pub embedding: Arc<dyn EmbeddingEngine>,
}

impl MemoryService {
    pub fn new(db: Arc<Connection>, embedding: Arc<dyn EmbeddingEngine>) -> Self {
        Self { db, embedding }
    }

    // Methods: create_memory, search_memories, list_memories, delete_memory
    // All return Result<T, MnemonicError>
    // All use conn.call(|c| { ... }) for DB access
}
```

### Pattern 2: Dual-Table Atomic Insert (POST /memories)

**What:** Insert into both `memories` and `vec_memories` in a single `conn.call()` closure using a rusqlite transaction.

**When to use:** Required for consistency — if the vec insert fails, the metadata row should not be committed.

```rust
// Source: tokio-rusqlite docs + existing project pattern
let memory_id = uuid::Uuid::now_v7().to_string();
let embedding = self.embedding.embed(&content).await
    .map_err(MnemonicError::Embedding)?;

// Convert f32 vec to bytes for sqlite-vec binding
// zerocopy 0.8 — f32 implements IntoBytes (confirmed in impls.rs line 79)
use zerocopy::IntoBytes;
let embedding_bytes: Vec<u8> = embedding.as_bytes().to_vec();
// OR: bytemuck::cast_slice::<f32, u8>(&embedding).to_vec()

self.db.call(move |c| -> Result<(), rusqlite::Error> {
    let tx = c.transaction()?;
    tx.execute(
        "INSERT INTO memories (id, content, agent_id, session_id, tags, embedding_model, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
        rusqlite::params![
            memory_id, content, agent_id, session_id, tags_json, embedding_model
        ],
    )?;
    tx.execute(
        "INSERT INTO vec_memories (memory_id, embedding) VALUES (?1, ?2)",
        rusqlite::params![memory_id, embedding_bytes],
    )?;
    tx.commit()?;
    Ok(())
}).await?;
```

### Pattern 3: KNN Search with agent_id Pre-Filter (CRITICAL)

**What:** sqlite-vec KNN queries do not support WHERE clauses that join to external tables. The solution is a CTE over-fetch pattern.

**When to use:** Any KNN search with optional agent_id or session_id filters (AGNT-03).

**Critical Finding:** The `vec_memories` virtual table only has `memory_id TEXT PRIMARY KEY` and `embedding float[384]`. It does not have agent_id as a metadata column. Therefore, metadata-column filtering (sqlite-vec v0.1.6+) does NOT apply here. We must use the CTE JOIN pattern.

```sql
-- KNN over-fetch then filter (Claude's Discretion: use 10x multiplier)
-- Example: user wants limit=10, agent_id='agent-foo'
-- Step 1: fetch 100 candidates from vec_memories KNN
-- Step 2: JOIN to memories WHERE agent_id = ?
-- Step 3: take first 10 results

WITH knn_candidates AS (
    SELECT memory_id, distance
    FROM vec_memories
    WHERE embedding MATCH ?1  -- bind: query_embedding.as_bytes()
    AND k = ?2                -- bind: over_fetch_limit (e.g. limit * 10)
)
SELECT m.id, m.content, m.agent_id, m.session_id, m.tags,
       m.embedding_model, m.created_at, m.updated_at,
       knn_candidates.distance
FROM knn_candidates
JOIN memories m ON m.id = knn_candidates.memory_id
WHERE (?3 IS NULL OR m.agent_id = ?3)
  AND (?4 IS NULL OR m.session_id = ?4)
ORDER BY knn_candidates.distance
LIMIT ?5  -- actual requested limit
```

**Over-fetch multiplier:** Use 10x. If limit=10 and agent_id is set, fetch k=100 from KNN, then filter. If no agent_id filter, fetch exactly k=limit from KNN.

**Binding query embedding:**
```rust
// zerocopy 0.8 — use IntoBytes trait
use zerocopy::IntoBytes;
let query_bytes: &[u8] = query_embedding.as_bytes();
// pass as rusqlite param — sqlite-vec recognizes BLOB as embedding vector
```

### Pattern 4: Axum Extractors — Standard Usage

```rust
// Source: https://docs.rs/axum/latest/axum/extract/index.html
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Json},
    http::StatusCode,
};

// GET /memories/search?q=...&agent_id=foo&limit=10
#[derive(serde::Deserialize)]
struct SearchParams {
    q: String,
    agent_id: Option<String>,
    session_id: Option<String>,
    tag: Option<String>,
    limit: Option<u32>,
    threshold: Option<f32>,
    after: Option<String>,
    before: Option<String>,
}

async fn search_handler(
    State(service): State<Arc<MemoryService>>,
    Query(params): Query<SearchParams>,
) -> Result<impl IntoResponse, ApiError> {
    // validate: params.q must be non-empty
    // call service.search_memories(params).await
    // return Json(SearchResponse { memories })
}

// POST /memories — Json must be LAST extractor (consumes body)
async fn create_memory_handler(
    State(service): State<Arc<MemoryService>>,
    Json(body): Json<CreateMemoryRequest>,
) -> Result<impl IntoResponse, ApiError> { ... }

// DELETE /memories/:id
async fn delete_memory_handler(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> { ... }
```

**Extractor ordering rule:** `FromRequestParts` extractors (State, Path, Query, headers) must come before `FromRequest` extractors (Json, Form — consume body). Json must be last.

### Pattern 5: Unified ApiError with IntoResponse

```rust
// src/error.rs addition (or src/server.rs — either is fine)
// Source: https://github.com/tokio-rs/axum/blob/main/examples/error-handling/src/main.rs

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),           // 400
    #[error("not found")]
    NotFound,                     // 404
    #[error("internal error")]
    Internal(#[from] MnemonicError),  // 500
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".to_string(),
            ),
        };
        (status, Json(serde_json::json!({"error": message}))).into_response()
    }
}
```

### Pattern 6: Integration Tests via oneshot (No Server Required)

```rust
// tests/integration.rs addition
// Source: https://github.com/tokio-rs/axum/blob/main/examples/testing/src/main.rs
use axum::{body::Body, http::{Request, StatusCode}};
use http_body_util::BodyExt;
use tower::ServiceExt; // for .oneshot()

#[tokio::test]
async fn test_post_memory() {
    setup();
    let config = test_config_file(); // file-based DB for persist test
    let conn = mnemonic::db::open(&config).await.unwrap();
    let engine = /* mock or LocalEngine */;
    let service = Arc::new(MemoryService::new(Arc::new(conn), engine));
    let app = mnemonic::server::build_router_with_service(service);

    let response = app
        .oneshot(
            Request::post("/memories")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"content":"test memory","agent_id":"a1"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["id"].is_string());
}
```

**Note on State:** `Router<S>` where S is non-() doesn't implement `Service`. Call `.with_state(state)` before using `oneshot`. Build the `Router` fully (with state wired) before passing to `oneshot`.

### Pattern 7: GET /memories List with SQL Filters

```rust
// Dynamic WHERE clause using conditional SQL fragments
// Avoid SQL injection — all values go through rusqlite params
let sql = "
    SELECT id, content, agent_id, session_id, tags, embedding_model, created_at, updated_at
    FROM memories
    WHERE (?1 IS NULL OR agent_id = ?1)
      AND (?2 IS NULL OR session_id = ?2)
      AND (?3 IS NULL OR tags LIKE '%' || ?3 || '%')  -- tag substring match
      AND (?4 IS NULL OR created_at > ?4)
      AND (?5 IS NULL OR created_at < ?5)
    ORDER BY created_at DESC
    LIMIT ?6 OFFSET ?7
";
// Separate COUNT query for total:
let count_sql = "SELECT COUNT(*) FROM memories WHERE ... (same filters, no LIMIT/OFFSET)";
```

### Anti-Patterns to Avoid

- **Embedding in response body:** All memory responses must omit the embedding vector — it's 384 floats (~1.5KB) and useless to callers.
- **Blocking in async handler:** Never call `LocalEngine::embed()` directly — it blocks. The service method is already async and wraps in `spawn_blocking`. Same applies to `conn.call()`.
- **Separate inserts without transaction:** Inserting into `memories` then `vec_memories` without a transaction risks inconsistent state if the second insert fails.
- **KNN without over-fetch when filtering:** If agent_id is set and you only request `k=limit`, you'll get fewer results than requested because many KNN candidates may belong to other agents.
- **Router with state in oneshot without `.with_state()`:** `Router<AppState>` does not implement `Service<Request>` — must call `.with_state(state)` first.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Vec<f32> to &[u8] for SQLite binding | Manual unsafe transmute | `zerocopy::IntoBytes::as_bytes()` | Safe, zero-copy, already in dep tree |
| HTTP error type with status codes | match arms in every handler | `ApiError` with `IntoResponse` | Single source of truth for error format |
| UUID generation | Custom ID scheme | `uuid::Uuid::now_v7().to_string()` | Time-ordered, globally unique, already in Cargo.toml |
| JSON deserialization of request bodies | Manual string parsing | `axum::extract::Json<T>` with serde derive | Content-type validation, automatic error rejection |
| API integration tests | Spinning up real TCP server | `tower::ServiceExt::oneshot` | Faster, no port conflicts, same routing logic |
| Agent_id SQL filter safety | String interpolation | `rusqlite::params![]` with `?N IS NULL OR col = ?N` | Prevents SQL injection, handles None cleanly |

**Key insight:** The `?N IS NULL OR col = ?N` pattern is the idiomatic rusqlite approach for optional SQL filters — bind `None::<String>` and the `IS NULL` check short-circuits, avoiding separate query branches.

---

## Common Pitfalls

### Pitfall 1: sqlite-vec KNN WHERE Clause Limitation

**What goes wrong:** Placing `AND agent_id = ?` inside the KNN query (matching against `memories` columns that don't exist in `vec_memories`) causes a compile-time or runtime SQL error.

**Why it happens:** `vec_memories` only has `memory_id` and `embedding`. The `agent_id` column lives in the `memories` table. sqlite-vec's KNN operator cannot join to external tables within its query.

**How to avoid:** Always use the CTE + JOIN pattern. The KNN step retrieves candidates; the JOIN step applies agent_id filtering.

**Warning signs:** `no such column: agent_id` error from sqlite at runtime.

### Pitfall 2: Router State Type Mismatch in Tests

**What goes wrong:** `Router<AppState>` doesn't implement `Service<Request>` — calling `.oneshot()` on it fails to compile.

**Why it happens:** `ServiceExt::oneshot` requires `Router<()>` (state erased).

**How to avoid:** Always call `.with_state(state)` before `.oneshot()`. The function signature for `build_router` should accept a fully-constructed `AppState` and return `Router<()>`.

**Warning signs:** Trait bound errors like `Router<AppState>: Service<...>` not satisfied.

### Pitfall 3: Json Extractor Not Last

**What goes wrong:** Handler compilation fails if `Json<T>` is not the last argument (it consumes the request body).

**Why it happens:** axum's extractor system requires that body-consuming extractors (`FromRequest<S>`) come last; all others must implement `FromRequestParts<S>`.

**How to avoid:** Order: `State`, `Path`, `Query` (all `FromRequestParts`) then `Json` last.

**Warning signs:** Compile error about extractor ordering or `FromRequest` vs `FromRequestParts`.

### Pitfall 4: zerocopy AsBytes vs IntoBytes Naming

**What goes wrong:** Importing `use zerocopy::AsBytes` compiles but triggers a deprecation warning in zerocopy 0.8.

**Why it happens:** zerocopy 0.8 renamed `AsBytes` to `IntoBytes`. The old name is preserved as a deprecated re-export.

**How to avoid:** Use `use zerocopy::IntoBytes;` and call `.as_bytes()` on the slice.

**Important:** `f32` DOES implement `IntoBytes` in zerocopy 0.8.47 (confirmed in `impls.rs:79`). No `float-nightly` feature needed for `f32` — only `f16` and `f128` need that.

**Warning signs:** Deprecation warnings from zerocopy; `as_bytes` not found if using wrong import.

### Pitfall 5: MemoryService vs AppState in Handlers

**What goes wrong:** If MemoryService is built from AppState inside each handler call, it creates a new struct on every request but the Arc fields are shared — this is fine for correctness but architecturally messy.

**Why it happens:** MemoryService should be constructed once and shared via Arc in AppState.

**How to avoid:** Add `service: Arc<MemoryService>` to `AppState`, construct it in `main.rs` alongside the DB and embedding setup, include it in the Router state.

### Pitfall 6: Tags Field Serde Round-Trip

**What goes wrong:** Tags stored as JSON string in SQLite (e.g., `'["foo","bar"]'`) but serde expects `Vec<String>` in the response struct.

**Why it happens:** The `memories` schema stores `tags TEXT DEFAULT '[]'`. Row queries return a `String`; the response struct needs `Vec<String>`.

**How to avoid:** In the response struct, tags field is `Vec<String>`. When reading from DB, deserialize with `serde_json::from_str::<Vec<String>>(&tags_str).unwrap_or_default()`. When writing, serialize with `serde_json::to_string(&tags)`.

---

## Code Examples

Verified patterns from official sources and project context:

### Vec<f32> to BLOB for sqlite-vec insertion

```rust
// Source: sqlite-vec demo.rs + zerocopy 0.8 impls.rs (f32: IntoBytes confirmed)
use zerocopy::IntoBytes;

let embedding: Vec<f32> = service.embedding.embed(&content).await?;
let embedding_bytes: &[u8] = embedding.as_bytes(); // zero-copy, safe

// Inside conn.call():
tx.execute(
    "INSERT INTO vec_memories (memory_id, embedding) VALUES (?1, ?2)",
    rusqlite::params![memory_id, embedding_bytes],
)?;
```

### KNN Query with Optional agent_id Filter

```sql
-- Recommended pattern (Claude's Discretion: over-fetch 10x)
WITH knn_candidates AS (
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
ORDER BY knn_candidates.distance
LIMIT ?5
```

### UUID v7 Generation

```rust
// Source: https://docs.rs/uuid/latest/uuid/struct.Uuid.html
// Feature already in Cargo.toml: uuid = { version = "1", features = ["v7"] }
let id = uuid::Uuid::now_v7().to_string();
// Produces time-ordered, globally unique string like "018f1234-5678-7abc-def0-123456789abc"
```

### ApiError IntoResponse

```rust
// Source: https://github.com/tokio-rs/axum/blob/main/examples/error-handling/src/main.rs
impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal server error".to_string()),
        };
        (status, Json(serde_json::json!({"error": message}))).into_response()
    }
}
```

### Axum Route Registration with MemoryService in State

```rust
// src/server.rs — updated build_router
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/memories", post(create_memory_handler))
        .route("/memories", get(list_memories_handler))
        .route("/memories/search", get(search_memories_handler))
        .route("/memories/:id", delete(delete_memory_handler))
        .with_state(state)
}
```

### Integration Test Pattern (oneshot)

```rust
// Source: https://github.com/tokio-rs/axum/blob/main/examples/testing/src/main.rs
use axum::{body::Body, http::{Request, StatusCode}};
use http_body_util::BodyExt; // for .collect()
use tower::ServiceExt;       // for .oneshot()

async fn build_test_app() -> axum::Router {
    let config = test_config(); // :memory: DB
    let conn = mnemonic::db::open(&config).await.unwrap();
    let engine = /* Arc<dyn EmbeddingEngine> */;
    let state = AppState {
        db: Arc::new(conn),
        config: Arc::new(config),
        embedding: engine,
        service: Arc::new(MemoryService::new(...)),
    };
    mnemonic::server::build_router(state) // returns Router<()> via .with_state()
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `zerocopy::AsBytes` import | `zerocopy::IntoBytes` import | zerocopy 0.8.0 | Old name still compiles but deprecated; prefer new name |
| sqlite-vss | sqlite-vec | 2024 | sqlite-vss archived; this project already uses sqlite-vec |
| axum 0.7 Router type inference | axum 0.8 explicit Router<S> state type | axum 0.8 | with_state() now required for test oneshot |

**Deprecated/outdated:**
- `zerocopy::AsBytes`: deprecated in 0.8, use `zerocopy::IntoBytes` and `.as_bytes()` method
- sqlite-vss: archived, irrelevant — project uses sqlite-vec

---

## Open Questions

1. **MemoryService location in AppState**
   - What we know: AppState currently has `db`, `config`, `embedding` fields
   - What's unclear: Should MemoryService replace those fields or be added alongside them?
   - Recommendation: Add `service: Arc<MemoryService>` to AppState. Handlers access state.service; internal service methods use self.db and self.embedding. Keep AppState fields for non-service uses (health check, config-only paths).

2. **Over-fetch multiplier for KNN**
   - What we know: Must over-fetch when agent_id filter is active
   - What's unclear: Optimal multiplier (10x chosen as Claude's Discretion)
   - Recommendation: Use 10x up to max k=1000. If `agent_id` is None, fetch exactly `k=limit`. Log a warning if results come back < limit (signals multiplier may need increasing for highly partitioned agents).

3. **test_config() for API tests: :memory: vs temp file**
   - What we know: WAL tests must use file DB; schema/embedding tests use :memory: fine
   - What's unclear: API tests that test "persists across server restarts" (Success Criterion 1) need a file DB
   - Recommendation: Use :memory: for most API unit/integration tests. For the "persists across restarts" criterion, use a temp file DB similar to the WAL test pattern in integration.rs:97-127.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) + tokio::test |
| Config file | none — inlined in Cargo.toml `[dev-dependencies]` |
| Quick run command | `cargo test --test integration` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| API-01 | POST /memories stores memory, returns 201 + id | Integration | `cargo test --test integration test_post_memory` | ❌ Wave 0 |
| API-01 | POST /memories with empty content returns 400 | Integration | `cargo test --test integration test_post_memory_validation` | ❌ Wave 0 |
| API-02 | GET /memories/search returns semantically ranked results | Integration | `cargo test --test integration test_search_memories` | ❌ Wave 0 |
| API-02 | Search without required `q` param returns 400 | Integration | `cargo test --test integration test_search_missing_q` | ❌ Wave 0 |
| API-03 | GET /memories returns filtered list with total | Integration | `cargo test --test integration test_list_memories` | ❌ Wave 0 |
| API-04 | DELETE /memories/:id removes memory, returns 200 + deleted object | Integration | `cargo test --test integration test_delete_memory` | ❌ Wave 0 |
| API-04 | DELETE /memories/:nonexistent returns 404 | Integration | `cargo test --test integration test_delete_not_found` | ❌ Wave 0 |
| API-05 | GET /health returns `{"status":"ok"}` 200 | Integration | `cargo test --test integration test_health` | ❌ Wave 0 |
| API-06 | All error paths return `{"error":"..."}` JSON | Integration | covered by above validation tests | ❌ Wave 0 |
| AGNT-01 | Two agents don't see each other's memories | Integration | `cargo test --test integration test_agent_isolation` | ❌ Wave 0 |
| AGNT-02 | Session filter scopes retrieval to session_id | Integration | `cargo test --test integration test_session_filter` | ❌ Wave 0 |
| AGNT-03 | Search with agent_id returns only that agent's memories | Integration | `cargo test --test integration test_search_agent_filter` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --test integration` (unit + integration, excludes embedding model download)
- **Per wave merge:** `cargo test` (full suite including embedding tests)
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

All API tests are new — add to existing `tests/integration.rs`:

- [ ] `test_post_memory` — covers API-01
- [ ] `test_post_memory_validation` — covers API-01 error path
- [ ] `test_search_memories` — covers API-02 (requires LocalEngine — slow test, tag with `#[ignore]` or use test_config with mock)
- [ ] `test_search_missing_q` — covers API-02 error path
- [ ] `test_list_memories` — covers API-03
- [ ] `test_delete_memory` — covers API-04
- [ ] `test_delete_not_found` — covers API-04 error path
- [ ] `test_health` — covers API-05 (already have `/health` route — simple test)
- [ ] `test_agent_isolation` — covers AGNT-01
- [ ] `test_session_filter` — covers AGNT-02
- [ ] `test_search_agent_filter` — covers AGNT-03

**Note on embedding in tests:** Search tests require an embedding engine. Options:
1. Use a `MockEmbeddingEngine` (deterministic random vectors) for fast unit tests
2. Use the shared `LOCAL_ENGINE` OnceLock pattern already in integration.rs for full semantic tests
Recommended: Add `MockEmbeddingEngine` struct to `tests/integration.rs` for API tests; keep semantic accuracy tests using `LOCAL_ENGINE`.

---

## Sources

### Primary (HIGH confidence)

- Zerocopy 0.8.47 source (`impls.rs:79`) — confirms `f32: IntoBytes` without `float-nightly` feature
- sqlite-vec demo.rs — confirms `.as_bytes()` pattern for Vec<f32> binding
- Existing `src/db.rs` — confirms schema: `memories` + `vec_memories` with `memory_id TEXT PRIMARY KEY`
- Existing `Cargo.toml` — confirms uuid v1.22.0 with v7 feature already present
- `cargo metadata` — confirms all dependency versions
- `cargo test` output — confirms 10 tests passing on current codebase

### Secondary (MEDIUM confidence)

- [sqlite-vec KNN docs](https://alexgarcia.xyz/sqlite-vec/features/knn.html) — CTE + JOIN pattern for agent filtering
- [sqlite-vec metadata blog post](https://alexgarcia.xyz/blog/2024/sqlite-vec-metadata-release/index.html) — v0.1.6 metadata columns (NOT applicable to our schema but documents the alternative)
- [sqlite-vec issue #196](https://github.com/asg017/sqlite-vec/issues/196) — confirms JOIN/CTE is the recommended pattern when metadata is in external table
- [axum error handling example](https://github.com/tokio-rs/axum/blob/main/examples/error-handling/src/main.rs) — IntoResponse pattern
- [axum testing example](https://github.com/tokio-rs/axum/blob/main/examples/testing/src/main.rs) — oneshot integration test pattern
- [axum extractors docs](https://docs.rs/axum/latest/axum/extract/index.html) — Query, Json, Path, State ordering rules
- [uuid docs](https://docs.rs/uuid/latest/uuid/struct.Uuid.html) — `Uuid::now_v7()` requires only `v7` feature (no `uuid_unstable` needed in v1.x)

### Tertiary (LOW confidence)

- None — all critical claims verified against source/official docs.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions verified via `cargo metadata`, all libs already in dep tree
- Architecture: HIGH — patterns verified from axum official examples and project's own established patterns
- sqlite-vec KNN + agent_id filter: HIGH — CTE + JOIN pattern confirmed by official docs and issue #196; f32 IntoBytes confirmed from zerocopy source
- Pitfalls: HIGH — sourced from official docs or confirmed by reading project source
- Test patterns: HIGH — axum official testing example, existing integration.rs patterns

**Research date:** 2026-03-19
**Valid until:** 2026-09-19 (stable ecosystem — axum, sqlite-vec, zerocopy are stable)
