# Phase 24: Postgres Backend - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement `PostgresBackend` behind the `backend-postgres` feature flag, satisfying all 7 `StorageBackend` trait methods using `sqlx` + `pgvector`. Wire it into the `create_backend()` factory (replacing the current `todo!()` stub). All memory CRUD, semantic search, and compaction operations must work against a Postgres+pgvector instance. Multi-agent namespace isolation via SQL WHERE clauses on `agent_id`. Compaction is fully atomic using Postgres transactions (insert merged + delete sources in one transaction). The default binary (without `--features backend-postgres`) must remain unchanged — zero new dependencies.

</domain>

<decisions>
## Implementation Decisions

### Postgres table schema
- **D-01:** Single table `memories` — all agents share one table, isolated by SQL WHERE on `agent_id` (per PGVR-04)
- **D-02:** Column definitions:
  - `id TEXT PRIMARY KEY` (our UUID v7 strings)
  - `content TEXT NOT NULL`
  - `agent_id TEXT NOT NULL`
  - `session_id TEXT NOT NULL`
  - `tags TEXT[] NOT NULL DEFAULT '{}'` (Postgres native array — enables `@>` containment queries for tag filtering)
  - `embedding_model TEXT NOT NULL`
  - `embedding vector(384) NOT NULL` (pgvector type, matching all-MiniLM-L6-v2 bundled model)
  - `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
  - `updated_at TIMESTAMPTZ`
- **D-03:** Indexes created at startup:
  - `CREATE INDEX IF NOT EXISTS idx_memories_agent_id ON memories(agent_id)` — for namespace filtering
  - `CREATE INDEX IF NOT EXISTS idx_memories_session_id ON memories(session_id)` — for session filtering
  - `CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at)` — for date range queries and ORDER BY
  - HNSW index on embedding column: `CREATE INDEX IF NOT EXISTS idx_memories_embedding ON memories USING hnsw (embedding vector_cosine_ops)` — for efficient approximate nearest neighbor search
- **D-04:** Schema auto-creation: `PostgresBackend::new()` runs `CREATE EXTENSION IF NOT EXISTS vector` then `CREATE TABLE IF NOT EXISTS memories (...)` then all indexes. Idempotent — safe on every startup

### Rust crate choices
- **D-05:** `sqlx` with `postgres` and `runtime-tokio` features — async-native, connection pooling built-in, matches the tokio runtime used throughout the project
- **D-06:** `pgvector` crate for the `Vector` type — provides sqlx `Type`/`Encode`/`Decode` impls for `vector(N)` columns, no manual byte serialization needed
- **D-07:** Both are optional dependencies gated behind `backend-postgres` feature in Cargo.toml

### PostgresBackend construction
- **D-08:** `PostgresBackend::new(config: &Config) -> Result<Self, ApiError>` reads `config.postgres_url`, creates a `PgPool` via `PgPoolOptions::new().connect(url)`, runs schema auto-creation, stores the pool in the struct
- **D-09:** `PgPool` is `Send + Sync` and handles connection pooling internally — store directly in the struct, no Arc wrapping needed (same pattern as QdrantBackend's client)
- **D-10:** Connection pool uses sqlx defaults (10 connections) — sufficient for Mnemonic's workload. No custom pool config needed

### Distance semantics
- **D-11:** pgvector cosine distance operator `<=>` returns distance directly (lower = more similar). This matches the StorageBackend trait contract out of the box — no score conversion needed (unlike Qdrant's `1.0 - score`)
- **D-12:** Search query: `SELECT *, embedding <=> $1::vector AS distance FROM memories WHERE agent_id = $2 ... ORDER BY distance ASC LIMIT $3`

### Compaction atomicity (PGVR-03)
- **D-13:** `write_compaction_result()` uses a Postgres transaction: `BEGIN` → `INSERT INTO memories` (merged memory) → `DELETE FROM memories WHERE id = ANY($1)` (source IDs) → `COMMIT`. If any step fails, the entire transaction rolls back — full atomicity, the key advantage over Qdrant
- **D-14:** This satisfies success criterion 4: "if the process is interrupted mid-compact, the database is left in a consistent state with no partial writes"

### List and pagination
- **D-15:** `list()` uses standard SQL `ORDER BY created_at DESC LIMIT $1 OFFSET $2` — native SQL pagination, no client-side workaround needed (unlike Qdrant's scroll API)
- **D-16:** Filters applied via SQL WHERE clauses: `agent_id = $1`, `session_id = $2`, `tags @> ARRAY[$3]::text[]` (containment), `created_at >= $4`, `created_at <= $5`
- **D-17:** Total count: `SELECT COUNT(*) FROM memories WHERE ...` with same filters — SQL native, same as SqliteBackend approach

### Search implementation
- **D-18:** Uses pgvector's `<=>` operator with SQL WHERE filtering — Postgres applies filters and vector search together, no over-fetch needed (like Qdrant, unlike SQLite's CTE 10x over-fetch)
- **D-19:** `threshold` filtering: applied in SQL via `WHERE embedding <=> $1::vector <= $2` — pushed to the database, not post-filtered

### fetch_candidates implementation
- **D-20:** `SELECT id, content, tags, created_at, embedding FROM memories WHERE agent_id = $1 ORDER BY created_at DESC LIMIT $2`
- **D-21:** Limit to `max_candidates + 1` (same over-fetch-by-one pattern as SqliteBackend and QdrantBackend) to detect truncation

### Tag handling
- **D-22:** Tags stored as `TEXT[]` (Postgres native array) — enables `@>` containment operator for filtering without JSON parsing
- **D-23:** Store: convert `Vec<String>` to `&[String]` for sqlx binding. Retrieve: sqlx decodes `TEXT[]` back to `Vec<String>` automatically

### Module structure and feature gating
- **D-24:** New file `src/storage/postgres.rs` containing `PostgresBackend` struct and `StorageBackend` impl, behind `#[cfg(feature = "backend-postgres")]`
- **D-25:** `src/storage/mod.rs` conditionally declares `pub mod postgres;` behind `#[cfg(feature = "backend-postgres")]` and conditionally re-exports `PostgresBackend`
- **D-26:** `Cargo.toml` feature `backend-postgres` gains dependencies: `sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"], optional = true }` and `pgvector = { version = "0.4", features = ["sqlx"], optional = true }` — only pulled when feature is enabled
- **D-27:** Wire `PostgresBackend::new(config).await?` into the `create_backend()` factory's `"postgres"` arm, replacing the `todo!()`

### Testing strategy
- **D-28:** Unit tests behind `#[cfg(test)]` in `postgres.rs` test individual helper functions (tag conversion, filter building, query construction) without needing a live Postgres instance
- **D-29:** Integration tests behind `#[cfg(all(test, feature = "backend-postgres"))]` require a running Postgres+pgvector instance. NOT run in default `cargo test`. Document how to run: `docker run -e POSTGRES_PASSWORD=test -p 5432:5432 pgvector/pgvector:pg17` + `MNEMONIC_POSTGRES_URL=postgres://postgres:test@localhost/mnemonic cargo test --features backend-postgres`
- **D-30:** The existing 273+ tests must still pass unchanged when built without `--features backend-postgres`

### Claude's Discretion
- Exact sqlx and pgvector crate versions (latest stable)
- Internal helper function decomposition within PostgresBackend
- Error message wording for Postgres connection failures
- PgPool configuration (max connections, timeouts)
- Whether to add database creation logic or require the database to exist
- Exact HNSW index parameters (m, ef_construction)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — PGVR-01 through PGVR-04 define the Postgres backend requirements
- `.planning/ROADMAP.md` Phase 24 section — success criteria and goal

### StorageBackend contract
- `src/storage/mod.rs` — StorageBackend trait definition, all 7 method signatures, distance semantics contract (lower-is-better), shared types (StoreRequest, CandidateRecord, MergedMemoryRequest)
- `src/storage/sqlite.rs` — Reference implementation: SqliteBackend shows exact method behavior, return types, error handling, and edge cases (NotFound, pagination, threshold filtering)

### Sibling backend (pattern reference)
- `src/storage/qdrant.rs` — QdrantBackend implementation: struct layout, `new()` construction, trait impl pattern, error mapping, helper functions. The Postgres backend follows the same structural pattern

### Factory wiring point
- `src/storage/mod.rs` lines 128-139 — `create_backend()` "postgres" arm with `todo!()` stub to replace
- `Cargo.toml` line 17 — `backend-postgres = []` feature declaration to add sqlx/pgvector dependencies

### Config fields (from Phase 22)
- `src/config.rs` — `postgres_url: Option<String>` field; `validate_config()` already checks postgres_url is present when storage_provider is "postgres"

### Error types
- `src/error.rs` — ApiError, MnemonicError, ConfigError, DbError types used by StorageBackend returns

### Prior decisions
- `.planning/phases/21-storage-trait-and-sqlite-backend/21-CONTEXT.md` — D-02: distance contract, D-06: feature gate strategy, D-07/D-08: compact_runs audit stays in separate SQLite
- `.planning/phases/22-config-extension-backend-factory-and-config-cli/22-CONTEXT.md` — D-12: feature gate error at create_backend time, D-15: factory accepts sqlite_conn but non-sqlite backends ignore it
- `.planning/phases/23-qdrant-backend/23-CONTEXT.md` — Structural template: collection/table auto-creation, point ID mapping, testing strategy, module structure
- `.planning/STATE.md` "Accumulated Context > Decisions" — all cross-phase decisions

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `QdrantBackend` in `src/storage/qdrant.rs`: Structural template — same new(), same trait impl, same error mapping pattern. Postgres version is simpler (SQL vs gRPC)
- `SqliteBackend` in `src/storage/sqlite.rs`: Reference implementation for all 7 trait methods — PostgresBackend must produce identical output types and semantics
- `create_backend()` in `src/storage/mod.rs`: Factory function with `todo!()` stub ready for PostgresBackend wiring
- `Config` field in `src/config.rs`: `postgres_url` already validated by `validate_config()`
- Feature flag `backend-postgres` in `Cargo.toml`: Already declared as empty feature — needs sqlx/pgvector dependencies added

### Established Patterns
- **#[async_trait] on StorageBackend**: PostgresBackend must use the same pattern
- **Arc<dyn StorageBackend>**: PostgresBackend is used as `Arc::new(PostgresBackend::new(...).await?)`
- **ApiError returns**: All trait methods return `Result<T, ApiError>` — map sqlx errors to ApiError::Internal
- **Score semantics**: Trait contract is lower-is-better distance; pgvector `<=>` returns distance directly — no conversion needed
- **Per-cluster compaction**: `write_compaction_result()` called once per cluster — Postgres wraps each call in a transaction for full atomicity

### Integration Points
- `src/storage/mod.rs`: Add `#[cfg(feature = "backend-postgres")] pub mod postgres;` and conditional re-export
- `src/storage/mod.rs` create_backend(): Replace `todo!()` with `PostgresBackend::new(config).await`
- `Cargo.toml`: Add `sqlx` and `pgvector` as optional dependencies under `backend-postgres` feature
- `src/storage/postgres.rs`: New file — entire PostgresBackend implementation

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. The SqliteBackend in sqlite.rs is the definitive reference for expected behavior. The QdrantBackend in qdrant.rs is the structural template for a second backend implementation. Postgres-specific advantages (real SQL, real transactions, native array types) should be leveraged rather than working around them.

</specifics>

<deferred>
## Deferred Ideas

- Connection string with SSL/TLS options — document but don't add custom config fields beyond postgres_url
- Read replicas for search queries — adds connection routing complexity, not needed for v1.4
- Postgres connection pool tuning (max_connections, idle_timeout) as config fields — sqlx defaults are sufficient
- IVFFLAT vs HNSW index selection as a config option — HNSW is the better default, can add selection later
- Vector dimension as a config field (384 hardcoded for bundled model) — same deferred item as Qdrant
- Database creation (`CREATE DATABASE IF NOT EXISTS`) — require database to exist, document in README

</deferred>

---

*Phase: 24-postgres-backend*
*Context gathered: 2026-03-21*
