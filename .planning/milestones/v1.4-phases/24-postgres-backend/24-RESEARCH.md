# Phase 24: Postgres Backend - Research

**Researched:** 2026-03-21
**Domain:** Rust / sqlx 0.8 / pgvector / PostgreSQL async storage backend
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Single table `memories` — all agents share one table, isolated by SQL WHERE on `agent_id`
- **D-02:** Columns: `id TEXT PRIMARY KEY`, `content TEXT NOT NULL`, `agent_id TEXT NOT NULL`, `session_id TEXT NOT NULL`, `tags TEXT[] NOT NULL DEFAULT '{}'`, `embedding_model TEXT NOT NULL`, `embedding vector(384) NOT NULL`, `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`, `updated_at TIMESTAMPTZ`
- **D-03:** Indexes at startup: `idx_memories_agent_id`, `idx_memories_session_id`, `idx_memories_created_at`, HNSW on embedding with `vector_cosine_ops`
- **D-04:** Schema auto-creation: `CREATE EXTENSION IF NOT EXISTS vector` then `CREATE TABLE IF NOT EXISTS memories (...)` then all `CREATE INDEX IF NOT EXISTS ...`. Idempotent.
- **D-05:** `sqlx` with `postgres` and `runtime-tokio` features
- **D-06:** `pgvector` crate with `sqlx` feature — provides `Type`/`Encode`/`Decode` impls for `vector(N)` columns
- **D-07:** Both are optional dependencies gated behind `backend-postgres` feature in Cargo.toml
- **D-08:** `PostgresBackend::new(config: &Config) -> Result<Self, ApiError>` reads `config.postgres_url`, creates `PgPool`, runs schema auto-creation
- **D-09:** `PgPool` stored directly in struct — `Send + Sync`, handles pooling internally
- **D-10:** sqlx defaults (10 connections) — no custom pool config
- **D-11:** pgvector `<=>` returns distance directly (lower = more similar). No score conversion needed.
- **D-12:** Search query: `SELECT *, embedding <=> $1::vector AS distance FROM memories WHERE agent_id = $2 ... ORDER BY distance ASC LIMIT $3`
- **D-13:** `write_compaction_result()` uses Postgres transaction: BEGIN → INSERT merged → DELETE source IDs → COMMIT
- **D-14:** Full atomicity via Postgres transactions satisfies PGVR-03
- **D-15:** `list()` uses `ORDER BY created_at DESC LIMIT $1 OFFSET $2`
- **D-16:** Filters via SQL WHERE: `agent_id = $1`, `session_id = $2`, `tags @> ARRAY[$3]::text[]`, `created_at >= $4`, `created_at <= $5`
- **D-17:** Total count: `SELECT COUNT(*) FROM memories WHERE ...` with same filters
- **D-18:** `<=>` with SQL WHERE — filters and vector search in one query, no over-fetch
- **D-19:** Threshold filtering: `WHERE embedding <=> $1::vector <= $2` — SQL-pushed
- **D-20:** `fetch_candidates`: `SELECT id, content, tags, created_at, embedding FROM memories WHERE agent_id = $1 ORDER BY created_at DESC LIMIT $2`
- **D-21:** Over-fetch by one (`max_candidates + 1`) to detect truncation
- **D-22:** Tags stored as `TEXT[]` — enables `@>` containment for filtering
- **D-23:** Store: `Vec<String>` → `&[String]` binding; retrieve: sqlx decodes `TEXT[]` → `Vec<String>` automatically
- **D-24:** New file `src/storage/postgres.rs` with `#[cfg(feature = "backend-postgres")]`
- **D-25:** `src/storage/mod.rs` gets `#[cfg(feature = "backend-postgres")] pub mod postgres;` and conditional re-export
- **D-26:** `Cargo.toml` `backend-postgres` feature gains: `sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"], optional = true }` and `pgvector = { version = "0.4", features = ["sqlx"], optional = true }`
- **D-27:** Wire `PostgresBackend::new(config).await?` into `create_backend()` factory replacing `todo!()`
- **D-28:** Unit tests in `postgres.rs` test helpers without live Postgres
- **D-29:** Integration tests behind `#[cfg(all(test, feature = "backend-postgres"))]` with docker setup documented
- **D-30:** Existing 273+ tests must pass unchanged without `--features backend-postgres`

### Claude's Discretion

- Exact sqlx and pgvector crate versions (latest stable)
- Internal helper function decomposition within PostgresBackend
- Error message wording for Postgres connection failures
- PgPool configuration (max connections, timeouts)
- Whether to add database creation logic or require the database to exist
- Exact HNSW index parameters (m, ef_construction)

### Deferred Ideas (OUT OF SCOPE)

- Connection string with SSL/TLS options — document but don't add custom config fields beyond postgres_url
- Read replicas for search queries
- Postgres connection pool tuning as config fields
- IVFFLAT vs HNSW index selection as a config option
- Vector dimension as a config field (384 hardcoded)
- Database creation (`CREATE DATABASE IF NOT EXISTS`) — require database to exist
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PGVR-01 | PostgresBackend implements StorageBackend using sqlx + pgvector, feature-gated behind backend-postgres | sqlx 0.8.6 + pgvector 0.4.1 verified as the correct crate combination; `#[async_trait]` pattern established in project |
| PGVR-02 | Vector search uses pgvector cosine distance operator with proper indexing | `<=>` operator confirmed; HNSW with `vector_cosine_ops` is the recommended index type; pgvector `Vector` type binds via `.bind()` |
| PGVR-03 | Compaction uses Postgres transactions for atomic delete+insert | sqlx `pool.begin()` / `tx.commit()` pattern confirmed; `&mut *tx` dereference for executor |
| PGVR-04 | Multi-agent namespace isolation via SQL WHERE filtering on agent_id | Standard SQL `WHERE agent_id = $1` — no special mechanism needed |
</phase_requirements>

---

## Summary

Phase 24 implements `PostgresBackend` — the third storage backend for Mnemonic after SQLite (Phase 21) and Qdrant (Phase 23). The implementation is exclusively SQL, using `sqlx` (async Rust SQL toolkit) and `pgvector` (Postgres vector extension bindings). Both crates are well-established, current, and designed to work together.

The design is thoroughly pre-decided in CONTEXT.md. Research confirms all decisions are implementable with straightforward APIs. The primary technical finding is how the two crates integrate: `pgvector 0.4` provides `Vector` as a Rust type that implements sqlx's `Type`, `Encode`, and `Decode` traits, meaning it binds to queries with `.bind(Vector::from(vec_of_f32))` and decodes from rows with `row.try_get::<Vector, _>("embedding")`. This is the only pgvector-specific API beyond standard SQL.

The Postgres backend has a genuine advantage over both prior backends: full ACID transactions mean `write_compaction_result()` is truly atomic — not an approximation like Qdrant's upsert-then-delete, and not limited to in-process like SQLite.

**Primary recommendation:** Follow the QdrantBackend structural template exactly, substituting sqlx pool operations for gRPC calls. The helper function decomposition should mirror qdrant.rs (struct → `new()` + `ensure_schema()` + helper fns + trait impl + unit tests).

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | 0.8.6 | Async Postgres driver with connection pooling | Official async-native Rust SQL toolkit; matches tokio runtime; `PgPool` is `Send + Sync`; used by `backend-postgres` feature decision |
| pgvector | 0.4.1 | `Vector` type with sqlx `Type`/`Encode`/`Decode` impls | Official pgvector Rust bindings; `features = ["sqlx"]` activates sqlx integration; version 0.4 required for sqlx 0.8 |

**Version verification (confirmed 2026-03-21):**
- `sqlx 0.8.6` — published 2025-05-19, latest stable (0.9.0-alpha.1 is prerelease, not for production)
- `pgvector 0.4.1` — published 2025-05-20, latest stable

### Installation (Cargo.toml additions)
```toml
[features]
# Update this existing line:
backend-postgres = ["dep:sqlx", "dep:pgvector"]

[dependencies]
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"], optional = true }
pgvector = { version = "0.4", features = ["sqlx"], optional = true }
```

Note: `runtime-tokio` and `postgres` are the minimal feature set needed. TLS features (`tls-native-tls`, `tls-rustls`) are omitted per D-05; SSL/TLS support is deferred. This matches the project's postgres_url-only config.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `sqlx` | `tokio-postgres` | tokio-postgres is lower-level; no pool built-in; sqlx provides higher-level ergonomics and connection pooling that matches the project's needs |
| `pgvector 0.4` | Manual `Vec<u8>` serialization | pgvector crate handles all binary encoding; manual serialization is error-prone and fragile |
| `sqlx::query()` (runtime) | `sqlx::query!()` (compile-time macro) | Macros require DATABASE_URL at compile time and a live DB in CI; runtime queries match the rest of the project and work in feature-gated code |

---

## Architecture Patterns

### Recommended File Structure
```
src/
└── storage/
    ├── mod.rs          # Add #[cfg(feature = "backend-postgres")] pub mod postgres; + re-export
    ├── sqlite.rs       # Unchanged (reference impl)
    ├── qdrant.rs       # Unchanged (structural template)
    └── postgres.rs     # New: PostgresBackend — entire impl
```

### Pattern 1: PostgresBackend Struct and Construction

**What:** `PgPool` stored directly; `ensure_schema()` runs idempotent DDL at startup.
**When to use:** Every `PostgresBackend::new()` call.

```rust
// Source: verified against sqlx 0.8.6 docs + pgvector-rust README
#[cfg(feature = "backend-postgres")]
pub struct PostgresBackend {
    pool: sqlx::PgPool,
}

impl PostgresBackend {
    pub async fn new(config: &Config) -> Result<Self, ApiError> {
        let url = config.postgres_url.as_deref()
            .ok_or_else(|| ApiError::Internal(MnemonicError::Config(
                ConfigError::Load("postgres_url is required when storage_provider is \"postgres\"".to_string())
            )))?;

        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect(url)
            .await
            .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Open(e.to_string()))))?;

        let backend = Self { pool };
        backend.ensure_schema().await?;
        Ok(backend)
    }

    async fn ensure_schema(&self) -> Result<(), ApiError> {
        // CREATE EXTENSION IF NOT EXISTS vector
        // CREATE TABLE IF NOT EXISTS memories (...)
        // CREATE INDEX IF NOT EXISTS idx_memories_agent_id ON memories(agent_id)
        // ... (all indexes from D-03)
        // Each sqlx::query(...).execute(&self.pool).await maps errors to ApiError::Internal(DbError::Schema)
    }
}
```

### Pattern 2: Vector Binding and Retrieval (pgvector)

**What:** Convert `Vec<f32>` to `pgvector::Vector` for INSERT; decode back with `try_get`.
**When to use:** `store()`, `search()`, `fetch_candidates()`, `write_compaction_result()`

```rust
// Source: pgvector-rust README (github.com/pgvector/pgvector-rust)
use pgvector::Vector;

// Binding: Vec<f32> -> Vector for INSERT
let embedding = Vector::from(req.embedding.clone());
sqlx::query("INSERT INTO memories (..., embedding) VALUES (..., $1)")
    .bind(embedding)
    .execute(&self.pool)
    .await?;

// Retrieval: decode Vector from row, convert back to Vec<f32>
let row = sqlx::query("SELECT embedding FROM memories WHERE id = $1")
    .bind(id)
    .fetch_optional(&self.pool)
    .await?;
// In fetch_candidates, get embedding field:
let vec: Vector = row.try_get("embedding")?;
let embedding: Vec<f32> = vec.into();
```

### Pattern 3: Transactions for Atomic Compaction (PGVR-03)

**What:** `pool.begin()` returns a transaction; execute queries via `&mut *tx`; call `tx.commit()`.
**When to use:** `write_compaction_result()` exclusively.

```rust
// Source: sqlx docs (docs.rs/sqlx/0.8.6/sqlx/struct.Transaction.html)
let mut tx = self.pool.begin().await
    .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

// INSERT merged memory
sqlx::query("INSERT INTO memories (...) VALUES (...)")
    .bind(...)
    .execute(&mut *tx)  // Deref: &mut *tx as executor
    .await
    .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

// DELETE source memories
sqlx::query("DELETE FROM memories WHERE id = ANY($1)")
    .bind(&req.source_ids[..])  // &[String] binds as TEXT[]
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;

tx.commit().await
    .map_err(|e| ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string()))))?;
```

If `tx` drops without `commit()`, sqlx automatically issues ROLLBACK.

### Pattern 4: Tag Array Binding (TEXT[])

**What:** `Vec<String>` binds as `TEXT[]` in sqlx postgres; `&[String]` also works. Retrieval decodes automatically.
**When to use:** All 7 StorageBackend methods that read or write tags.

```rust
// Store tags: Vec<String> -> TEXT[]
sqlx::query("INSERT INTO memories (..., tags, ...) VALUES (..., $1, ...)")
    .bind(&req.tags[..])  // &[String] slice binds as TEXT[]
    .execute(&self.pool).await?;

// Filter by tag: @> containment
sqlx::query("... WHERE tags @> ARRAY[$1]::text[]")
    .bind(tag_value)  // single &str or String
    .fetch_all(&self.pool).await?;

// Delete multiple IDs atomically:
sqlx::query("DELETE FROM memories WHERE id = ANY($1)")
    .bind(&source_ids[..])  // &[String] -> TEXT[]
    .execute(&mut *tx).await?;
```

### Pattern 5: Optional Filter Building for list() and search()

Because sqlx does not support dynamic SQL in `query!` macros, and because list/search require conditional WHERE clauses, use runtime `sqlx::query()` with SQL string building. The pattern used in SqliteBackend (build WHERE clause as string with `$N` params) also applies here, with numbered params (`$1`, `$2`, etc.) instead of SQLite's `?N`.

```rust
// Build WHERE clause dynamically, track param index
let mut conditions = Vec::<String>::new();
let mut idx: i32 = 1;

conditions.push(format!("agent_id = ${}", idx)); idx += 1;  // always required

if params.session_id.is_some() {
    conditions.push(format!("session_id = ${}", idx)); idx += 1;
}
if params.tag.is_some() {
    conditions.push(format!("tags @> ARRAY[${}]::text[]", idx)); idx += 1;
}
// ... etc

let where_clause = if conditions.is_empty() {
    String::new()
} else {
    format!("WHERE {}", conditions.join(" AND "))
};

let sql = format!("SELECT ... FROM memories {} ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
    where_clause, idx, idx + 1);
```

### Anti-Patterns to Avoid

- **Using `sqlx::query!` macro for feature-gated code:** The compile-time macro requires DATABASE_URL set at compile time. For optional feature backends, use `sqlx::query()` runtime function.
- **Not dereferencing the transaction:** `execute(&tx)` (shared ref) will not compile — must use `execute(&mut *tx)`.
- **Using chrono or time features:** Not needed — `created_at` and `updated_at` are stored as `TEXT` strings in `Memory` struct. Use `$1::timestamptz` cast in SQL when inserting the string literal, or use `CURRENT_TIMESTAMP` defaults. Store as ISO 8601 TEXT, return as TEXT (matches SqliteBackend contract).
- **Wrapping PgPool in Arc:** `PgPool` is already `Clone + Send + Sync` and internally reference-counted. No extra `Arc` needed (matches D-09, same as QdrantBackend's client).
- **`binding` query vector without `::vector` cast:** Postgres may fail to infer the type for `$1` when pgvector is involved. Use explicit cast in SQL: `$1::vector` for safety when the param type may be ambiguous.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Connection pooling | Custom pool / single connection | `sqlx::PgPool` (via `PgPoolOptions::new().connect(url)`) | Thread-safe, reconnects, backpressure, idle timeout — dozens of edge cases |
| Vector type encoding | Manual `f32` → byte serialization | `pgvector::Vector` with `features = ["sqlx"]` | pgvector binary format is version-specific; the crate handles it |
| Postgres array encoding | Custom TEXT[] serialization | `&[String]` bind directly | sqlx postgres driver handles array wire protocol automatically |
| Transaction rollback on panic | Explicit rollback | sqlx `Transaction` Drop impl | If tx goes out of scope without `commit()`, sqlx auto-rolls back |
| Time formatting | Custom ISO 8601 formatter | `DEFAULT NOW()` in schema + return TEXT from `created_at` column | Postgres handles the default; Memory struct stores as String |

**Key insight:** sqlx + pgvector is a complete toolkit. The only custom code needed is SQL strings — all type encoding, connection management, and transaction semantics are handled by the crates.

---

## Common Pitfalls

### Pitfall 1: sqlx numbered params vs SQLite's `?`
**What goes wrong:** Writing `?1` or `?` in SQL instead of `$1`, `$2` — sqlx postgres requires `$N` numbered params.
**Why it happens:** SqliteBackend uses `?N` (rusqlite syntax); developers copy the pattern.
**How to avoid:** Always use `$1`, `$2`, ... in all sqlx Postgres SQL strings.
**Warning signs:** `sqlx::Error::Protocol` or `ERROR: syntax error at or near "?"` at runtime.

### Pitfall 2: `query!` macro vs `query()` function in feature-gated code
**What goes wrong:** Using `sqlx::query!("...")` which requires `DATABASE_URL` at compile time and a live DB in CI.
**Why it happens:** sqlx macros are the "recommended" path in tutorials.
**How to avoid:** Use `sqlx::query("...")` (runtime) throughout PostgresBackend. No `DATABASE_URL` needed.
**Warning signs:** Build fails with "error: `DATABASE_URL` must be set" when `--features backend-postgres` is enabled.

### Pitfall 3: Transaction executor dereferencing
**What goes wrong:** `sqlx::query(...).execute(&tx)` fails — `Transaction` doesn't implement `Executor` via shared ref.
**Why it happens:** Pool and connection both work with `&pool` / `&conn`, so developers expect the same for transactions.
**How to avoid:** Always use `execute(&mut *tx)` — the `DerefMut` impl on `Transaction` exposes the underlying connection.
**Warning signs:** Compile error: `the trait Executor is not implemented for &Transaction<'_, Postgres>`.

### Pitfall 4: pgvector extension not installed
**What goes wrong:** `CREATE EXTENSION IF NOT EXISTS vector` fails with "extension not available".
**Why it happens:** pgvector is a Postgres extension that must be pre-installed in the Postgres image.
**How to avoid:** Document that users must use a pgvector-enabled Postgres. For tests, use `pgvector/pgvector:pg17` Docker image (official image with extension pre-installed).
**Warning signs:** `ERROR: could not open extension control file "vector.control"` at startup.

### Pitfall 5: TIMESTAMPTZ vs TEXT mismatch with Memory.created_at
**What goes wrong:** Storing `created_at` as `TIMESTAMPTZ` in schema but the `Memory` struct holds `created_at: String`.
**Why it happens:** The column type is TIMESTAMPTZ, but sqlx returns it as a `chrono::DateTime` unless the `chrono` feature is enabled.
**How to avoid:** Two options: (a) use `sqlx::query(...).fetch_one(...).await?` and cast in SQL: `created_at::text AS created_at` to get ISO 8601 string back; or (b) read `created_at` as String directly — sqlx will format it as ISO 8601. The safest approach is to SELECT with `TO_CHAR(created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at` or cast: `created_at AT TIME ZONE 'UTC' || '' AS created_at`. Alternatively, store the `created_at` in the INSERT as a text string converted to timestamptz: `$1::timestamptz` and retrieve with `created_at::text`.
**Warning signs:** Compile error about missing `chrono` feature, or runtime type decoding error on TIMESTAMPTZ column.

**Recommended approach for created_at handling:** Insert using `NOW()` default (per D-02 schema), then SELECT `TO_CHAR(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at` to get back a consistent ISO 8601 string matching the format used throughout the project (see `now_iso8601()` in qdrant.rs). This avoids adding `chrono` as a dependency.

### Pitfall 6: `session_id` in `write_compaction_result`
**What goes wrong:** SqliteBackend and QdrantBackend both store `session_id = ''` (empty string) for merged compaction memories. Postgres `session_id TEXT NOT NULL` will accept empty string, but the code must explicitly pass `""` not `NULL`.
**Why it happens:** The MergedMemoryRequest struct doesn't have a session_id field — it's a compaction artifact.
**How to avoid:** In the INSERT for compaction, bind `""` (empty string) for session_id explicitly.

### Pitfall 7: Dynamic WHERE clause parameter indexing
**What goes wrong:** With optional filters, building `$N` params gets off-by-one errors if filter presence is not tracked carefully.
**Why it happens:** Unlike SQLite's `?N IS NULL OR col = ?N` trick, Postgres requires explicit conditional inclusion.
**How to avoid:** Use a `param_idx: i32` counter that increments as each condition is added. Alternatively, use sqlx's `QueryBuilder` for safe parameter building with push_bind. The `QueryBuilder` approach is cleaner for complex optional filtering.

---

## Code Examples

### Schema DDL (ensure_schema)

```sql
-- Source: D-02, D-03, D-04 from CONTEXT.md
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS memories (
    id              TEXT PRIMARY KEY,
    content         TEXT NOT NULL,
    agent_id        TEXT NOT NULL,
    session_id      TEXT NOT NULL,
    tags            TEXT[] NOT NULL DEFAULT '{}',
    embedding_model TEXT NOT NULL,
    embedding       vector(384) NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_memories_agent_id ON memories(agent_id);
CREATE INDEX IF NOT EXISTS idx_memories_session_id ON memories(session_id);
CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at);
CREATE INDEX IF NOT EXISTS idx_memories_embedding ON memories USING hnsw (embedding vector_cosine_ops);
```

### Semantic Search with Threshold

```sql
-- Source: D-12, D-18, D-19 from CONTEXT.md
-- Bind order: $1 = embedding::vector, $2 = agent_id, $3 = threshold (optional), $4 = limit
SELECT
    id, content, agent_id, session_id, tags, embedding_model,
    TO_CHAR(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at,
    TO_CHAR(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS updated_at,
    embedding <=> $1::vector AS distance
FROM memories
WHERE agent_id = $2
  AND embedding <=> $1::vector <= $3   -- threshold filter (when provided)
ORDER BY distance ASC
LIMIT $4
```

### fetch_candidates

```sql
-- Source: D-20, D-21 from CONTEXT.md
-- Bind: $1 = agent_id, $2 = max_candidates + 1
SELECT
    id, content, tags,
    TO_CHAR(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at,
    embedding
FROM memories
WHERE agent_id = $1
ORDER BY created_at DESC
LIMIT $2
```

### Compaction Transaction

```rust
// Source: D-13, D-14 from CONTEXT.md + sqlx Transaction docs
let mut tx = self.pool.begin().await.map_err(map_db_err)?;

sqlx::query(
    "INSERT INTO memories (id, content, agent_id, session_id, tags, embedding_model, created_at, embedding)
     VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz, $8::vector)"
)
.bind(&req.new_id)
.bind(&req.content)
.bind(&req.agent_id)
.bind("")                                   // session_id = empty for merged
.bind(&req.tags[..])                        // TEXT[]
.bind(&req.embedding_model)
.bind(&req.created_at)                      // ISO 8601 string cast to timestamptz
.bind(Vector::from(req.embedding.clone()))  // pgvector::Vector
.execute(&mut *tx)
.await
.map_err(map_db_err)?;

sqlx::query("DELETE FROM memories WHERE id = ANY($1)")
    .bind(&req.source_ids[..])  // &[String] -> TEXT[]
    .execute(&mut *tx)
    .await
    .map_err(map_db_err)?;

tx.commit().await.map_err(map_db_err)?;
```

### Error Mapping Helper

```rust
// Consistent with existing error mapping pattern in qdrant.rs
fn map_db_err(e: sqlx::Error) -> ApiError {
    ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string())))
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `pgvector 0.3` for sqlx | `pgvector 0.4` for sqlx 0.8 | July 2024 | Must use 0.4 with sqlx 0.8; 0.3 is for sqlx <0.8 |
| `sqlx 0.7` | `sqlx 0.8` | August 2024 | Pool API minor changes; 0.8.6 is current stable |
| IVFFLAT indexes | HNSW indexes | pgvector 0.5 (2023) | HNSW recommended for most use cases: better recall, no training phase |
| `runtime-tokio-native-tls` | `runtime-tokio` + separate tls features | sqlx 0.7+ | TLS is now opt-in; `runtime-tokio` alone works for non-TLS connections |

**Deprecated/outdated:**
- `sqlx 0.9.0-alpha.1`: Prerelease only — do not use in production. Use 0.8.6.
- `pgvector 0.3`: Compatible with sqlx <0.8 only. This project uses sqlx 0.8 — use 0.4.1.

---

## Open Questions

1. **created_at storage and retrieval format**
   - What we know: Schema column is `TIMESTAMPTZ`; `Memory.created_at` is `String`. sqlx won't auto-decode TIMESTAMPTZ to String without `chrono` feature.
   - What's unclear: Whether to use `TO_CHAR(created_at AT TIME ZONE 'UTC', ...)` in every SELECT, or to add `chrono` as an optional dep, or to insert/select as text.
   - Recommendation: Use `TO_CHAR(...)` in all SELECT statements to avoid adding `chrono` dependency. For INSERT of `created_at` in `write_compaction_result`, bind as `$N::timestamptz` or let `DEFAULT NOW()` handle it and re-read via RETURNING clause. The planner should make this explicit.

2. **QueryBuilder vs manual string building for optional filters**
   - What we know: sqlx provides `QueryBuilder` for safe dynamic query building with push_bind; manual `$N` index tracking is error-prone.
   - What's unclear: Whether `QueryBuilder` handles the pgvector `::vector` cast syntax cleanly.
   - Recommendation: Use `QueryBuilder` for `list()` and `search()` optional filters. It's the idiomatic sqlx approach for dynamic queries and avoids off-by-one parameter index bugs.

3. **`updated_at` column handling**
   - What we know: Column is `TIMESTAMPTZ` nullable. Memory struct holds `updated_at: Option<String>`. The project never sets updated_at (it's None in all backends).
   - What's unclear: How sqlx decodes nullable TIMESTAMPTZ to `Option<String>` without chrono.
   - Recommendation: Use `TO_CHAR(updated_at AT TIME ZONE 'UTC', ...) AS updated_at` with `NULL` passing through as SQL NULL, which sqlx decodes as `None` for `Option<String>`.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test runner (`cargo test`) |
| Config file | None — tests use `#[tokio::test]` and `#[test]` attributes |
| Quick run command | `cargo test --lib 2>&1 \| tail -5` (unit tests only, no feature needed) |
| Full suite command | `cargo test 2>&1 \| tail -10` (all 273+ tests) |
| Feature-gated run | `docker run -d -e POSTGRES_PASSWORD=test -p 5432:5432 pgvector/pgvector:pg17 && MNEMONIC_POSTGRES_URL=postgres://postgres:test@localhost/mnemonic cargo test --features backend-postgres 2>&1 \| tail -20` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PGVR-01 | PostgresBackend implements StorageBackend; compiles with `--features backend-postgres` | compile-time | `cargo build --features backend-postgres` | Wave 0 |
| PGVR-01 | All 7 trait methods return correct types | unit (no DB) | `cargo test --features backend-postgres --lib storage::postgres` | Wave 0 |
| PGVR-02 | Cosine search returns results ordered by distance ASC | integration | `cargo test --features backend-postgres -- test_postgres_search_ordered` | Wave 0 |
| PGVR-03 | write_compaction_result is atomic (interrupt = consistent state) | integration | `cargo test --features backend-postgres -- test_postgres_compaction_atomic` | Wave 0 |
| PGVR-04 | agent_id isolation — agent-A cannot see agent-B memories | integration | `cargo test --features backend-postgres -- test_postgres_agent_isolation` | Wave 0 |
| PGVR-01 | Existing 273+ tests pass without `--features backend-postgres` | regression | `cargo test` | Existing |

### Sampling Rate
- **Per task commit:** `cargo test --lib 2>&1 | tail -5` (existing unit tests, confirms no regressions)
- **Per wave merge:** `cargo test 2>&1 | tail -10` (full suite without postgres feature)
- **Phase gate:** Full suite + `cargo build --features backend-postgres` green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/storage/postgres.rs` — new file, does not exist yet
- [ ] Integration tests for PGVR-02, PGVR-03, PGVR-04 do not exist yet (in `tests/` or inline `#[cfg(all(test, feature = "backend-postgres"))]`)
- [ ] Docker setup documented for running integration tests (in README or test module doc comment)

---

## Sources

### Primary (HIGH confidence)
- `crates.io/api/v1/crates/sqlx/versions` — confirmed sqlx 0.8.6 is latest stable (2025-05-19)
- `crates.io/api/v1/crates/pgvector/versions` — confirmed pgvector 0.4.1 is latest stable (2025-05-20)
- `docs.rs/sqlx/0.8.6/sqlx/struct.Transaction.html` — `pool.begin()`, `&mut *tx` executor deref, auto-rollback on drop
- `github.com/pgvector/pgvector-rust` (README) — `Vector::from(vec)`, `.bind(embedding)`, `row.try_get::<Vector>("embedding")?`, `vec.into(): Vec<f32>`, HNSW `vector_cosine_ops`
- `docs.rs/sqlx/latest/sqlx/postgres/types/index.html` — `Vec<String>` / `&[String]` binds as `TEXT[]`; `PgHasArrayType` trait

### Secondary (MEDIUM confidence)
- `docs.rs/pgvector/0.4.1/pgvector/` — `Vector` API overview
- sqlx README examples (github.com/launchbadge/sqlx) — `PgPoolOptions::new().connect(url)`, `sqlx::query()` runtime function, `.fetch_all()` / `.fetch_one()` / `.execute()` / `.fetch_optional()`
- Multiple WebSearch results corroborating `pgvector 0.4` requires `sqlx 0.8+` (0.3 for older sqlx)

### Tertiary (LOW confidence)
- `TO_CHAR(created_at AT TIME ZONE 'UTC', ...)` approach for TIMESTAMPTZ → String — not explicitly confirmed in official docs but is standard Postgres SQL; alternative is adding `chrono` feature which is well-documented

### Project codebase (HIGH confidence — definitive)
- `src/storage/qdrant.rs` — structural template; `ensure_collection()` pattern, `#[async_trait]` usage, helper function decomposition, unit test pattern
- `src/storage/sqlite.rs` — behavioral reference; all 7 method semantics, return types, error handling
- `src/storage/mod.rs` — trait definition, `create_backend()` factory with `todo!()` stub at lines 128-133
- `src/config.rs` — `postgres_url: Option<String>` field already present
- `Cargo.toml` — `backend-postgres = []` feature already declared (line 17); needs sqlx/pgvector deps added

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — versions verified against crates.io registry 2026-03-21
- Architecture: HIGH — all decisions pre-made in CONTEXT.md; confirmed implementable with verified APIs
- Pitfalls: HIGH — based on project code analysis + verified sqlx/pgvector documentation
- created_at format: MEDIUM — TO_CHAR approach is standard Postgres but not explicitly tested in project

**Research date:** 2026-03-21
**Valid until:** 2026-06-21 (90 days — sqlx/pgvector are stable; unlikely to change breaking APIs)
