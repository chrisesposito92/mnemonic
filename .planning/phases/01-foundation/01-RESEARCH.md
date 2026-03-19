# Phase 1: Foundation - Research

**Researched:** 2026-03-19
**Domain:** Rust binary, SQLite + sqlite-vec, tokio-rusqlite async, axum HTTP, layered configuration
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Schema design**
- Full schema from day one — include all columns needed across all phases to avoid migrations
- Columns: id (UUID v7 TEXT PK), content (TEXT), agent_id (TEXT), session_id (TEXT), tags (JSON array in TEXT column), embedding_model (TEXT), created_at (DATETIME), updated_at (DATETIME nullable)
- Tags stored as JSON array in a single TEXT column, queryable via SQLite json_each()
- IDs are UUID v7 (time-ordered, globally unique, TEXT primary key)
- Include updated_at column now (nullable, defaults to null) for future PUT support
- Embedding virtual table via sqlite-vec created alongside the memories table

**Configuration behavior**
- Environment variable prefix: MNEMONIC_ (e.g., MNEMONIC_PORT, MNEMONIC_DB_PATH, MNEMONIC_EMBEDDING_PROVIDER)
- TOML config discovery: look for `mnemonic.toml` in CWD only; override path with MNEMONIC_CONFIG_PATH env var
- Precedence: Environment variables > TOML file > Defaults
- Default database path: ./mnemonic.db (current working directory)
- Default port: 8080
- Default embedding provider: local

**Startup output**
- Logging via tracing + tracing-subscriber (Rust ecosystem standard, future OpenTelemetry compatible)
- Default log level: info (configurable via RUST_LOG)
- Human-readable log format by default (pretty-printed with colors in terminal)
- Compact startup info logs: version, listen address, storage path (WAL mode), embedding provider
- No ASCII art banner — clean structured log lines

**Project layout**
- Single crate (one Cargo.toml, one src/ directory)
- Flat domain modules: src/main.rs, src/config.rs, src/db.rs, src/server.rs, src/error.rs
- Add submodules only when a file gets too large
- Error handling: thiserror for typed errors (db, config), anyhow for main.rs top-level propagation
- Phase 1 includes a placeholder axum server with GET /health returning {"status":"ok"}

### Claude's Discretion
- Exact tracing-subscriber format configuration
- sqlite-vec virtual table naming and column conventions
- TOML config file structure and field naming
- Cargo.toml dependency version pinning strategy
- Test structure (integration tests, unit tests placement)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| STOR-01 | Server persists memories in a single SQLite database file with sqlite-vec for vector search | rusqlite "bundled" feature + sqlite-vec 0.1.7 via sqlite3_auto_extension registration |
| STOR-02 | Server starts with WAL mode enabled and single-writer connection to prevent SQLITE_BUSY errors | `PRAGMA journal_mode=WAL` executed on open; tokio-rusqlite uses single background thread per Connection |
| STOR-03 | All database access uses tokio-rusqlite async closures to avoid blocking the tokio runtime | tokio-rusqlite 0.7.0 Connection::call() pattern sends closures to dedicated background thread |
| STOR-04 | Schema tracks embedding_model per memory row to prevent vector space mismatch | Column included in full schema defined in db.rs; populated by Phase 2 embedding writes |
| CONF-01 | Server runs with zero configuration using sensible defaults (port 8080, local embeddings, ./mnemonic.db) | Figment 0.10.19 with hardcoded Default struct; no args required |
| CONF-02 | User can override settings via environment variables (port, storage path, embedding provider, OpenAI API key) | Figment Env::prefixed("MNEMONIC_") merged over defaults |
| CONF-03 | User can optionally provide a TOML configuration file for all settings | Figment Toml::file("mnemonic.toml") merged between defaults and env vars |
</phase_requirements>

---

## Summary

Phase 1 establishes the Rust project skeleton for Mnemonic: a single-crate binary with SQLite persistence (WAL mode, sqlite-vec extension), async database access via tokio-rusqlite, layered configuration via figment, and a minimal axum HTTP server with a health endpoint.

The Rust ecosystem crates required are all stable and widely used. The most complex integration is loading the sqlite-vec extension — it requires calling `sqlite3_auto_extension` via rusqlite's FFI before any Connection is opened, which must happen exactly once at process startup. This is well-documented in the official sqlite-vec Rust guide.

For configuration, figment 0.10.19 handles the three-layer precedence (defaults → TOML → env vars) idiomatically. For tracing, tracing-subscriber's `.pretty()` formatter with `EnvFilter::from_default_env()` satisfies both the human-readable terminal output requirement and future OpenTelemetry compatibility.

**Primary recommendation:** Wire up the extension registration in main.rs before opening any SQLite connection, use a single `tokio-rusqlite::Connection` (not a pool — WAL handles concurrent reads), and use figment's merge order to encode the correct config precedence.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.50.0 | Async runtime | Standard Rust async runtime; axum and tokio-rusqlite both require it |
| axum | 0.8.8 | HTTP framework | Official Tokio project; ergonomic, macro-free routing |
| rusqlite | 0.39.0 | SQLite bindings | Ergonomic SQLite bindings; "bundled" feature embeds libsqlite3 |
| sqlite-vec | 0.1.7 | Vector search extension | Actively maintained (sqlite-vss is archived); FFI bindings only |
| tokio-rusqlite | 0.7.0 | Async SQLite bridge | Only crate that bridges rusqlite to tokio without blocking the runtime |
| figment | 0.10.19 | Layered configuration | Handles defaults + TOML + env vars with correct precedence natively |
| serde | 1.0.228 | Serialization framework | Required by figment, axum JSON, and schema types |
| serde_json | 1.0.149 | JSON support | Required for JSON API responses and tags column serialization |
| toml | 1.0.7 | TOML file parsing | figment uses it via the "toml" feature flag |
| tracing | 0.1.44 | Structured logging | Standard Tokio ecosystem tracing; OpenTelemetry compatible |
| tracing-subscriber | 0.3.23 | Log formatting/filtering | Companion to tracing; provides EnvFilter and pretty format |
| thiserror | 2.0.18 | Typed error enums | derive(Error) for db.rs and config.rs error types |
| anyhow | 1.0.102 | Top-level error propagation | `?` chaining in main.rs |
| uuid | 1.22.0 | UUID v7 generation | Supports new_v7() for time-ordered primary keys |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| zerocopy | latest | Zero-copy byte slices | Passing Vec<f32> embeddings to sqlite-vec without allocation (Phase 2 uses this) |
| tower | latest | HTTP middleware | axum is built on tower; for future middleware layers |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| figment | manual env parsing + toml crate | figment is 10 lines vs. 100+ lines; no reason to hand-roll |
| figment | config crate | figment has cleaner API, better type inference; both valid |
| thiserror | anyhow everywhere | thiserror gives typed errors; callers can match; better for library-style modules |
| uuid crate | nanoid / cuid2 | uuid v7 is the standard for time-ordered UUIDs; TEXT storage compatible |

**Installation:**
```bash
cargo add tokio --features full
cargo add axum
cargo add rusqlite --features bundled
cargo add sqlite-vec
cargo add tokio-rusqlite
cargo add figment --features toml,env
cargo add serde --features derive
cargo add serde_json
cargo add tracing tracing-subscriber --features tracing-subscriber/env-filter,tracing-subscriber/fmt
cargo add thiserror
cargo add anyhow
cargo add uuid --features v7
```

**Version verification:** Versions above confirmed against crates.io registry (2026-03-19). Training data may lag; verify before writing Cargo.toml by running `cargo search <name>`.

---

## Architecture Patterns

### Recommended Project Structure

```
mnemonic/
├── Cargo.toml
├── src/
│   ├── main.rs       # Entry point: init tracing, load config, open DB, start server
│   ├── config.rs     # Config struct + figment extraction
│   ├── db.rs         # DB init, schema creation, WAL/sqlite-vec setup, Db newtype
│   ├── server.rs     # axum Router factory, AppState, /health handler
│   └── error.rs      # MnemonicError enum (thiserror), DbError, ConfigError
└── tests/
    └── integration.rs # #[tokio::test] tests against in-memory DB
```

### Pattern 1: sqlite-vec Registration via sqlite3_auto_extension

**What:** Registers the sqlite-vec extension with SQLite's global auto-extension list so every Connection opened after this call automatically has the extension loaded.

**When to use:** Call exactly once at process startup in main.rs, before any `Connection::open` call.

**Why not load_extension:** `load_extension` requires a dynamic .so file at runtime, which breaks the single-binary requirement. `sqlite3_auto_extension` statically links the extension at compile time.

```rust
// Source: https://alexgarcia.xyz/sqlite-vec/rust.html
// Cargo.toml: sqlite-vec = "*", rusqlite = { version = "0.39", features = ["bundled"] }
use sqlite_vec::sqlite3_vec_init;
use rusqlite::ffi::sqlite3_auto_extension;

fn register_sqlite_vec() {
    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    }
}
```

Call this once in `main()` before opening any connection.

### Pattern 2: tokio-rusqlite Async Closure

**What:** All SQLite operations are passed as closures to `Connection::call()`. The closure executes on a dedicated background thread; the result is returned asynchronously.

**When to use:** For every database operation — no exceptions. Never use rusqlite's `Connection` directly from an async context.

```rust
// Source: https://docs.rs/tokio-rusqlite/latest/tokio_rusqlite/
use tokio_rusqlite::Connection;

async fn create_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.call(|c| {
        c.execute_batch("
            PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                agent_id TEXT NOT NULL DEFAULT '',
                session_id TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '[]',
                embedding_model TEXT NOT NULL DEFAULT '',
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS vec_memories USING vec0(
                memory_id TEXT PRIMARY KEY,
                embedding float[384]
            );
        ")?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow::anyhow!(e))
}
```

### Pattern 3: Figment Layered Configuration

**What:** Figment merges configuration sources in precedence order. Merge order determines precedence — later merges win.

**When to use:** Always. The three-layer pattern (defaults → TOML file → env vars) is the locked decision.

```rust
// Source: https://docs.rs/figment/latest/figment/
use figment::{Figment, providers::{Toml, Env, Serialized}};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub port: u16,
    pub db_path: String,
    pub embedding_provider: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            db_path: "./mnemonic.db".to_string(),
            embedding_provider: "local".to_string(),
        }
    }
}

pub fn load_config() -> anyhow::Result<Config> {
    // Discover optional TOML config path
    let toml_path = std::env::var("MNEMONIC_CONFIG_PATH")
        .unwrap_or_else(|_| "mnemonic.toml".to_string());

    let config: Config = Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::file(&toml_path))          // optional TOML; missing file is silently ignored
        .merge(Env::prefixed("MNEMONIC_"))       // env vars win over TOML
        .extract()?;

    Ok(config)
}
```

**MNEMONIC_PORT=9090 maps to `port` field** because figment lowercases env var names after stripping the prefix.

### Pattern 4: axum Server with Shared State

**What:** axum 0.8 uses `tokio::net::TcpListener` and `axum::serve()`. App state is wrapped in `Arc` and injected via `with_state()`.

**When to use:** Standard server startup pattern for this version.

```rust
// Source: https://docs.rs/axum/latest/axum/
use axum::{routing::get, Router, extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<tokio_rusqlite::Connection>,
    pub config: Arc<crate::config::Config>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .with_state(state)
}

async fn health_handler() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

pub async fn serve(config: &crate::config::Config, state: AppState) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(address = %addr, "server listening");
    axum::serve(listener, build_router(state)).await?;
    Ok(())
}
```

### Pattern 5: Tracing Initialization

**What:** Set up tracing-subscriber with pretty formatting (human-readable terminal output) and EnvFilter to respect RUST_LOG.

**When to use:** First call in main(), before anything else.

```rust
// Source: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/
use tracing_subscriber::{fmt, EnvFilter, prelude::*};

pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(fmt::layer().pretty())
        .with(EnvFilter::from_default_env()
            .add_directive("mnemonic=info".parse().unwrap()))
        .init();
}
```

This allows `RUST_LOG=debug` to override, defaults to `info` for the mnemonic crate.

### Pattern 6: sqlite-vec Virtual Table Naming

**What:** Naming and column conventions for the vec0 virtual table.

**Recommendation (Claude's Discretion):**
- Table name: `vec_memories`
- Primary key column: `memory_id TEXT` — links back to `memories.id`
- Embedding column: `embedding float[384]` — all-MiniLM-L6-v2 produces 384-dimensional vectors
- Distance metric: default (L2); can be overridden to cosine per project decision in Phase 2

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS vec_memories USING vec0(
    memory_id TEXT PRIMARY KEY,
    embedding float[384]
);
```

**KNN query pattern (for Phase 2/3 reference):**
```sql
-- Basic KNN with k parameter
SELECT memory_id, distance
FROM vec_memories
WHERE embedding MATCH :query_vec
  AND k = 10;

-- With JOIN to memories table (Phase 3 pattern)
WITH knn AS (
    SELECT memory_id, distance
    FROM vec_memories
    WHERE embedding MATCH :query_vec AND k = :k
)
SELECT m.*, knn.distance
FROM knn
JOIN memories m ON m.id = knn.memory_id
WHERE m.agent_id = :agent_id;
```

### Anti-Patterns to Avoid

- **Blocking SQLite calls from async context:** Never call rusqlite methods directly from an async fn. Always wrap in `conn.call(|c| { ... }).await`. Blocking the tokio thread pool causes request timeouts under concurrent load.
- **Opening multiple connections for WAL "pooling":** WAL mode improves concurrent reads but write serialization still applies. For a local agent memory server, a single `tokio-rusqlite::Connection` is correct. Do not create a pool.
- **Calling `sqlite3_auto_extension` multiple times:** It appends to a global list. Calling it twice loads the extension twice, causing initialization errors. Guard with a `std::sync::Once`.
- **Optional TOML file causing startup failure:** `Toml::file()` in figment silently ignores missing files. Do NOT use `Toml::file_exact()` which returns an error if the file doesn't exist.
- **Environment variable key mismatch:** Figment lowercases env var names after stripping the prefix. `MNEMONIC_DB_PATH` maps to field `db_path`. Field names in Config must use snake_case.
- **axum 0.7 route syntax:** axum 0.8 changed path parameters from `/:id` to `/{id}`. Use the new syntax — the old syntax compiles but produces a deprecation warning and may not work correctly.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Layered config (defaults + TOML + env) | Custom env var parsing + toml::from_str | figment 0.10.19 | Handles missing files, type coercion, prefix stripping, and merge order in ~5 lines |
| Async SQLite bridge | spawn_blocking wrappers | tokio-rusqlite | spawn_blocking doesn't guarantee serial execution; tokio-rusqlite uses dedicated thread + mpsc for ordering |
| WAL mode setup | Manual PRAGMA calls | execute_batch in connection init | Not hand-rolling — but must be done in the connection-open closure, not after |
| UUID v7 generation | Custom timestamp-based IDs | uuid crate v1.22 with `Uuid::new_v7(Timestamp::now(...))` | UUID v7 spec handles monotonicity, collisions, and byte ordering correctly |
| Extension auto-loading | dlopen / load_extension | sqlite3_auto_extension | dlopen requires .so file on disk; breaks single-binary distribution |

**Key insight:** The config and async-SQLite layers are the two places where hand-rolling causes the most downstream pain. Use figment and tokio-rusqlite exactly as documented.

---

## Common Pitfalls

### Pitfall 1: sqlite3_auto_extension Called Too Late

**What goes wrong:** If any `Connection::open` call happens before `sqlite3_auto_extension` is called (e.g., during test setup or early initialization), that connection won't have sqlite-vec loaded. Subsequent calls to `CREATE VIRTUAL TABLE ... USING vec0` fail with "no such module: vec0".

**Why it happens:** The auto-extension list is populated at the time of registration. Connections opened before registration are unaffected.

**How to avoid:** Call `register_sqlite_vec()` as literally the first thing in `main()`, wrapped in `std::sync::Once`. For tests, call it in a test-global `once_cell::sync::Lazy` initializer.

**Warning signs:** `no such module: vec0` error at runtime.

### Pitfall 2: figment Env Key Mapping Mismatch

**What goes wrong:** `MNEMONIC_DB_PATH` should map to the `db_path` field, but if the struct field is named `db_path` and figment lowercases + strips `MNEMONIC_`, it becomes `db_path` — this works. However, if the TOML file uses `db-path` (kebab-case) instead of `db_path` (snake_case), figment's normalization may not reconcile them.

**Why it happens:** TOML convention sometimes uses kebab-case for keys; Rust struct fields use snake_case; figment maps them.

**How to avoid:** Use snake_case for all TOML keys and all struct fields. Document the TOML format with a `mnemonic.toml.example` file.

**Warning signs:** Config extraction returns default values even when TOML or env var is set.

### Pitfall 3: WAL Mode Not Applied to Existing Database

**What goes wrong:** If `mnemonic.db` already exists in journal (non-WAL) mode and the server starts again, `PRAGMA journal_mode=WAL` succeeds but the existing file stays in its original format until a checkpoint. The application still works, but WAL benefits may not apply on the first run after migration.

**Why it happens:** SQLite journal mode changes are applied at the connection level on the next write.

**How to avoid:** Always execute `PRAGMA journal_mode=WAL` as the first statement when opening the connection. This is idempotent — no harm if already in WAL mode.

**Warning signs:** None on startup; only shows up under concurrent load testing.

### Pitfall 4: tokio-rusqlite Error Type Mismatch

**What goes wrong:** `conn.call()` returns `tokio_rusqlite::Result<T>`, but `tokio_rusqlite::Error` does not implement `std::error::Error` in all versions (it wraps a `rusqlite::Error` or `tokio` channel error). Using `?` directly in an `anyhow::Result` context may fail to compile.

**Why it happens:** The error type hierarchy for async DB wrappers can be complex.

**How to avoid:** Wrap `conn.call(...).await.map_err(|e| anyhow::anyhow!("{}", e))?` at the call site, or define a `From<tokio_rusqlite::Error> for MnemonicError` impl in error.rs.

**Warning signs:** Compile error "the trait `std::error::Error` is not implemented for `tokio_rusqlite::Error`".

### Pitfall 5: Single-File sqlite-vec Version Mismatch

**What goes wrong:** The sqlite-vec Rust crate (FFI bindings) and the underlying C extension must be compatible versions. Using a wildcard (`sqlite-vec = "*"`) picks up the latest crate, but if rusqlite's bundled SQLite version is incompatible with sqlite-vec's bundled C code, the extension fails to load.

**Why it happens:** sqlite-vec 0.1.x is actively developed and the C FFI surface evolves.

**How to avoid:** Pin `sqlite-vec = "0.1.7"` explicitly in Cargo.toml. Review release notes before bumping.

**Warning signs:** Segfault on startup or cryptic FFI error during extension initialization.

---

## Code Examples

Verified patterns from official sources:

### Complete main.rs Wire-Up

```rust
// Recommended main.rs structure for Phase 1
use anyhow::Result;

mod config;
mod db;
mod error;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Register sqlite-vec BEFORE any Connection::open
    db::register_sqlite_vec();

    // 2. Init tracing
    server::init_tracing();

    // 3. Load config (defaults -> TOML -> env vars)
    let config = config::load_config()?;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        port = config.port,
        db_path = %config.db_path,
        embedding_provider = %config.embedding_provider,
        "mnemonic starting"
    );

    // 4. Open DB and apply schema
    let conn = db::open(&config).await?;

    // 5. Start axum server
    let state = server::AppState {
        db: std::sync::Arc::new(conn),
        config: std::sync::Arc::new(config.clone()),
    };
    server::serve(&config, state).await?;

    Ok(())
}
```

### db.rs register + open pattern

```rust
// src/db.rs
use rusqlite::ffi::sqlite3_auto_extension;
use sqlite_vec::sqlite3_vec_init;
use std::sync::Once;
use tokio_rusqlite::Connection;

static SQLITE_VEC_REGISTERED: Once = Once::new();

pub fn register_sqlite_vec() {
    SQLITE_VEC_REGISTERED.call_once(|| {
        unsafe {
            sqlite3_auto_extension(Some(
                std::mem::transmute(sqlite3_vec_init as *const ()),
            ));
        }
    });
}

pub async fn open(config: &crate::config::Config) -> anyhow::Result<Connection> {
    let conn = Connection::open(&config.db_path).await
        .map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;

    conn.call(|c| {
        c.execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;

            CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                agent_id TEXT NOT NULL DEFAULT '',
                session_id TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '[]',
                embedding_model TEXT NOT NULL DEFAULT '',
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS vec_memories USING vec0(
                memory_id TEXT PRIMARY KEY,
                embedding float[384]
            );
        ")?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow::anyhow!("Schema initialization failed: {}", e))?;

    Ok(conn)
}
```

### uuid v7 generation

```rust
// Source: https://docs.rs/uuid/latest/uuid/
use uuid::{Uuid, Timestamp, NoContext};

fn new_memory_id() -> String {
    // Uuid::new_v7 requires a Timestamp; use now_v1 as the timestamp source
    let ts = Timestamp::now(NoContext);
    Uuid::new_v7(ts).to_string()
}
```

Note: `uuid::Timestamp::now(NoContext)` is the idiomatic way with uuid 1.x. The `v7` feature must be enabled.

### Minimal Cargo.toml (Phase 1)

```toml
[package]
name = "mnemonic"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
axum = "0.8"
rusqlite = { version = "0.39", features = ["bundled"] }
sqlite-vec = "0.1.7"
tokio-rusqlite = "0.7"
figment = { version = "0.10", features = ["toml", "env"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
thiserror = "2"
anyhow = "1"
uuid = { version = "1", features = ["v7"] }
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `axum::Server::bind(...).serve(...)` | `axum::serve(TcpListener, Router)` | axum 0.8 (Jan 2025) | Old pattern removed; new pattern required |
| `/:param` route syntax | `/{param}` route syntax | axum 0.8 | Old syntax deprecated; use new brace syntax |
| sqlite-vss | sqlite-vec | 2023/2024 | sqlite-vss archived; sqlite-vec is the successor |
| `spawn_blocking` for rusqlite | tokio-rusqlite | 2022+ | tokio-rusqlite provides proper ordering guarantees |
| `config` crate | figment | ongoing | figment has cleaner merge semantics; both still used |

**Deprecated/outdated:**
- `sqlite-vss`: Archived by author; `sqlite-vec` is the replacement
- axum 0.7 `Server::bind` pattern: Removed in 0.8
- `#[async_trait]` on axum extractors: No longer needed in 0.8 (Rust RPIT in traits)

---

## Open Questions

1. **uuid v7 Timestamp constructor API**
   - What we know: `uuid::Uuid::new_v7()` requires a `Timestamp` argument; `Timestamp::now(NoContext)` is documented
   - What's unclear: The exact import path may differ between uuid 1.x minor versions; `NoContext` may need `use uuid::timestamp::context::NoContext` or `use uuid::NoContext`
   - Recommendation: Verify with `cargo doc --open` after adding the dependency; fallback to `uuid7` crate if API is awkward

2. **figment missing TOML file behavior**
   - What we know: `Toml::file()` is documented to silently ignore missing files
   - What's unclear: Whether figment 0.10.19 changed this behavior vs. older docs
   - Recommendation: Write a test that loads config with no mnemonic.toml present; assert defaults are returned

3. **sqlite-vec embedding column size for Phase 1**
   - What we know: all-MiniLM-L6-v2 produces 384-dimensional vectors (Phase 2 decision)
   - What's unclear: Whether `float[384]` in the virtual table is alterable later without a DROP/CREATE cycle
   - Recommendation: Hardcode `float[384]` now since all-MiniLM-L6-v2 is locked; document this as a migration cost if model changes

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness + `#[tokio::test]` (tokio 1.50) |
| Config file | none required — `cargo test` discovers tests automatically |
| Quick run command | `cargo test` |
| Full suite command | `cargo test -- --test-threads=1` (serialize DB tests) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| STOR-01 | `memories` table and `vec_memories` virtual table exist after `db::open()` | integration | `cargo test test_schema_created` | ❌ Wave 0 |
| STOR-02 | WAL mode is active after open — `PRAGMA journal_mode` returns "wal" | integration | `cargo test test_wal_mode` | ❌ Wave 0 |
| STOR-03 | `db::open()` returns without blocking the tokio runtime — verified by running in `#[tokio::test]` | integration | `cargo test test_db_open_async` | ❌ Wave 0 |
| STOR-04 | `embedding_model` column exists in `memories` table schema | integration | `cargo test test_embedding_model_column` | ❌ Wave 0 |
| CONF-01 | `config::load_config()` with no env vars returns port 8080, db_path "./mnemonic.db", provider "local" | unit | `cargo test test_config_defaults` | ❌ Wave 0 |
| CONF-02 | `MNEMONIC_PORT=9090` env var results in `config.port == 9090` | unit | `cargo test test_config_env_override` | ❌ Wave 0 |
| CONF-03 | Writing a `mnemonic.toml` with `port = 7070` results in `config.port == 7070` (when no env override) | unit | `cargo test test_config_toml` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test`
- **Per wave merge:** `cargo test -- --test-threads=1`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/integration.rs` — covers STOR-01, STOR-02, STOR-03, STOR-04 with in-memory DB
- [ ] `src/config.rs` unit test module — covers CONF-01, CONF-02, CONF-03 using figment `Jail` for env isolation
- [ ] `register_sqlite_vec()` called in test setup via `std::sync::Once` (same guard as production)

---

## Sources

### Primary (HIGH confidence)
- `cargo search` against crates.io (2026-03-19) — all version numbers verified
- [sqlite-vec Rust guide](https://alexgarcia.xyz/sqlite-vec/rust.html) — extension registration pattern
- [sqlite-vec KNN query docs](https://alexgarcia.xyz/sqlite-vec/features/knn.html) — CREATE VIRTUAL TABLE and KNN query syntax
- [tokio-rusqlite docs.rs](https://docs.rs/tokio-rusqlite/latest/tokio_rusqlite/) — Connection::call() API and examples
- [axum docs.rs](https://docs.rs/axum/latest/axum/) — Router, TcpListener, serve(), State extractor
- [figment docs.rs](https://docs.rs/figment/latest/figment/) — Env::prefixed(), Toml::file(), merge order
- [tracing-subscriber docs.rs](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/) — EnvFilter, fmt::layer().pretty()

### Secondary (MEDIUM confidence)
- [axum 0.8 announcement](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0) — breaking changes (route syntax, #[async_trait] removal)
- [figment guide (generalistprogrammer.com)](https://generalistprogrammer.com/tutorials/figment-rust-crate-guide) — version 0.10.19 confirmed

### Tertiary (LOW confidence)
- WebSearch results for tokio-rusqlite error type behavior — needs validation with actual compile attempt

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions confirmed via `cargo search` against live registry (2026-03-19)
- Architecture: HIGH — all patterns sourced from official documentation
- Pitfalls: MEDIUM — most derived from official docs + ecosystem knowledge; tokio-rusqlite error type pitfall is LOW (unconfirmed compile behavior)
- sqlite-vec virtual table schema: HIGH — verified against official KNN docs

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (30 days; sqlite-vec is active but stable at 0.1.x)
