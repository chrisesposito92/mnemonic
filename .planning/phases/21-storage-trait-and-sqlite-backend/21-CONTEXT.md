# Phase 21: Storage Trait and SQLite Backend - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Define a `StorageBackend` async trait that abstracts all memory storage operations, implement `SqliteBackend` by wrapping existing SQLite+sqlite-vec code with zero behavior change, and refactor `MemoryService` and `CompactionService` to hold `Arc<dyn StorageBackend>` instead of direct `tokio-rusqlite` connections. All 239 existing tests must pass unchanged after the refactor. KeyService remains on its direct `Arc<Connection>` — it is not part of this refactor.

</domain>

<decisions>
## Implementation Decisions

### Trait method surface
- **D-01:** StorageBackend exposes one async method per logical storage operation, mirroring the current MemoryService/CompactionService method boundaries: store, get_by_id, list, search, delete, plus compaction sub-operations (fetch_candidates, insert_merged_memory, delete_source_memories)
- **D-02:** Search results use lower-is-better distance semantics — this is the trait contract. SqliteBackend passes through sqlite-vec distances directly. Future backends (Qdrant) must convert scores to distances via `1.0 - score`
- **D-03:** The trait uses `#[async_trait]` — native async fn in traits is not dyn-compatible as of early 2026

### Module organization
- **D-04:** New `src/storage/` module tree: `mod.rs` defines the `StorageBackend` trait and shared types, `sqlite.rs` contains `SqliteBackend` implementation
- **D-05:** `src/storage/mod.rs` re-exports `StorageBackend` trait and `SqliteBackend` so existing code can `use crate::storage::{StorageBackend, SqliteBackend}`
- **D-06:** Future backends (Qdrant, Postgres) will each get their own file in `src/storage/` behind feature gates — this phase only creates the structure and sqlite.rs

### Compact audit log placement
- **D-07:** `compact_runs` table is NOT part of the StorageBackend trait — it remains a separate concern handled by CompactionService directly via its own SQLite connection (or companion SQLite file for non-SQLite backends in future phases)
- **D-08:** CompactionService keeps a dedicated `Arc<Connection>` for compact_runs audit logging, separate from the `Arc<dyn StorageBackend>` it uses for memory operations. This avoids requiring every backend to implement SQL-specific audit tables

### Embedding handling in trait
- **D-09:** StorageBackend methods that need embeddings (store, search) accept pre-computed `Vec<f32>` — the trait does not handle embedding generation. MemoryService continues to own the `Arc<dyn EmbeddingEngine>` and calls it before passing vectors to the backend

### Claude's Discretion
- Exact method signatures and error types for StorageBackend trait methods
- How to handle the dual-table insert pattern (memories + vec_memories) inside SqliteBackend
- Whether to introduce a StorageError type or reuse existing ApiError/DbError
- Test refactoring approach — how to adapt test helpers while keeping all 239 tests green
- Naming conventions for trait methods (e.g., `store_memory` vs `store` vs `create`)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Storage trait design
- `.planning/REQUIREMENTS.md` — STOR-01 through STOR-05 define the trait requirements and test pass criteria
- `.planning/ROADMAP.md` Phase 21 section — success criteria including "new StorageBackend implementor can be added without modifying MemoryService or CompactionService"

### Existing trait patterns to follow
- `src/embedding.rs` lines 1-11 — EmbeddingEngine async trait pattern (the model for StorageBackend's trait design)
- `src/summarization.rs` — SummarizationEngine trait pattern (same async_trait + Arc<dyn ...> approach)

### Current storage code to wrap
- `src/service.rs` — MemoryService with all memory CRUD and search operations (the code being abstracted)
- `src/compaction.rs` — CompactionService with fetch_candidates, clustering, and atomic writes (compaction operations to abstract)
- `src/db.rs` — Schema definition, sqlite-vec registration, and database opening (stays as SQLite-specific infrastructure)
- `src/server.rs` lines 34-39 — AppState struct that wires services together (must be updated to use StorageBackend)

### Auth boundary (do NOT touch)
- `src/auth.rs` — KeyService with direct Arc<Connection> (explicitly excluded from this refactor per STATE.md decision)

### Prior decisions
- `.planning/STATE.md` "Accumulated Context > Decisions" section — async_trait requirement, KeyService exclusion, distance contract, feature gate strategy

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `EmbeddingEngine` trait pattern (`src/embedding.rs`): Exact template for StorageBackend — `#[async_trait]`, `Send + Sync` bounds, used as `Arc<dyn EmbeddingEngine>`
- `SummarizationEngine` trait pattern (`src/summarization.rs`): Same pattern, confirms the codebase convention
- `MockEmbeddingEngine` (`src/embedding.rs`): Pattern for creating `MockStorageBackend` for tests

### Established Patterns
- **Trait objects via Arc<dyn T>**: Both EmbeddingEngine and SummarizationEngine use this pattern — StorageBackend will follow suit
- **tokio-rusqlite `conn.call()` for all SQLite access**: All DB operations use `self.db.call(move |c| { ... })` closures — this pattern moves entirely into SqliteBackend
- **Atomic dual-table writes**: Memory create/delete operate on both `memories` and `vec_memories` tables in a single transaction — SqliteBackend must preserve this
- **CTE over-fetch for filtered KNN**: Search uses 10x over-fetch with post-filter — this is SQLite-specific logic that belongs in SqliteBackend, not the trait

### Integration Points
- `AppState` in `src/server.rs`: Currently holds `Arc<MemoryService>` — MemoryService constructor will change to accept `Arc<dyn StorageBackend>` instead of `Arc<Connection>`
- `main.rs` init functions: `init_db_and_embedding()` and `init_compaction()` create services — these become the factory point for backend selection
- `src/lib.rs`: Module declarations — must add `pub mod storage;`
- Test files: Tests that construct MemoryService/CompactionService directly will need updated constructors

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. The existing EmbeddingEngine trait pattern is the clear template to follow for StorageBackend design.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 21-storage-trait-and-sqlite-backend*
*Context gathered: 2026-03-21*
