# Stack Research

**Domain:** Pluggable storage backends for Rust vector memory server (Qdrant, Postgres)
**Researched:** 2026-03-21 (v1.4 update)
**Confidence:** HIGH for library choices; MEDIUM for feature flag patterns (verified via official docs + crates.io, no Context7 coverage)

---

## Context: What Already Exists (LOCKED)

The following stack is validated across v1.0–v1.3 and must not change:

| Component | Crate | Version |
|-----------|-------|---------|
| HTTP server | axum | 0.8 |
| Async runtime | tokio | 1 (full) |
| SQLite | rusqlite (bundled) + sqlite-vec | 0.37 + 0.1.7 |
| Async SQLite | tokio-rusqlite | 0.7 |
| Embeddings | candle-core/nn/transformers | 0.9 |
| Async trait dispatch | async-trait | 0.1 |
| HTTP client | reqwest | 0.13 |
| Serialization | serde + serde_json | 1 |
| Error handling | thiserror + anyhow | 2 + 1 |
| Config | figment | 0.10 |
| UUIDs | uuid | 1 (v7) |
| Logging | tracing + tracing-subscriber | 0.1 / 0.3 |
| CLI | clap | 4 |

The project already uses `#[async_trait]` on `EmbeddingEngine`. The storage trait must follow the exact same pattern.

---

## New Dependencies Required for v1.4

### Core Additions

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| qdrant-client | 1.17.0 | Qdrant vector DB client | Official SDK from Qdrant team; gRPC via tonic; builder-pattern API; tokio 1.40+ compatible; only maintained client for Qdrant in Rust |
| sqlx | 0.8.6 | Async PostgreSQL client | Pure Rust, async-native, built-in PgPool connection pooling; works with pgvector crate; compile-time query checking optional but available |
| pgvector | 0.4.1 | Rust types for pgvector Postgres extension | Official pgvector Rust support; feature flags for sqlx and tokio-postgres; exports `Vector` type for f32 arrays |

### No Additional Supporting Libraries Needed

`async-trait`, `uuid`, `serde`, `thiserror`, `anyhow`, and `tokio` are already present and cover all supporting needs for the new backends.

---

## Cargo.toml Changes

```toml
# Add to [dependencies] — all optional, guarded by feature flags
qdrant-client = { version = "1.17", optional = true }
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-native-tls", "postgres", "uuid"], optional = true }
pgvector = { version = "0.4", features = ["sqlx"], optional = true }

# Add [features] section
[features]
default = []
backend-qdrant = ["dep:qdrant-client"]
backend-postgres = ["dep:sqlx", "dep:pgvector"]
```

**Why `dep:` prefix:** Rust 1.60+ syntax that prevents implicit feature exposure for optional dependencies. Users enabling `backend-postgres` do not accidentally expose `sqlx` as a feature name on mnemonic.

**Why `native-tls` for sqlx:** The existing `reqwest 0.13` already pulls in native-tls. Using `rustls` would introduce a second TLS stack. Match what the project already uses to minimize binary size and dependency surface.

**Why `uuid` feature in sqlx:** Enables `sqlx::types::Uuid` mapping, consistent with the existing `uuid` crate already in the project.

**Build note:** `qdrant-client` pulls in `tonic 0.12.3` and `prost 0.13.3` transitively. Do NOT add `tonic` directly — let `qdrant-client` own it to avoid version conflicts.

---

## The Storage Trait Pattern

Follow the existing `EmbeddingEngine` pattern in `src/embedding.rs` exactly — same crate, same attribute macro, same object-safety strategy.

```rust
// src/storage/mod.rs
use async_trait::async_trait;
use crate::error::StorageError;
use crate::service::{Memory, CreateMemoryRequest, SearchParams, ListParams};

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn create_memory(&self, req: &CreateMemoryRequest, embedding: Vec<f32>) -> Result<Memory, StorageError>;
    async fn list_memories(&self, params: &ListParams) -> Result<Vec<Memory>, StorageError>;
    async fn get_memory(&self, id: &str) -> Result<Option<Memory>, StorageError>;
    async fn search_memories(&self, embedding: Vec<f32>, params: &SearchParams) -> Result<Vec<Memory>, StorageError>;
    async fn delete_memory(&self, id: &str) -> Result<bool, StorageError>;
    async fn list_for_compaction(&self, agent_id: &str, limit: usize) -> Result<Vec<Memory>, StorageError>;
}
```

`MemoryService` becomes backend-agnostic:
```rust
pub struct MemoryService {
    pub storage: Arc<dyn StorageBackend>,
    pub embedding: Arc<dyn EmbeddingEngine>,
    pub embedding_model: String,
}
```

**Why `#[async_trait]` instead of native async fn in traits:** Native `async fn` in traits (stabilized Rust 1.75) does NOT support `dyn Trait` — the trait would not be object-safe. `async-trait` transforms methods to `Pin<Box<dyn Future>>`, which enables `Arc<dyn StorageBackend>`. This is the correct and required approach as of March 2026; native dyn async trait support is still in active development (rust-lang/impl-trait-utils#34, baby steps blog March 2025).

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| sqlx 0.8 | tokio-postgres (direct) | tokio-postgres requires manual connection pool (bb8/deadpool); sqlx bundles PgPool and has cleaner pgvector type binding; query pipelining advantage of tokio-postgres is irrelevant for memory workloads |
| sqlx 0.8 | diesel + diesel-async | Diesel requires schema macro codegen; diesel-async adds complexity; Mnemonic uses raw SQL idioms throughout — no benefit from ORM layer |
| qdrant-client 1.17 | qdrant_rest_client (second-state) | Unofficial, incomplete REST-only client; official SDK has full API coverage, maintained by Qdrant team, actively versioned |
| Cargo feature flags | Runtime config selection | Feature flags keep the binary lean — each backend only compiled when opted in; runtime selection would compile all three backends into every binary |
| async-trait | Native RPITIT | Native async fn in traits cannot be used as `dyn Trait` as of Rust 1.85; async-trait is the only current solution for object-safe async traits |

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `diesel` | Adds schema codegen + DSL overhead; incompatible with project's raw SQL pattern | `sqlx` with raw SQL |
| `sea-orm` | ORM indirection over project that uses direct SQL; unnecessary complexity | `sqlx` |
| `bb8` or `deadpool` | sqlx 0.8 has built-in `PgPool` — external pool crate is redundant | `sqlx::PgPool` |
| `tokio-postgres` direct | More setup for identical result; pgvector binding cleaner through sqlx+pgvector | `sqlx` + `pgvector` |
| `qdrant_rest_client` | Unofficial, incomplete, not maintained by Qdrant | `qdrant-client` |
| `tonic` direct | `qdrant-client` owns tonic 0.12.3 transitively — direct add risks version conflict | Let qdrant-client manage it |
| Bumping `rusqlite` to 0.39 | Known version conflict with `sqlite-vec 0.1.7` and `libsqlite3-sys` | Keep at 0.37 |

---

## Stack Patterns by Backend

**SQLite backend (default, zero-config):**
- Wrap `tokio_rusqlite::Connection` in a `SqliteStorage` struct
- Implement `StorageBackend` on `SqliteStorage`
- Keep `register_sqlite_vec()` and all schema migrations in `src/storage/sqlite.rs`
- No new dependencies

**Qdrant backend (feature flag: `backend-qdrant`):**
- `Qdrant::from_url(url).api_key(key).build()?` for connection
- Uses gRPC — requires a running Qdrant server (not embedded)
- Point IDs: use existing `uuid` crate; enable `uuid` feature on `qdrant-client`
- Payload fields in Qdrant points carry `content`, `agent_id`, `session_id`, `tags`, `created_at`
- Vector dimension must match 384 (all-MiniLM-L6-v2 output)
- Implement `StorageBackend` in `src/storage/qdrant.rs` behind `#[cfg(feature = "backend-qdrant")]`

**Postgres backend (feature flag: `backend-postgres`):**
- `sqlx::PgPool::connect(url).await?` for connection pool
- Requires pgvector extension installed on the Postgres server (`CREATE EXTENSION vector`)
- Column type: `vector(384)` for embeddings
- Bind vectors: `pgvector::Vector::from(vec_f32)` in sqlx queries
- `pgvector = { version = "0.4", features = ["sqlx"] }` — required to get `Type` impl for sqlx
- Implement `StorageBackend` in `src/storage/postgres.rs` behind `#[cfg(feature = "backend-postgres")]`

---

## Version Compatibility Matrix

| Package | Version | Compatible With | Notes |
|---------|---------|-----------------|-------|
| qdrant-client 1.17 | tonic ^0.12.3 | tokio ^1.40 | Do not add tonic directly |
| sqlx 0.8.6 | postgres, runtime-tokio | tokio 1.x, native-tls | Match existing reqwest TLS backend |
| pgvector 0.4.1 | sqlx ^0.8 | postgres-types ^0.2 | Must enable `sqlx` feature |
| rusqlite 0.37 | sqlite-vec 0.1.7 | libsqlite3-sys (bundled) | Do not upgrade to 0.39 |
| async-trait 0.1 | already present | tokio 1.x | Follow EmbeddingEngine pattern |
| reqwest 0.13 | native-tls | tokio 1.x | Match sqlx TLS choice |

---

## Integration Points in Existing Codebase

| File | Change Required |
|------|----------------|
| `src/service.rs` | Replace `Arc<Connection>` field with `Arc<dyn StorageBackend>`; all SQL moves out |
| `src/db.rs` | Becomes thin — keep `register_sqlite_vec()`, move SQL into `src/storage/sqlite.rs` |
| `src/main.rs` / `src/cli.rs` | Read backend config at startup; construct appropriate backend; wrap in `Arc<dyn StorageBackend>` |
| `src/config.rs` | Add `backend` enum (`sqlite` default, `qdrant`, `postgres`) with URL/credentials fields |
| `src/error.rs` | Add `StorageError` enum covering all three backends |
| `Cargo.toml` | Add optional deps + feature flags (see above) |
| New: `src/storage/mod.rs` | `StorageBackend` trait definition |
| New: `src/storage/sqlite.rs` | `SqliteStorage` implementing `StorageBackend` |
| New: `src/storage/qdrant.rs` | `QdrantStorage` implementing `StorageBackend` (cfg-gated) |
| New: `src/storage/postgres.rs` | `PostgresStorage` implementing `StorageBackend` (cfg-gated) |

---

## Sources

- [qdrant-client docs.rs 1.17](https://docs.rs/qdrant-client/latest/qdrant_client/index.html) — API overview, tonic 0.12.3, tokio 1.40+ confirmed (HIGH)
- [qdrant/rust-client Cargo.toml (master)](https://raw.githubusercontent.com/qdrant/rust-client/master/Cargo.toml) — Exact deps and feature list (HIGH)
- [qdrant/rust-client README](https://github.com/qdrant/rust-client/blob/master/README.md) — Upsert and search API examples (HIGH)
- [pgvector 0.4.1 docs.rs](https://docs.rs/pgvector/latest/pgvector/) — Feature flags, sqlx integration, Vector types (HIGH)
- [sqlx 0.8.6 docs.rs](https://docs.rs/sqlx/latest/sqlx/) — Runtime features, PgPool, Postgres support (HIGH)
- [axum sqlx-postgres example](https://github.com/tokio-rs/axum/blob/main/examples/sqlx-postgres/Cargo.toml) — Confirmed sqlx 0.8 feature pattern (MEDIUM)
- [Rust async fn in traits blog (Dec 2023)](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/) — Confirms native async fn not object-safe (HIGH)
- [rust-lang/impl-trait-utils#34](https://github.com/rust-lang/impl-trait-utils/issues/34) — dyn async trait still in development (HIGH)
- [baby steps: dyn async traits part 10 (Mar 2025)](https://smallcultfollowing.com/babysteps/blog/2025/03/24/box-box-box/) — Latest state of native dyn async (MEDIUM)

---

*Stack research for: Mnemonic v1.4 — Pluggable Storage Backends*
*Researched: 2026-03-21*
