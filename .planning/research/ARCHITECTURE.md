# Architecture Research

**Domain:** Rust single-binary agent memory server — v1.4 pluggable storage backends
**Researched:** 2026-03-21
**Confidence:** HIGH (direct code inspection of v1.3 codebase + verified against official Rust docs and crate documentation)

---

## Context: What Already Exists (v1.3)

The v1.3 binary is 22,198 lines of Rust across 12 source files. The storage layer is a direct SQLite dependency woven into both services:

```
MemoryService {
    db: Arc<tokio_rusqlite::Connection>,   // direct SQLite handle
    embedding: Arc<dyn EmbeddingEngine>,
    embedding_model: String,
}

CompactionService {
    db: Arc<tokio_rusqlite::Connection>,   // same direct SQLite handle
    embedding: Arc<dyn EmbeddingEngine>,
    summarization: Option<Arc<dyn SummarizationEngine>>,
    embedding_model: String,
}

AppState {
    service:     Arc<MemoryService>,
    compaction:  Arc<CompactionService>,
    key_service: Arc<KeyService>,         // also holds Arc<Connection>
}
```

Every method in `MemoryService` and `CompactionService` calls `self.db.call(|c| { ... })` directly — raw rusqlite closures with SQL strings. The sqlite-vec `MATCH` syntax (`embedding MATCH ?1`) is SQLite-specific and has no direct equivalent in Qdrant or Postgres (which use different APIs entirely).

**The key precedent already established by this codebase:** The `EmbeddingEngine` and `SummarizationEngine` traits show the exact pattern to replicate. Both are `#[async_trait]` object-safe traits held as `Arc<dyn Trait>`, selected by config at startup, and passed into services. The storage backend trait should follow this exact pattern.

---

## v1.4 System Overview

```
┌────────────────────────────────────────────────────────────────────────────┐
│                           Entry Point (main.rs)                             │
│                                                                             │
│   config::load_config() → storage_provider = "sqlite"|"qdrant"|"postgres"  │
│                                                                             │
│   build backend: Arc<dyn StorageBackend>                                    │
│     ├── "sqlite"   → SqliteBackend::new(conn)                               │
│     ├── "qdrant"   → QdrantBackend::new(url, api_key, collection)           │
│     └── "postgres" → PostgresBackend::new(conn_pool)                        │
│                                                                             │
│   MemoryService::new(backend, embedding)                                    │
│   CompactionService::new(backend, embedding, llm)                           │
└────────────────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────▼───────────────┐
              │     Service Layer (unchanged)   │
              │                                 │
              │  MemoryService                  │
              │    create_memory()              │
              │    search_memories()            │
              │    list_memories()              │
              │    delete_memory()              │
              │    get_memory_agent_id()        │
              │                                 │
              │  CompactionService              │
              │    compact()                    │
              │    fetch_candidates()           │
              └───────────────┬───────────────┘
                              │  calls Arc<dyn StorageBackend>
              ┌───────────────▼───────────────┐
              │   Storage Abstraction Layer     │
              │   trait StorageBackend          │
              │   (storage.rs — NEW)            │
              └───────┬──────────┬─────────────┘
                      │          │
      ┌───────────────▼──┐  ┌───▼──────────────────┐
      │  SqliteBackend   │  │  QdrantBackend (feat) │
      │  (storage/       │  │  (storage/qdrant.rs)  │
      │   sqlite.rs)     │  │                       │
      │  tokio-rusqlite  │  │  qdrant-client        │
      │  sqlite-vec MATCH│  │  gRPC upsert/query    │
      └──────────────────┘  └───────────────────────┘
                                    │
                         ┌──────────▼──────────────┐
                         │  PostgresBackend (feat)  │
                         │  (storage/postgres.rs)   │
                         │  sqlx + pgvector         │
                         │  <-> operator            │
                         └─────────────────────────┘
```

---

## The StorageBackend Trait

This is the core design decision for v1.4. The trait must cover exactly the operations currently scattered across `MemoryService` and `CompactionService` raw `db.call()` closures.

### Required Operations (derived from current codebase)

From `MemoryService`:
- Insert a memory with its embedding
- KNN vector search with metadata filters (agent_id, session_id, tag, after, before)
- List memories with metadata filters (paginated)
- Fetch agent_id for a single memory (ownership check)
- Delete a memory by id (memory row + vector row, atomic)

From `CompactionService`:
- Fetch all candidates for an agent with their embeddings (for clustering)
- Atomic batch write: insert N new merged memories + delete M source memories

From `KeyService` (auth.rs): KeyService currently holds `Arc<Connection>` directly. The auth tables (`api_keys`) are SQLite-specific and do not belong in the storage backend trait. KeyService stays as-is and retains its direct SQLite connection — this is correct because API key management is always local metadata regardless of where memories are stored.

### Trait Definition

```rust
// src/storage.rs (NEW)

use async_trait::async_trait;
use crate::error::StorageError;
use crate::service::{Memory, ListParams, SearchResultItem, SearchParams};

/// A candidate memory fetched for compaction, including raw embedding bytes.
pub struct MemoryCandidate {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub embedding: Vec<f32>,
}

/// A new memory to be written by MemoryService::create_memory.
pub struct NewMemory {
    pub id: String,
    pub content: String,
    pub agent_id: String,
    pub session_id: String,
    pub tags: Vec<String>,
    pub embedding: Vec<f32>,
    pub embedding_model: String,
}

/// A merged cluster to be atomically written during compaction.
pub struct MergedMemory {
    pub new_id: String,
    pub content: String,
    pub agent_id: String,
    pub tags: Vec<String>,
    pub embedding: Vec<f32>,
    pub embedding_model: String,
    pub created_at: String,        // earliest created_at from sources
    pub source_ids: Vec<String>,   // ids to delete atomically
}

/// Pluggable storage backend for Mnemonic.
///
/// All implementations must be Send + Sync to be held as Arc<dyn StorageBackend>.
/// The async_trait macro transforms these into Pin<Box<dyn Future + Send + 'async_trait>>,
/// making them safe for dynamic dispatch in tokio multi-thread executors.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    // ── Schema / lifecycle ──────────────────────────────────────────────────
    /// Initialize schema (create tables/collections/indexes).
    /// Called once at startup. Idempotent — safe to call on an existing store.
    async fn initialize(&self) -> Result<(), StorageError>;

    // ── Memory CRUD ─────────────────────────────────────────────────────────
    /// Insert a new memory atomically (content row + vector entry).
    async fn insert_memory(&self, memory: NewMemory) -> Result<String, StorageError>;

    /// Fetch one memory by id. Returns None if not found.
    async fn get_memory(&self, id: &str) -> Result<Option<Memory>, StorageError>;

    /// Fetch only the agent_id for a memory (used by delete ownership check).
    async fn get_memory_agent_id(&self, id: &str) -> Result<Option<String>, StorageError>;

    /// Delete a memory by id (content + vector, atomic).
    /// Returns the deleted Memory, or None if not found.
    async fn delete_memory(&self, id: &str) -> Result<Option<Memory>, StorageError>;

    // ── Query ───────────────────────────────────────────────────────────────
    /// Paginated list with metadata filters (no vector search).
    async fn list_memories(
        &self,
        params: ListParams,
    ) -> Result<(Vec<Memory>, u64), StorageError>;

    /// KNN vector search with metadata filters and optional distance threshold.
    async fn search_memories(
        &self,
        query_embedding: Vec<f32>,
        params: SearchParams,
    ) -> Result<Vec<SearchResultItem>, StorageError>;

    // ── Compaction ──────────────────────────────────────────────────────────
    /// Fetch all memories for an agent with their embeddings (for clustering).
    /// Returns (candidates, truncated).
    async fn fetch_compaction_candidates(
        &self,
        agent_id: &str,
        max_candidates: u32,
    ) -> Result<(Vec<MemoryCandidate>, bool), StorageError>;

    /// Atomically delete source memories and insert merged replacements.
    /// All operations succeed or all fail.
    async fn apply_compaction(
        &self,
        merges: Vec<MergedMemory>,
    ) -> Result<(), StorageError>;

    // ── Audit log ───────────────────────────────────────────────────────────
    /// Record the start of a compaction run.
    async fn create_compact_run(
        &self,
        run_id: &str,
        agent_id: &str,
        threshold: f32,
        dry_run: bool,
    ) -> Result<(), StorageError>;

    /// Update a compaction run record to completed/failed.
    async fn finish_compact_run(
        &self,
        run_id: &str,
        status: &str,
        clusters_found: u32,
        memories_merged: u32,
        memories_created: u32,
    ) -> Result<(), StorageError>;
}
```

### Why This Trait Shape

**`search_memories` receives a pre-computed `query_embedding`:** The service layer calls `self.embedding.embed(query)` before calling the backend. The backend only handles vector storage and retrieval — it does not embed text. This keeps the trait backend-agnostic and mirrors how sqlite-vec, Qdrant, and pgvector all work: they accept a float vector and return nearest neighbors.

**`apply_compaction` takes `Vec<MergedMemory>`:** The compaction clustering logic (similarity matrix, greedy clustering, content synthesis) stays in `CompactionService` — it is embedding-dependent business logic, not storage logic. The backend only receives the finalized write operations. This keeps the clustering algorithm portable across backends.

**`compact_runs` audit log in the trait:** The SQLite backend stores this in a real table. Qdrant and Postgres backends should also persist audit records somewhere. For v1.4, a simple approach is: Qdrant backend writes compact_run records to a companion SQLite file (since Qdrant is not a relational store), while Postgres backend uses a normal table. This is an implementation detail per backend — the trait just declares the capability.

**`KeyService` stays out of the trait:** API keys are always local metadata. Even when using Qdrant for memories, the user's key store should remain on the local machine. `KeyService` keeps its `Arc<tokio_rusqlite::Connection>` unchanged.

---

## Async Trait and Send Bounds

**Confidence: HIGH** — verified against `async-trait` crate docs and existing usage in this codebase.

Rust's native `async fn` in traits (stabilized 1.75) does not yet support `dyn Trait` dispatch. The `async-trait` macro is the correct solution for `Arc<dyn StorageBackend>`.

The project already uses `async-trait = "0.1"` in `Cargo.toml` for `EmbeddingEngine` and `SummarizationEngine`. The storage backend trait follows the identical pattern:

```rust
// This is what async-trait generates under the hood:
// fn insert_memory(&self, memory: NewMemory)
//     -> Pin<Box<dyn Future<Output = Result<String, StorageError>> + Send + 'async_trait>>
```

**Critical Send requirement:** All backend implementations (`SqliteBackend`, `QdrantBackend`, `PostgresBackend`) must be `Send + Sync` because they are held as `Arc<dyn StorageBackend + Send + Sync>` in `AppState`. This is safe for:

- `SqliteBackend`: wraps `Arc<tokio_rusqlite::Connection>` which is already `Send + Sync` (the `tokio-rusqlite` crate guarantees this — its whole purpose is async-safe SQLite access).
- `QdrantBackend`: wraps `qdrant_client::Qdrant` which is `Send + Sync` (gRPC client backed by tonic/hyper).
- `PostgresBackend`: wraps `sqlx::PgPool` which is `Send + Sync` (designed for concurrent async access).

**`#[async_trait(?Send)]` is NOT appropriate here:** The `?Send` form is for single-threaded executors. Axum uses `tokio::runtime::Builder::new_multi_thread()`, so all futures crossing `.await` points must be `Send`.

---

## How Config Selects the Backend

The existing `Config` struct grows three new optional fields:

```rust
// src/config.rs — additions

pub struct Config {
    // ... existing fields ...
    pub storage_provider: String,     // "sqlite" (default), "qdrant", "postgres"
    pub qdrant_url: Option<String>,   // required when storage_provider = "qdrant"
    pub qdrant_api_key: Option<String>,
    pub qdrant_collection: Option<String>,
    pub postgres_url: Option<String>, // required when storage_provider = "postgres"
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // ...
            storage_provider: "sqlite".to_string(),
            qdrant_url: None,
            qdrant_api_key: None,
            qdrant_collection: None,
            postgres_url: None,
        }
    }
}
```

`validate_config()` adds a new match block:

```rust
match config.storage_provider.as_str() {
    "sqlite" => {}
    "qdrant" => {
        if config.qdrant_url.is_none() {
            anyhow::bail!("storage_provider=qdrant requires qdrant_url");
        }
    }
    "postgres" => {
        if config.postgres_url.is_none() {
            anyhow::bail!("storage_provider=postgres requires postgres_url");
        }
    }
    other => anyhow::bail!("unknown storage_provider: {:?}", other),
}
```

Backend construction in `main.rs` follows the same pattern as embedding engine selection:

```rust
let storage: Arc<dyn StorageBackend> = match config.storage_provider.as_str() {
    "sqlite" => {
        let conn = Arc::new(db::open(&config).await?);
        Arc::new(SqliteBackend::new(conn))
    }
    "qdrant" => {
        let backend = QdrantBackend::new(
            config.qdrant_url.as_ref().unwrap(),
            config.qdrant_api_key.clone(),
            config.qdrant_collection.clone().unwrap_or("mnemonic".to_string()),
        ).await?;
        backend.initialize().await?;
        Arc::new(backend)
    }
    "postgres" => {
        let backend = PostgresBackend::new(
            config.postgres_url.as_ref().unwrap()
        ).await?;
        backend.initialize().await?;
        Arc::new(backend)
    }
    _ => unreachable!(), // validate_config rejects unknown providers
};
```

---

## How AppState Changes

```rust
// BEFORE (v1.3):
pub struct AppState {
    pub service:     Arc<MemoryService>,        // owns Arc<Connection>
    pub compaction:  Arc<CompactionService>,    // owns Arc<Connection>
    pub key_service: Arc<KeyService>,           // owns Arc<Connection>
}

// AFTER (v1.4):
pub struct AppState {
    pub service:     Arc<MemoryService>,        // owns Arc<dyn StorageBackend>
    pub compaction:  Arc<CompactionService>,    // owns Arc<dyn StorageBackend>
    pub key_service: Arc<KeyService>,           // unchanged — still Arc<Connection>
}
```

`MemoryService` and `CompactionService` constructors change signature:

```rust
// BEFORE:
impl MemoryService {
    pub fn new(
        db: Arc<tokio_rusqlite::Connection>,
        embedding: Arc<dyn EmbeddingEngine>,
        embedding_model: String,
    ) -> Self

// AFTER:
impl MemoryService {
    pub fn new(
        storage: Arc<dyn StorageBackend>,
        embedding: Arc<dyn EmbeddingEngine>,
        embedding_model: String,
    ) -> Self
```

---

## How MemoryService Is Refactored

Each existing method in `MemoryService` that calls `self.db.call(|c| { ... })` becomes a delegation to `self.storage`:

```rust
// BEFORE (direct rusqlite):
pub async fn create_memory(&self, req: CreateMemoryRequest) -> Result<Memory, ApiError> {
    let embedding = self.embedding.embed(&req.content).await?;
    let embedding_bytes: Vec<u8> = embedding.as_bytes().to_vec();
    let id = uuid::Uuid::now_v7().to_string();
    // ... build params ...
    let created_at = self.db.call(move |c| -> Result<String, rusqlite::Error> {
        let tx = c.transaction()?;
        tx.execute("INSERT INTO memories ...", params![...])?;
        tx.execute("INSERT INTO vec_memories ...", params![...])?;
        // ...
        tx.commit()?;
        Ok(created_at)
    }).await?;
    Ok(Memory { ... })
}

// AFTER (delegated to backend):
pub async fn create_memory(&self, req: CreateMemoryRequest) -> Result<Memory, ApiError> {
    let embedding = self.embedding.embed(&req.content).await?;
    let id = uuid::Uuid::now_v7().to_string();
    let new_memory = NewMemory {
        id: id.clone(),
        content: req.content.clone(),
        agent_id: req.agent_id.clone().unwrap_or_default(),
        session_id: req.session_id.clone().unwrap_or_default(),
        tags: req.tags.clone().unwrap_or_default(),
        embedding,
        embedding_model: self.embedding_model.clone(),
    };
    let created_at = self.storage.insert_memory(new_memory).await
        .map_err(ApiError::from)?;
    Ok(Memory { id, content: req.content, ..., created_at })
}
```

The business logic (embedding, ID generation, request validation, response shaping) stays in `MemoryService`. Only the raw storage calls move to the backend implementations.

### search_memories refactor

The key difference from a simple delegation is that `search_memories` currently receives raw SQL parameters and builds the KNN query inline. In v1.4:

1. `MemoryService::search_memories` calls `self.embedding.embed(&q)` to get the query vector.
2. It then calls `self.storage.search_memories(query_embedding, params)`.
3. Each backend implements the vector search in its own way:
   - `SqliteBackend`: `sqlite-vec MATCH` with a CTE and metadata JOIN.
   - `QdrantBackend`: `client.query(QueryPointsBuilder::new(collection).query(vector).filter(payload_filter))`.
   - `PostgresBackend`: `SELECT ... ORDER BY embedding <-> $1 LIMIT $2` with WHERE clause filters.

The threshold filtering (distance <= threshold) can stay in `MemoryService` after the backend returns results, or be passed into the backend as a parameter. Recommended: pass `threshold: Option<f32>` into `search_memories` so each backend can apply it at the query level (Qdrant has native score thresholds; Postgres can use a WHERE clause; SQLite does it post-fetch). This avoids fetching then filtering in the service layer.

---

## How CompactionService Is Refactored

CompactionService has two storage-intensive operations:

**`fetch_candidates`** becomes `self.storage.fetch_compaction_candidates(agent_id, max_candidates)`. The backend returns `Vec<MemoryCandidate>` including the raw `Vec<f32>` embedding. Each backend fetches embeddings differently:
- SQLite: existing query joining `memories` and `vec_memories` with `unsafe` f32 cast from bytes.
- Qdrant: `scroll` or `query` with `with_vectors: true` to retrieve stored vectors.
- Postgres: `SELECT ... embedding::float[]` with pgvector.

The clustering math (`compute_pairs`, `cluster_candidates`, `cosine_similarity`) remains in `CompactionService` — it is pure Rust, embedding-dependent, and backend-agnostic.

**The atomic write** (`apply_compaction`) takes the finalized `Vec<MergedMemory>` to the backend. The backend handles atomicity:
- SQLite: single transaction, same as today.
- Qdrant: no native transactions — use sequential upsert then delete; document that Qdrant is eventually consistent during compaction (crash between write and delete leaves orphan source records; acceptable for v1.4).
- Postgres: full transaction via `sqlx::Transaction`.

The compact_run audit log calls map to `self.storage.create_compact_run(...)` and `self.storage.finish_compact_run(...)`. For Qdrant, this is stored in a companion SQLite file — the `QdrantBackend` can hold an internal `Arc<tokio_rusqlite::Connection>` for this purpose only.

---

## Recommended Project Structure (v1.4 delta)

```
src/
├── main.rs           # MODIFIED: backend selection logic added
├── cli.rs            # MODIFIED: add Config subcommand (mnemonic config)
├── config.rs         # MODIFIED: add storage_provider + backend fields
├── error.rs          # MODIFIED: add StorageError variant
├── service.rs        # MODIFIED: swap Arc<Connection> for Arc<dyn StorageBackend>
├── compaction.rs     # MODIFIED: swap Arc<Connection> for Arc<dyn StorageBackend>
├── storage.rs        # NEW: StorageBackend trait + type definitions
├── storage/          # NEW: backend implementations
│   ├── mod.rs        # re-exports
│   ├── sqlite.rs     # SqliteBackend (wraps existing db.rs logic)
│   ├── qdrant.rs     # QdrantBackend (feature = "qdrant")
│   └── postgres.rs   # PostgresBackend (feature = "postgres")
├── db.rs             # MODIFIED: open() used by SqliteBackend only; may be folded into storage/sqlite.rs
├── server.rs         # NO CHANGE: AppState field types change but Clone/Send bounds unchanged
├── auth.rs           # NO CHANGE: KeyService retains Arc<Connection>
├── embedding.rs      # NO CHANGE
├── summarization.rs  # NO CHANGE
└── lib.rs            # MODIFIED: re-export storage module
```

### Feature Flags

Non-default backends should be behind Cargo feature flags to keep the binary slim when only SQLite is needed:

```toml
# Cargo.toml additions
[features]
default = []
qdrant = ["dep:qdrant-client"]
postgres = ["dep:sqlx", "dep:pgvector"]

[dependencies]
qdrant-client = { version = "1", optional = true }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid"], optional = true }
pgvector = { version = "0.4", features = ["sqlx"], optional = true }
```

**Why feature flags:** The single-binary promise means users who only want SQLite should not compile in gRPC (tonic) or libpq. Feature flags are the standard Rust solution. The release CI would produce three artifacts: `mnemonic` (SQLite only), `mnemonic-qdrant`, `mnemonic-postgres`.

---

## Architectural Patterns

### Pattern 1: Trait-Object Storage (Repository Pattern)

**What:** `Arc<dyn StorageBackend + Send + Sync>` is the single reference passed to both `MemoryService` and `CompactionService`. Selection happens once in `main.rs` at startup.

**When to use:** Whenever a system has genuinely swappable implementations that differ in their underlying I/O mechanism. The EmbeddingEngine and SummarizationEngine traits in this codebase already prove this pattern compiles and works at runtime.

**Trade-offs:** Dynamic dispatch adds one pointer indirection per call (negligible vs. I/O latency). Each storage call is already at least one async context switch, so the vtable lookup is immeasurably small. Against: monomorphization would give zero-cost generics, but would require `MemoryService<S: StorageBackend>` everywhere, making AppState non-clonable without bounds propagation. `Arc<dyn Trait>` is cleaner for this use case.

**Example:**
```rust
// In service.rs — no change in call sites, only in what `storage` points to:
let result = self.storage.insert_memory(new_memory).await?;
```

### Pattern 2: Backend-as-Leaf, Logic-in-Service

**What:** The storage backend is a pure I/O leaf. All business logic — embedding, ID generation, clustering algorithms, content synthesis — stays in the service layer. Backends are not allowed to call back into services or embed text.

**When to use:** Always. If a backend starts containing business logic, it becomes impossible to swap backends without reimplementing that logic.

**Trade-offs:** The service layer and backend must agree on data types (`NewMemory`, `MemoryCandidate`, etc.). These structs become the API contract and must be designed carefully upfront. Changing them later requires updating all backend implementations.

### Pattern 3: Companion SQLite for Qdrant Metadata

**What:** The `QdrantBackend` holds an internal `Arc<tokio_rusqlite::Connection>` for storing compact_run audit records and potentially API key associations. Qdrant is a vector-only store — it is not suited for relational metadata.

**When to use:** When using a specialized vector store that lacks the relational capabilities needed for audit/auth metadata.

**Trade-offs:** Two storage systems instead of one. The companion SQLite file is small (audit records only) and its path defaults to the same directory as `mnemonic.db` would have been. Users need to manage two files instead of one, but this is an inherent trade-off of using a dedicated vector store.

### Pattern 4: Mirroring EmbeddingEngine Pattern Exactly

**What:** `StorageBackend` follows the same design as `EmbeddingEngine`: `#[async_trait]` on the trait, `Arc<dyn StorageBackend>` everywhere, selected by config string at startup, Mock implementation for tests.

**When to use:** Consistency is a feature. New contributors can understand the storage backend pattern by reading the embedding engine code they already know.

**Trade-offs:** The pattern is slightly verbose (each impl requires `#[async_trait]` and the `impl StorageBackend for ...` block), but no more so than what already exists in `embedding.rs`.

---

## Data Flow: Request Through the New Layer

### POST /memories (create)

```
HTTP POST /memories { content, agent_id, ... }
  |
  v
create_memory_handler (server.rs)
  └── state.service.create_memory(req)
        ├── self.embedding.embed(content)     → Vec<f32>
        ├── uuid::Uuid::now_v7()              → id
        └── self.storage.insert_memory(NewMemory { id, content, embedding, ... })
              ├── SqliteBackend:   db.call(|c| { tx.execute INSERT memories; tx.execute INSERT vec_memories; tx.commit() })
              ├── QdrantBackend:   client.upsert_points(collection, [PointStruct { id, vector, payload }])
              └── PostgresBackend: pool.begin(); INSERT INTO memories ...; INSERT INTO embeddings ...; commit()
              → returns created_at: String
        → Memory { id, content, agent_id, ..., created_at }
  |
  v
201 Created { memory }
```

### GET /memories/search (semantic search)

```
HTTP GET /memories/search?q=...&agent_id=...
  |
  v
search_memories_handler (server.rs)
  └── state.service.search_memories(params)
        ├── self.embedding.embed(params.q)    → Vec<f32> (query vector)
        └── self.storage.search_memories(query_vec, params)
              ├── SqliteBackend:   CTE with vec_memories MATCH + JOIN memories WHERE filters
              ├── QdrantBackend:   client.query(QueryPointsBuilder.query(vec).filter(payload_filter))
              └── PostgresBackend: SELECT ... ORDER BY embedding <-> $1 WHERE agent_id = $2 LIMIT $3
              → Vec<SearchResultItem> (with distance/score)
  |
  v
200 OK { memories: [...] }
```

### POST /memories/compact

```
POST /memories/compact { agent_id, threshold, max_candidates, dry_run }
  |
  v
compact_memories_handler (server.rs)
  └── state.compaction.compact(req)
        ├── self.storage.create_compact_run(run_id, agent_id, threshold, dry_run)
        ├── self.storage.fetch_compaction_candidates(agent_id, max_candidates)
        │     → (Vec<MemoryCandidate>, truncated)
        │       each candidate has .embedding: Vec<f32>
        ├── compute_pairs(&candidates, threshold)    [pure Rust, unchanged]
        ├── cluster_candidates(&pairs, ...)          [pure Rust, unchanged]
        ├── for each cluster: synthesize_content()  [LLM or concat, unchanged]
        ├── self.embedding.embed(merged_content)     [unchanged]
        └── if !dry_run:
              self.storage.apply_compaction(Vec<MergedMemory>)
                ├── SqliteBackend:   single transaction: INSERT new + DELETE sources
                ├── QdrantBackend:   upsert new → delete sources (no TX; document risk)
                └── PostgresBackend: full sqlx transaction
        └── self.storage.finish_compact_run(run_id, "completed", ...)
  |
  v
200 OK { run_id, clusters_found, memories_merged, ... }
```

---

## Mapping sqlite-vec MATCH to Other Backends

This is the most technically sensitive translation. Confidence: MEDIUM (API shapes verified against official docs, but not integration-tested).

| Operation | SQLite + sqlite-vec | Qdrant | Postgres + pgvector |
|-----------|--------------------|---------|--------------------|
| Insert vector | `INSERT INTO vec_memories (memory_id, embedding) VALUES (?, ?)` with raw bytes | `client.upsert_points(collection, [PointStruct::new(id, vec, payload)])` | `INSERT INTO embeddings (memory_id, embedding) VALUES ($1, $2)` with `pgvector::Vector` |
| KNN search | `SELECT ... FROM vec_memories WHERE embedding MATCH ? AND k = ?` | `client.query(QueryPointsBuilder::new(col).query(vec).limit(k).filter(...))` | `SELECT ... ORDER BY embedding <-> $1 LIMIT $2` |
| Filter by metadata | CTE post-filter: `JOIN memories ON id WHERE agent_id = ?` | `Filter::all([Condition::matches("agent_id", value)])` in payload | `WHERE agent_id = $3` in same query |
| Distance/score | `knn_candidates.distance` (L2 distance from sqlite-vec) | `result.points[i].score` (cosine similarity, higher = better) | `embedding <-> $1 AS distance` (L2 by default) |
| Distance semantics | **Lower = more similar** (L2 distance) | **Higher = more similar** (score 0.0-1.0 for cosine) | **Lower = more similar** (L2 distance) |
| Threshold | `WHERE distance <= threshold` | `QueryPointsBuilder::score_threshold(threshold)` | `WHERE embedding <-> $1 <= $2` |

**Distance direction mismatch:** This is a real gotcha. The current `SearchResultItem.distance` field and the compaction threshold are defined in terms of "lower = more similar" (L2 distance). Qdrant returns scores where "higher = more similar." The `QdrantBackend::search_memories` implementation must invert scores (e.g., `1.0 - score` for normalized cosine) before returning `SearchResultItem`. The `StorageBackend` contract should define that `distance` in results always follows "lower = more similar" semantics.

**K over-fetch for filters:** The current SQLite implementation fetches `k = limit * 10` when agent_id or session_id filters are present (because post-KNN filtering can drop results). All backends must implement this same over-fetch logic internally — it is a correct behavior requirement, not a SQLite-specific workaround.

---

## Integration Points: New vs. Modified Components

| Component | Status | What Changes |
|-----------|--------|--------------|
| `storage.rs` | NEW | `StorageBackend` trait definition + `NewMemory`, `MemoryCandidate`, `MergedMemory` structs |
| `storage/sqlite.rs` | NEW | `SqliteBackend` — moves raw `db.call()` logic from service.rs and compaction.rs |
| `storage/qdrant.rs` | NEW (feature-gated) | `QdrantBackend` — qdrant-client gRPC calls |
| `storage/postgres.rs` | NEW (feature-gated) | `PostgresBackend` — sqlx + pgvector queries |
| `service.rs` | MODIFIED | Replace `Arc<Connection>` with `Arc<dyn StorageBackend>`; each method delegates to `self.storage` |
| `compaction.rs` | MODIFIED | Replace `Arc<Connection>` with `Arc<dyn StorageBackend>`; `fetch_candidates` and atomic write delegate to `self.storage` |
| `config.rs` | MODIFIED | Add `storage_provider`, `qdrant_url`, `qdrant_api_key`, `qdrant_collection`, `postgres_url` fields |
| `error.rs` | MODIFIED | Add `StorageError` type; add `From<StorageError> for ApiError` |
| `main.rs` | MODIFIED | Add backend construction block after config load |
| `cli.rs` | MODIFIED | Add `Config` subcommand for viewing/switching backend settings |
| `db.rs` | MODIFIED | `open()` called by `SqliteBackend` only; register_sqlite_vec() may move to SqliteBackend::new() |
| `Cargo.toml` | MODIFIED | Add optional `qdrant-client`, `sqlx`, `pgvector` dependencies behind features |
| `auth.rs` | NO CHANGE | KeyService retains direct `Arc<Connection>` |
| `server.rs` | NO CHANGE | AppState field types change but the struct shape is identical |
| `embedding.rs` | NO CHANGE | |
| `summarization.rs` | NO CHANGE | |

---

## Build Order (v1.4 phases, considering dependencies)

```
Phase A — StorageBackend trait + SqliteBackend (zero behavior change)
  1. error.rs: add StorageError, From<StorageError> for ApiError
  2. storage.rs: define trait + NewMemory/MemoryCandidate/MergedMemory structs
  3. storage/sqlite.rs: SqliteBackend — move all raw db.call() logic from service.rs
     and compaction.rs into the backend impl; tests pass unchanged
  4. service.rs: swap Arc<Connection> for Arc<dyn StorageBackend>;
     delegate to self.storage instead of self.db.call()
  5. compaction.rs: same swap; fetch_candidates and apply_compaction delegate
  6. main.rs: construct SqliteBackend; pass Arc<dyn StorageBackend> to services
  Result: binary behaves identically to v1.3; all 239 tests pass
  Test: run full test suite — zero behavior change

Phase B — Config extension + validate_config
  1. config.rs: add storage_provider + backend-specific fields
  2. validate_config(): add storage_provider match block
  3. cli.rs: add mnemonic config subcommand (view current backend + switch)
  Result: mnemonic config show works; storage_provider=sqlite is the only valid option still

Phase C — Qdrant backend (feature-gated)
  1. Cargo.toml: add qdrant-client optional dep + "qdrant" feature
  2. storage/qdrant.rs: QdrantBackend implementing StorageBackend
     - initialize(): create collection with 384-dim cosine vectors
     - insert_memory(): upsert PointStruct with payload {agent_id, session_id, tags, ...}
     - search_memories(): query() with payload filter, convert score to distance
     - list_memories(): scroll() with payload filter (no vector; pagination via offset)
     - delete_memory(): delete_points() by ID
     - fetch_compaction_candidates(): query with_vectors=true
     - apply_compaction(): upsert new + delete sources + companion SQLite for audit log
  3. main.rs: extend backend construction block for "qdrant" arm (cfg-gated)
  Result: mnemonic --features qdrant with storage_provider=qdrant works
  Test: integration test against local Qdrant Docker container

Phase D — Postgres backend (feature-gated)
  1. Cargo.toml: add sqlx + pgvector optional deps + "postgres" feature
  2. storage/postgres.rs: PostgresBackend implementing StorageBackend
     - initialize(): CREATE EXTENSION IF NOT EXISTS vector; CREATE TABLE IF NOT EXISTS ...
     - insert_memory(): INSERT into memories + embeddings tables within sqlx transaction
     - search_memories(): SELECT ... ORDER BY embedding <-> $1 WHERE ... LIMIT $2
     - list_memories(): SELECT with WHERE clause + COUNT(*)
     - delete_memory(): DELETE within transaction
     - fetch_compaction_candidates(): SELECT with embedding column
     - apply_compaction(): full sqlx Transaction, insert + delete
     - compact audit: standard Postgres table
  3. main.rs: extend backend construction block for "postgres" arm (cfg-gated)
  Result: mnemonic --features postgres with storage_provider=postgres works
  Test: integration test against local Postgres + pgvector container

Phase E — Release artifacts
  1. GitHub Actions: add build matrix entries for qdrant and postgres feature variants
  2. Documentation: README backend switching section; mnemonic config help text
  Result: three published binary variants per platform
```

**Rationale for this ordering:**

Phase A is the highest-value step: it extracts SQLite behind the trait without changing any behavior. After Phase A, the codebase is structurally ready for new backends and all existing tests validate correctness. Phases C and D can be developed in parallel or by different contributors because the trait contract is fixed after Phase A. Phase B (config) is placed before C/D so that the backend-selection mechanism exists before any new backends are implemented.

---

## Anti-Patterns

### Anti-Pattern 1: Leaking rusqlite Types into the Trait

**What people do:** Define `StorageBackend::insert_memory(&self, c: &rusqlite::Connection, ...)` or include `rusqlite::Error` in the return type.

**Why it's wrong:** Ties the trait to one backend. `QdrantBackend` does not have a `rusqlite::Connection`. The entire point of the abstraction collapses.

**Do this instead:** `StorageError` is a project-specific error type. Each backend maps its native errors to `StorageError`. The trait never mentions `rusqlite`, `qdrant_client`, or `sqlx`.

### Anti-Pattern 2: Making StorageBackend Generic Instead of Dyn

**What people do:** `struct MemoryService<S: StorageBackend>` with monomorphized dispatch.

**Why it's wrong:** `AppState` would become `AppState<S: StorageBackend>`, which means `Router` and all axum handlers become generic too. The `Clone` requirement on `AppState` forces `S: Clone`. Every handler signature grows a bound. This is a large API surface change for a zero-latency benefit (the vtable lookup is invisible against I/O).

**Do this instead:** `Arc<dyn StorageBackend + Send + Sync>`. Identical to the existing `EmbeddingEngine` pattern. No generics needed.

### Anti-Pattern 3: Transactions Across the Trait Boundary

**What people do:** Add `fn begin_transaction(&self) -> Box<dyn Transaction>` to the trait.

**Why it's wrong:** Transactions are fundamentally different across backends. SQLite transactions are synchronous and scoped to a connection. Qdrant has no transactions. Postgres transactions are async and tied to a connection from the pool. Abstracting this correctly is extremely complex and was not needed.

**Do this instead:** Make `apply_compaction` the atomic write operation. The backend is responsible for making its own atomicity guarantees internally. The trait promises "all-or-nothing" semantics for `apply_compaction`; each backend implements that guarantee differently. Document the Qdrant caveat (best-effort, not transactional) explicitly.

### Anti-Pattern 4: Embedding Inside the Backend

**What people do:** Add `fn embed_and_insert(&self, text: &str)` to the trait, pulling `EmbeddingEngine` into the backend.

**Why it's wrong:** Backends should not know about embeddings. Embeddings are a service-layer concern. If the embedding model changes, the backend should not need to change. If the backend changes, the embedding logic should not need to move.

**Do this instead:** Service layer embeds text, passes `Vec<f32>` to the backend. Backends receive pre-computed vectors.

### Anti-Pattern 5: Placing KeyService Behind the Storage Trait

**What people do:** Add `create_key`, `validate_key`, `revoke_key` to `StorageBackend`.

**Why it's wrong:** API keys are always local metadata — they should not be stored in Qdrant or Postgres. A remote vector store should not gate local auth operations. Operators who switch to a remote backend would need to migrate their keys there too.

**Do this instead:** `KeyService` stays as-is, always backed by SQLite. When using Qdrant or Postgres for memories, the local SQLite file is still used for key management. This is correct — the key store is always local, the memory store may be remote.

---

## Scaling Considerations

| Scale | Storage Architecture Behavior |
|-------|-------------------------------|
| Single user, local | SQLite default. Zero config, zero external services. Identical to v1.3. |
| Small team, shared | Postgres backend with a shared Postgres instance. All agents share one pool. pgvector handles concurrent writes. |
| Large scale, high-throughput | Qdrant backend. Qdrant is purpose-built for high-throughput vector search, supports horizontal scaling and sharding. |
| Hybrid | Multiple mnemonic instances, each with their own SQLite. No shared state. Simpler than a central store. |

The scaling decision is externalized — the user selects the backend by config. The binary does not change.

---

## Sources

- Existing v1.3 source code (direct inspection) — HIGH confidence
- `async-trait` crate: https://docs.rs/async-trait/latest/async_trait/ — HIGH confidence
- Qdrant Rust client: https://docs.rs/qdrant-client/latest/qdrant_client/ — MEDIUM confidence (API verified, integration not tested)
- Qdrant Rust client README: https://github.com/qdrant/rust-client/blob/master/README.md — MEDIUM confidence
- pgvector Rust crate: https://github.com/pgvector/pgvector-rust — MEDIUM confidence (API verified, integration not tested)
- Rust async fn in dyn trait status: https://rust-lang.github.io/async-fundamentals-initiative/explainer/async_fn_in_dyn_trait.html — HIGH confidence (confirms async-trait is still required for dyn dispatch)
- EmbeddingEngine trait in this codebase (embedding.rs) — HIGH confidence (proven pattern to mirror)

---

*Architecture research for: Mnemonic v1.4 — pluggable storage backends*
*Researched: 2026-03-21*
