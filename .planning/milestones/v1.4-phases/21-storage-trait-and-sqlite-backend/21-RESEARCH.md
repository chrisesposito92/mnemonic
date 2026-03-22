# Phase 21: Storage Trait and SQLite Backend - Research

**Researched:** 2026-03-21
**Domain:** Rust async trait abstraction over SQLite storage (tokio-rusqlite)
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-01:** StorageBackend exposes one async method per logical storage operation, mirroring current MemoryService/CompactionService method boundaries: store, get_by_id, list, search, delete, plus compaction sub-operations (fetch_candidates, insert_merged_memory, delete_source_memories).

**D-02:** Search results use lower-is-better distance semantics — this is the trait contract. SqliteBackend passes through sqlite-vec distances directly. Future backends (Qdrant) must convert scores to distances via `1.0 - score`.

**D-03:** The trait uses `#[async_trait]` — native async fn in traits is not dyn-compatible as of early 2026.

**D-04:** New `src/storage/` module tree: `mod.rs` defines the `StorageBackend` trait and shared types, `sqlite.rs` contains `SqliteBackend` implementation.

**D-05:** `src/storage/mod.rs` re-exports `StorageBackend` trait and `SqliteBackend` so existing code can `use crate::storage::{StorageBackend, SqliteBackend}`.

**D-06:** Future backends (Qdrant, Postgres) will each get their own file in `src/storage/` behind feature gates — this phase only creates the structure and sqlite.rs.

**D-07:** `compact_runs` table is NOT part of the StorageBackend trait — it remains a separate concern handled by CompactionService directly via its own SQLite connection.

**D-08:** CompactionService keeps a dedicated `Arc<Connection>` for compact_runs audit logging, separate from the `Arc<dyn StorageBackend>` it uses for memory operations.

**D-09:** StorageBackend methods that need embeddings (store, search) accept pre-computed `Vec<f32>` — the trait does not handle embedding generation. MemoryService continues to own the `Arc<dyn EmbeddingEngine>`.

### Claude's Discretion

- Exact method signatures and error types for StorageBackend trait methods
- How to handle the dual-table insert pattern (memories + vec_memories) inside SqliteBackend
- Whether to introduce a StorageError type or reuse existing ApiError/DbError
- Test refactoring approach — how to adapt test helpers while keeping all 239 tests green
- Naming conventions for trait methods (e.g., `store_memory` vs `store` vs `create`)

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STOR-01 | StorageBackend async trait defines store, get, list, search, delete, and compact operations with normalized distance semantics | EmbeddingEngine trait pattern in src/embedding.rs is the direct template; #[async_trait] is already a dependency |
| STOR-02 | SqliteBackend implements StorageBackend by wrapping existing SQLite+sqlite-vec code with zero behavior change | All SQLite logic is isolated in service.rs and compaction.rs; moving it into SqliteBackend is mechanical extraction |
| STOR-03 | MemoryService holds Arc<dyn StorageBackend> instead of direct tokio-rusqlite connection | Constructor change + removal of db field; embedding field stays; trait methods replace inline db.call() blocks |
| STOR-04 | CompactionService uses StorageBackend trait methods instead of direct SQLite queries; keeps Arc<Connection> for compact_runs | D-07/D-08 define the split: backend Arc for memory ops, Connection Arc for audit log |
| STOR-05 | All 239 existing tests pass unchanged after trait refactor | 239 = 63 lib (run twice as lib+bin) + 55 cli_integration + 4 error_types + 54 integration passing + 1 ignored = 239 passing total |
</phase_requirements>

---

## Summary

Phase 21 is a pure refactor: extract all SQLite memory operations from MemoryService and CompactionService into a new `SqliteBackend` struct that implements a new `StorageBackend` async trait, then make both services hold `Arc<dyn StorageBackend>`. The existing `EmbeddingEngine` trait in `src/embedding.rs` is the exact template to follow — `#[async_trait]`, `Send + Sync` bounds, used everywhere as `Arc<dyn T>`. No new dependencies are required; `async-trait` is already in Cargo.toml.

The critical constraint is zero behavior change. sqlite-vec distances are already lower-is-better (the MATCH operator returns L2 distances), so `SqliteBackend` passes them through unchanged. The dual-table invariant (memories + vec_memories always updated atomically in a single transaction) must be preserved inside `SqliteBackend` — the trait boundary does not change transaction semantics.

The most surgical risk in this refactor is CompactionService's dual-connection design (D-07/D-08): it must hold both `Arc<dyn StorageBackend>` for memory operations AND `Arc<Connection>` for compact_runs audit writes. This is intentional and well-reasoned — it avoids requiring every future backend to implement SQL audit tables. The test infrastructure in `tests/integration.rs` directly constructs `MemoryService::new()` and `CompactionService::new()` with concrete argument types, so those constructor signatures must be updated carefully to keep all 239 tests green.

**Primary recommendation:** Follow the EmbeddingEngine pattern exactly. Define trait in `src/storage/mod.rs`, implement in `src/storage/sqlite.rs`, update constructors, add `pub mod storage;` to both `src/lib.rs` and `src/main.rs`. The refactor is mechanical and low-risk if done module by module.

---

## Standard Stack

### Core (already in Cargo.toml — no new dependencies needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| async-trait | 0.1 | `#[async_trait]` macro for dyn-compatible async traits | Already used for EmbeddingEngine and SummarizationEngine; locked by D-03 |
| tokio-rusqlite | 0.7 | `Arc<Connection>` for SqliteBackend and compact_runs audit | Already used; all db.call() patterns stay |
| rusqlite | 0.37 | Synchronous SQLite operations inside tokio-rusqlite closures | Already used; all SQL stays the same |

### No New Dependencies

This phase adds zero new Cargo dependencies. All required crates are already declared. The `src/storage/` module tree is pure Rust code reorganization.

---

## Architecture Patterns

### Recommended Module Structure

```
src/
├── storage/
│   ├── mod.rs        # StorageBackend trait + shared input/output types
│   └── sqlite.rs     # SqliteBackend struct + #[async_trait] impl
├── service.rs        # MemoryService: holds Arc<dyn StorageBackend> + Arc<dyn EmbeddingEngine>
├── compaction.rs     # CompactionService: holds Arc<dyn StorageBackend> + Arc<Connection> (audit)
├── db.rs             # Unchanged — schema init, register_sqlite_vec
├── embedding.rs      # Unchanged
├── auth.rs           # Unchanged — KeyService keeps its own Arc<Connection>
└── lib.rs            # Add: pub mod storage;
```

### Pattern 1: EmbeddingEngine is the Template

The existing `EmbeddingEngine` trait is the exact model to replicate for `StorageBackend`:

```rust
// src/embedding.rs — existing pattern (HIGH confidence: read from source)
use async_trait::async_trait;

#[async_trait]
pub trait EmbeddingEngine: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}
```

`StorageBackend` follows the same structure:

```rust
// src/storage/mod.rs — new file following EmbeddingEngine pattern
use async_trait::async_trait;
use crate::error::ApiError;
use crate::service::{Memory, ListResponse, SearchResponse, ListParams, SearchParams};

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn store(&self, req: StoreRequest) -> Result<Memory, ApiError>;
    async fn get_by_id(&self, id: &str) -> Result<Option<Memory>, ApiError>;
    async fn list(&self, params: ListParams) -> Result<ListResponse, ApiError>;
    async fn search(&self, embedding: Vec<f32>, params: SearchParams) -> Result<SearchResponse, ApiError>;
    async fn delete(&self, id: &str) -> Result<Memory, ApiError>;
    // Compaction sub-operations
    async fn fetch_candidates(&self, agent_id: &str, max_candidates: u32) -> Result<(Vec<CandidateRecord>, bool), ApiError>;
    async fn insert_merged_memory(&self, req: MergedMemoryRequest) -> Result<Memory, ApiError>;
    async fn delete_memories(&self, ids: &[String]) -> Result<(), ApiError>;
}
```

**Note on error type:** Reuse `ApiError` for trait methods — it is already the return type for all MemoryService and CompactionService methods, and it converts from `tokio_rusqlite::Error` via the existing `From` impl in `src/error.rs`. Introducing a new `StorageError` is Claude's discretion territory but adds unnecessary complexity for a pure refactor.

### Pattern 2: SqliteBackend Wraps Arc<Connection>

```rust
// src/storage/sqlite.rs
use std::sync::Arc;
use tokio_rusqlite::Connection;
use async_trait::async_trait;
use crate::storage::StorageBackend;

pub struct SqliteBackend {
    db: Arc<Connection>,
}

impl SqliteBackend {
    pub fn new(db: Arc<Connection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StorageBackend for SqliteBackend {
    async fn store(&self, req: StoreRequest) -> Result<Memory, ApiError> {
        // Move exact code from MemoryService::create_memory's db.call() block here
        // The dual-table atomic transaction (memories + vec_memories) is preserved verbatim
        self.db.call(move |c| {
            let tx = c.transaction()?;
            // ... exact SQL unchanged ...
            tx.commit()?;
            Ok(created_at)
        }).await?;
        // ...
    }
    // ... other methods
}
```

### Pattern 3: MemoryService Constructor Change

```rust
// src/service.rs — AFTER refactor
pub struct MemoryService {
    pub backend: Arc<dyn StorageBackend>,       // was: pub db: Arc<Connection>
    pub embedding: Arc<dyn EmbeddingEngine>,
    pub embedding_model: String,
}

impl MemoryService {
    pub fn new(
        backend: Arc<dyn StorageBackend>,        // was: db: Arc<Connection>
        embedding: Arc<dyn EmbeddingEngine>,
        embedding_model: String,
    ) -> Self {
        Self { backend, embedding, embedding_model }
    }

    pub async fn create_memory(&self, req: CreateMemoryRequest) -> Result<Memory, ApiError> {
        // Validate + embed (unchanged)
        let embedding = self.embedding.embed(&req.content).await?;
        // Delegate to backend (replaces inline db.call block)
        self.backend.store(StoreRequest { id, content, agent_id, ..., embedding }).await
    }
}
```

### Pattern 4: CompactionService Dual-Connection Design (D-07/D-08)

```rust
// src/compaction.rs — AFTER refactor
pub struct CompactionService {
    backend: Arc<dyn StorageBackend>,     // NEW: for memory fetch/insert/delete
    audit_db: Arc<Connection>,            // KEPT: for compact_runs INSERT/UPDATE only
    embedding: Arc<dyn EmbeddingEngine>,
    summarization: Option<Arc<dyn SummarizationEngine>>,
    embedding_model: String,
}

impl CompactionService {
    pub fn new(
        backend: Arc<dyn StorageBackend>,
        audit_db: Arc<Connection>,        // caller passes db_arc.clone() for audit
        embedding: Arc<dyn EmbeddingEngine>,
        summarization: Option<Arc<dyn SummarizationEngine>>,
        embedding_model: String,
    ) -> Self { ... }
}
```

In `main.rs`, the factory becomes:

```rust
let db_arc = Arc::new(conn);
let backend: Arc<dyn StorageBackend> = Arc::new(SqliteBackend::new(db_arc.clone()));
let key_service = Arc::new(auth::KeyService::new(db_arc.clone()));
let service = Arc::new(MemoryService::new(backend.clone(), embedding.clone(), embedding_model.clone()));
let compaction = Arc::new(CompactionService::new(
    backend.clone(),
    db_arc.clone(),   // audit_db — same connection is fine for SQLite
    embedding.clone(),
    llm_engine,
    embedding_model.clone(),
));
```

### Pattern 5: MockStorageBackend for Tests

The existing `MockEmbeddingEngine` in `tests/integration.rs` shows the test mock pattern:

```rust
// tests/integration.rs — existing MockEmbeddingEngine pattern
struct MockEmbeddingEngine;

#[async_trait::async_trait]
impl mnemonic::embedding::EmbeddingEngine for MockEmbeddingEngine {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, mnemonic::error::EmbeddingError> { ... }
}
```

For unit tests of future backends, a `MockStorageBackend` can follow this exact pattern. However, for Phase 21, integration tests use a real `SqliteBackend` (in-memory DB), so no mock is required yet.

### Anti-Patterns to Avoid

- **Putting compact_runs SQL in the StorageBackend trait:** D-07 locks this out. The trait must only expose memory operations.
- **Moving embedding generation into StorageBackend:** D-09 locks this out. The trait takes `Vec<f32>`, never `&str`.
- **Introducing StorageError wrapping ApiError:** Adds an extra conversion layer with no benefit for a refactor that must keep all tests passing. Reuse ApiError directly.
- **Changing CandidateMemory from a private struct to a pub type:** It is currently private to compaction.rs. Extracting it to `src/storage/mod.rs` may break the module boundary. Consider a separate `CandidateRecord` type in `src/storage/mod.rs` OR keep fetch_candidates returning the existing private struct by making it pub — the planner should decide.
- **Touching auth.rs or KeyService:** D-AUTH locks KeyService on its direct `Arc<Connection>`. No changes to auth module.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async trait object safety | Manual wrapper enum or Box<dyn Fn> | `#[async_trait]` (already in Cargo.toml) | Native async fn in traits is not dyn-compatible in Rust as of early 2026; async_trait macro generates the necessary boxing |
| Error conversion from rusqlite | Custom error mapping in every trait method | `From<tokio_rusqlite::Error> for ApiError` already in error.rs | The impl already exists at line 140 of error.rs |
| Send + Sync bounds management | Manual PhantomData or unsafe impl | Add `Send + Sync` to trait definition like EmbeddingEngine | Arc<dyn StorageBackend> requires Send + Sync on the trait |

**Key insight:** This is a pure reorganization, not a new capability. Every problem in this phase is already solved by the existing codebase patterns.

---

## Common Pitfalls

### Pitfall 1: CandidateMemory Visibility

**What goes wrong:** `CandidateMemory` struct in `compaction.rs` is currently private. When `fetch_candidates` moves to `SqliteBackend`, the return type must cross module boundaries. If the struct stays private to compaction.rs, the trait cannot reference it.

**Why it happens:** The struct was never designed for public use — it is an internal implementation detail of compaction.

**How to avoid:** Define a public `CandidateRecord` struct in `src/storage/mod.rs` (or reuse `Memory` with an extra `embedding` field). The planner should explicitly specify which public type is used in the `fetch_candidates` return signature.

**Warning signs:** Compiler error "type `CandidateMemory` is private" when compiling `src/storage/sqlite.rs`.

### Pitfall 2: Test Constructor Signature Mismatch

**What goes wrong:** `tests/integration.rs` lines 616-623 directly construct `MemoryService::new(db.clone(), embedding.clone(), ...)` and `CompactionService::new(db.clone(), embedding.clone(), ...)`. After the refactor, these constructors require `Arc<dyn StorageBackend>` as the first argument instead of `Arc<Connection>`.

**Why it happens:** Integration tests construct services directly to set up test state before routing requests through the axum router.

**How to avoid:** Update `build_test_state()` helper (and similar helpers `build_test_compact_state()`, `build_auth_app()`, `build_scoped_auth_app()`) to construct `SqliteBackend` first, then pass `Arc::new(SqliteBackend::new(db.clone()))` to each service constructor. This change is localized to the helper functions — individual test bodies need no changes.

**Warning signs:** Compilation error "expected `Arc<dyn StorageBackend>`, found `Arc<Connection>`" in test helpers.

### Pitfall 3: Dual-Connection Confusion for CompactionService

**What goes wrong:** CompactionService currently uses `self.db` for both memory operations AND compact_runs audit. After the refactor, memory operations route through `self.backend` but compact_runs must still go through a direct `Arc<Connection>`. If the developer forgets to keep the `Arc<Connection>` for audit, compact_runs INSERT/UPDATE calls break.

**Why it happens:** D-07/D-08 introduce a two-field design that was not present before.

**How to avoid:** Name the fields clearly: `backend: Arc<dyn StorageBackend>` (memory ops) and `audit_db: Arc<Connection>` (compact_runs only). Add a comment in the struct definition referencing D-07/D-08.

**Warning signs:** `compact_runs` table writes fail silently, or the `audit_db` field is accidentally removed.

### Pitfall 4: Atomic Transaction Boundary Moves

**What goes wrong:** `MemoryService::create_memory` currently performs a single atomic `db.call(|c| { tx = c.transaction(); ... tx.commit() })`. If this gets split across two methods (one for embed, one for store), the transaction no longer wraps both inserts.

**Why it happens:** Refactoring the inline `db.call` block into a trait method call looks like a simple extraction but must preserve the single-closure transaction.

**How to avoid:** The `store()` trait method on SqliteBackend must perform the complete dual-table transaction (INSERT INTO memories + INSERT INTO vec_memories + SELECT created_at, all inside a single `db.call` closure). The embedding vector is passed in as `Vec<f32>` — it is pre-computed before calling `backend.store()`.

**Warning signs:** Test `test_post_memory` passes but `test_schema_created` reveals vec_memories and memories can get out of sync.

### Pitfall 5: `get_memory_agent_id` Must Move to Trait

**What goes wrong:** `MemoryService::get_memory_agent_id()` is called by the `delete_memory_handler` in server.rs for scope enforcement. It currently queries SQLite directly. If it is not exposed through the `StorageBackend` trait, MemoryService cannot implement it without keeping a direct `Arc<Connection>`.

**Why it happens:** It is a lightweight fetch that doesn't fit neatly into "CRUD" operations and can be overlooked.

**How to avoid:** Include `get_by_id` or a dedicated `get_agent_id` method in the trait surface (D-01 lists get_by_id). `get_memory_agent_id` can be implemented in MemoryService by calling `self.backend.get_by_id(id).await?.map(|m| m.agent_id)` — no new trait method needed if `get_by_id` returns a full `Memory`.

### Pitfall 6: `pub mod storage` in Both lib.rs and main.rs

**What goes wrong:** `src/lib.rs` has module declarations. `src/main.rs` also has its own set of `mod` declarations (not `use mnemonic::...`). Both files must declare `mod storage;` independently.

**Why it happens:** The binary crate (`main.rs`) does not use the library crate — it re-declares all modules. This is the existing pattern for the entire codebase.

**How to avoid:** Add `mod storage;` to `src/main.rs` (before `mod service;`) and `pub mod storage;` to `src/lib.rs`.

**Warning signs:** Works when running `cargo test` (uses lib crate) but binary fails to compile with "unresolved module `storage`".

---

## Code Examples

Verified patterns from source code:

### EmbeddingEngine Trait (Direct Template)

```rust
// Source: src/embedding.rs lines 1-11 (read from filesystem)
use async_trait::async_trait;
use crate::error::EmbeddingError;

#[async_trait]
pub trait EmbeddingEngine: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}
```

### Existing tokio-rusqlite Pattern (Must Be Preserved in SqliteBackend)

```rust
// Source: src/service.rs lines 115-133 (read from filesystem)
// This entire db.call block moves verbatim into SqliteBackend::store()
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
```

### Existing Error Conversions (Already Available — Don't Re-implement)

```rust
// Source: src/error.rs lines 30-34, 140-144 (read from filesystem)
impl From<tokio_rusqlite::Error> for DbError {
    fn from(e: tokio_rusqlite::Error) -> Self {
        DbError::Query(format!("{}", e))
    }
}

impl From<tokio_rusqlite::Error> for ApiError {
    fn from(e: tokio_rusqlite::Error) -> Self {
        ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string())))
    }
}
```

### Test Helper Constructor (Must Be Updated)

```rust
// Source: tests/integration.rs lines 610-631 (read from filesystem)
// BEFORE refactor:
async fn build_test_state() -> (AppState, Arc<MemoryService>) {
    let conn = mnemonic::db::open(&config).await.unwrap();
    let db = Arc::new(conn);
    let service = Arc::new(MemoryService::new(db.clone(), embedding.clone(), "mock-model".to_string()));
    let compaction = Arc::new(CompactionService::new(db.clone(), embedding.clone(), None, "mock-model".to_string()));
    // ...
}

// AFTER refactor (what the planner must produce):
async fn build_test_state() -> (AppState, Arc<MemoryService>) {
    let conn = mnemonic::db::open(&config).await.unwrap();
    let db = Arc::new(conn);
    let backend: Arc<dyn StorageBackend> = Arc::new(SqliteBackend::new(db.clone()));
    let service = Arc::new(MemoryService::new(backend.clone(), embedding.clone(), "mock-model".to_string()));
    let compaction = Arc::new(CompactionService::new(backend.clone(), db.clone(), embedding.clone(), None, "mock-model".to_string()));
    // ...
}
```

---

## Actual Test Count (Verified)

The success criterion "239 tests passing" is the actual current baseline:

| Test Suite | Binary/Lib | Count | Notes |
|------------|-----------|-------|-------|
| Unit tests (lib target) | mnemonic lib | 63 | auth, compaction, embedding, server, error |
| Unit tests (bin target) | mnemonic bin | 63 | Same tests, run again for binary crate |
| CLI integration | cli_integration | 55 | CLI command tests |
| Error types | error_types | 4 | Error variant tests |
| Integration | integration | 54 passing + 1 ignored | HTTP API + embedding tests; test_openai_embedding is #[ignore] |
| **Total passing** | | **239** | Matches STOR-05 requirement |

The 239 is confirmed: `cargo test` produces 63+63+55+4+54 = 239 passing, 1 ignored.

---

## Shared Types for the Trait

These input types must be defined in `src/storage/mod.rs` (Claude's discretion per CONTEXT.md):

| Type | Fields | Used By |
|------|--------|---------|
| `StoreRequest` | id, content, agent_id, session_id, tags, embedding_model, embedding: Vec<f32> | `store()` |
| `CandidateRecord` | id, content, tags, created_at, embedding: Vec<f32> | `fetch_candidates()` return type |
| `MergedMemoryRequest` | new_id, agent_id, content, tags, embedding_model, created_at, source_ids, embedding: Vec<f32> | `insert_merged_memory()` |

`Memory`, `ListResponse`, `SearchResponse`, `ListParams`, `SearchParams` are already defined in `service.rs` — the trait can reference them there, or they can be moved to `storage/mod.rs` and re-exported. Given zero-behavior-change goal, leaving them in `service.rs` and referencing them from the trait is lower risk (fewer file changes).

**Recommendation:** Keep `Memory`, `ListParams`, etc. in `service.rs`. Import them from `crate::service` in `storage/mod.rs`. This minimizes the number of files changed and reduces test breakage risk.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `async fn` in traits (not dyn-compatible) | `#[async_trait]` with boxing | Rust 2024 / late 2023 | Native async fn in traits exists in nightly but is not dyn-compatible without `dyn*` — use async_trait macro for stable dyn dispatch |

**Deprecated/outdated:**
- Native async fn in trait definitions for dyn-dispatch: Not usable for `Arc<dyn Trait>` without nightly features as of early 2026. Decision D-03 locks this.

---

## Open Questions

1. **CandidateRecord vs CandidateMemory naming**
   - What we know: `CandidateMemory` is private in compaction.rs; the trait needs a public type
   - What's unclear: Whether to rename it `CandidateRecord` in storage/mod.rs or just `pub struct CandidateMemory` there
   - Recommendation: Define `pub struct CandidateRecord` in `src/storage/mod.rs` with the same fields; update CompactionService to use it internally

2. **CompactionService constructor arity change**
   - What we know: The test helper `build_test_compact_state()` (if it exists) passes `(db, embedding, None, "mock-model")`. After refactor it needs `(backend, audit_db, embedding, None, "mock-model")`.
   - What's unclear: Whether any CLI init functions in `src/cli.rs` (e.g., `init_compaction()`) also construct CompactionService and need updating
   - Recommendation: Search for all `CompactionService::new(` call sites before writing the plan — there are at least 2 (main.rs and cli.rs)

3. **`delete_source_memories` as one call or many**
   - What we know: Compaction deletes N source IDs atomically in one transaction. The trait could be `delete_memories(&[String])` (batch) or `delete(&str)` (single) called N times.
   - What's unclear: Whether SqliteBackend should expose batch delete or the atomicity must be at the `insert_merged_memory` level
   - Recommendation: Define a single `write_compaction_result(merged: MergedMemoryRequest, source_ids: &[String])` method that does the complete atomic insert+delete in one transaction inside SqliteBackend. This preserves atomicity at the backend layer.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | None — standard Cargo test runner |
| Quick run command | `cargo test --lib -- --test-output immediate 2>&1 \| tail -5` |
| Full suite command | `cargo test 2>&1 \| grep "test result:"` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| STOR-01 | StorageBackend trait compiles with all methods | unit (compile) | `cargo build` | Wave 0 (new file) |
| STOR-01 | StorageBackend is dyn-compatible (Arc<dyn StorageBackend> works) | unit | `cargo test --lib tests::test_storage_trait_object_safe` | Wave 0 |
| STOR-02 | SqliteBackend::store creates memory, searchable via search | integration | `cargo test test_post_memory` | Existing (after constructor update) |
| STOR-02 | SqliteBackend preserves atomic dual-table write | integration | `cargo test test_compact_atomic_write` | Existing (after constructor update) |
| STOR-03 | MemoryService holds Arc<dyn StorageBackend> — existing create/search/list/delete tests pass | integration | `cargo test -- test_post_memory test_search_memories test_list_memories test_delete_memory` | Existing |
| STOR-04 | CompactionService compact_runs audit still writes correctly | integration | `cargo test test_compact_http_basic` | Existing |
| STOR-04 | CompactionService memory fetch/insert/delete goes through backend | integration | `cargo test test_compact_atomic_write test_compact_dry_run` | Existing |
| STOR-05 | All 239 tests pass | all | `cargo test 2>&1 \| grep "test result:"` must show 239 passing | Existing |

### Sampling Rate

- **Per plan step commit:** `cargo build` (compile check, no test run needed)
- **Per wave merge:** `cargo test --lib` + `cargo test --test integration` (unit + core integration)
- **Phase gate:** `cargo test` full suite — 239 passing before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `src/storage/mod.rs` — new file defining StorageBackend trait
- [ ] `src/storage/sqlite.rs` — new file with SqliteBackend implementation
- No test framework gaps — cargo test is the standard runner, already configured

---

## Sources

### Primary (HIGH confidence)

- `src/embedding.rs` — EmbeddingEngine trait pattern read directly from source (direct template for StorageBackend)
- `src/service.rs` — Full MemoryService implementation read from source (all SQL to move into SqliteBackend)
- `src/compaction.rs` — Full CompactionService implementation read from source (audit split design, fetch_candidates SQL)
- `src/error.rs` — Error types and From impls read from source (reuse ApiError, no new StorageError needed)
- `src/server.rs` — AppState struct read from source (wiring change required)
- `src/main.rs` — Service construction read from source (factory change required)
- `src/lib.rs`, `src/main.rs` — Module declarations read (both need `mod storage;`)
- `tests/integration.rs` — Test helpers read from source (constructor update sites identified)
- `Cargo.toml` — Dependency list verified (no new crates needed)
- `cargo test` run — 239 passing tests confirmed as baseline

### Secondary (MEDIUM confidence)

- async-trait 0.1 crate behavior: confirmed by existing usage in EmbeddingEngine and SummarizationEngine — `#[async_trait]` + `dyn T` works in this codebase

### Tertiary (LOW confidence)

- None

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies verified from Cargo.toml; no new crates needed
- Architecture: HIGH — EmbeddingEngine and SummarizationEngine patterns verified from source; all construction sites read
- Pitfalls: HIGH — identified directly from reading source code, not speculation
- Test count: HIGH — verified by running `cargo test` (239 passing, 1 ignored)

**Research date:** 2026-03-21
**Valid until:** No external dependencies change — this is pure source-code refactoring; research is stable indefinitely
