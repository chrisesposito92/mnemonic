# Phase 10: Auth Schema Foundation - Research

**Researched:** 2026-03-20
**Domain:** Rust / SQLite idempotent schema migration, axum error handling, Rust module organization
**Confidence:** HIGH — primary sources are the existing codebase (direct inspection) and project research documents already written during v1.2 planning.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Schema Columns**
- D-01: Use BLAKE3 (not SHA-256) for key hashing — `blake3` crate, 32-byte output, hex-encoded for storage. Faster, pure Rust, no OpenSSL.
- D-02: Use `constant_time_eq` crate (not `subtle`) for hash comparison — `constant_time_eq_32()` on `[u8; 32]`.
- D-03: Display ID column stores the first 8 hex characters of BLAKE3(raw_key) — NOT a prefix of the raw key itself. Prevents enumeration (Auth Pitfall 7).
- D-04: `agent_id TEXT` with NULL = wildcard (any agent). No sentinel strings.
- D-05: Soft delete via `revoked_at DATETIME` — NULL = active. Preserves audit trail. Idempotent if revoked twice.
- D-06: Schema uses `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` — idempotent migration, safe on existing v1.1 databases.

**401 Error Response**
- D-07: Add `ApiError::Unauthorized(String)` variant — separate from BadRequest/NotFound/Internal. Maps to HTTP 401.
- D-08: 401 response body is detailed: `{ "error": "unauthorized", "auth_mode": "active", "hint": "Provide Authorization: Bearer mnk_..." }`. Helps agent developers debug.
- D-09: No trace/request ID in error responses — keep it simple, consistent with existing patterns.
- D-10: Only add `Unauthorized` variant now. `Forbidden` (403) deferred to Phase 13 when scope enforcement is wired.

**Startup Auth Log**
- D-11: Log auth mode at startup (INFO level) with actionable hint: `"Auth: OPEN (no keys) — run 'mnemonic keys create' to enable"` or `"Auth: ACTIVE (3 keys)"`.
- D-12: Check active key count during server init via DB query — log once at startup.
- D-13: INFO level for both open and active modes — consistent with existing startup log messages (bind address, etc.).

**Module Skeleton Scope**
- D-14: `auth.rs` contains `AuthContext`, `ApiKey` structs with real fields, and `KeyService` struct with method signatures returning `todo!()`. Enough scaffolding for Phase 11 to fill in.
- D-15: Crypto helpers (BLAKE3 hashing, token generation) deferred to Phase 11 — not part of foundation.
- D-16: Add `key_service: Arc<KeyService>` to `AppState` in Phase 10. Avoids breaking AppState construction in later phases.
- D-17: Implement real `KeyService::count_active_keys()` function — queries DB, used by startup log. First real auth function, tested against empty DB returning 0.

### Claude's Discretion
- Exact column ordering in DDL
- Whether to add `last_used_at DATETIME` column now or defer to Phase 11
- Index strategy beyond `hashed_key` (e.g., whether to index `agent_id` on api_keys)
- Exact wording of startup log messages

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| INFRA-01 | `api_keys` table is created via idempotent SQLite migration on startup | Existing `CREATE TABLE IF NOT EXISTS` pattern in `db.rs::open()` is the direct template; `execute_batch` wrapping is already established |
| INFRA-03 | Server startup log announces whether running in open or authenticated mode | `KeyService::count_active_keys()` (D-17) is the real function that powers the startup log; `tracing::info!` at INFO level matches all existing startup messages |
</phase_requirements>

---

## Summary

Phase 10 is a pure foundation phase — no HTTP middleware, no crypto, no key generation. Its job is to create the exact four artifacts that every downstream phase will build on: (1) the `api_keys` DDL block in `db.rs`, (2) the `ApiError::Unauthorized` variant in `error.rs`, (3) the `auth.rs` module skeleton with real structs and one real function, and (4) the startup auth-mode log in `main.rs`.

All four modifications are additive. No existing code is deleted or restructured. The riskiest touch point is `db.rs::open()` — the new DDL block must land inside the existing `execute_batch` call and be tested against a real v1.1-schema in-memory database. The second riskiest is `AppState` in `server.rs`, which gains a `key_service` field that must be wired through `main.rs` and through every existing test that constructs `AppState` directly.

The research below confirms that every pattern needed for this phase already exists in the codebase. No new architectural patterns are introduced — only additions following established conventions.

**Primary recommendation:** Follow the build order from ARCHITECTURE.md (db.rs → error.rs → auth.rs → server.rs → main.rs), write a migration test against an in-memory DB, and verify zero compiler warnings before marking the phase done.

---

## Standard Stack

### Core (Phase 10 scope — no new dependencies required)

| Library | Current Version | Purpose | Notes |
|---------|----------------|---------|-------|
| `tokio-rusqlite` | 0.7 | Async SQLite via `conn.call()` | All DB work in Phase 10 uses this pattern |
| `rusqlite` | 0.37 | SQLite underlying driver | `execute_batch` for DDL in `open()` |
| `thiserror` | 2 | Derive macro for error enums | `Unauthorized` variant uses same derive |
| `axum` | 0.8 | HTTP framework | `IntoResponse` impl for new variant |
| `tracing` | 0.1 | Structured logging | `tracing::info!` for startup log |
| `serde_json` | 1 | JSON response bodies | `json!()` macro for 401 body |

### New Dependencies for Phase 10

None. BLAKE3 hashing (D-01) and `constant_time_eq` (D-02) are deferred to Phase 11. The `KeyService::count_active_keys()` function uses only `tokio-rusqlite` (already present).

### Dependencies Required Later (not Phase 10)

| Library | Purpose | Phase |
|---------|---------|-------|
| `blake3` | Key hashing (replaces SHA-256 from ARCHITECTURE.md) | Phase 11 |
| `constant_time_eq` | Constant-time hash comparison | Phase 11 |
| `rand` | Random token generation | Phase 11 |
| `base64` | Token encoding | Phase 11 |

**Version verification:** No new Cargo.toml additions in Phase 10. All libraries above are already in Cargo.toml.

---

## Architecture Patterns

### Pattern 1: Idempotent DDL in `execute_batch`

**What:** Add the `api_keys` DDL as a new block inside the existing `conn.call()` / `execute_batch` call in `db.rs::open()`. This is the same pattern used for `memories`, `compact_runs`, and all their indexes.

**When to use:** Every schema addition in this codebase. The `CREATE TABLE IF NOT EXISTS` + `CREATE INDEX IF NOT EXISTS` idiom means the migration is safe to run on any existing database version.

**The DDL block to add** (per decisions D-01 through D-06, adapting ARCHITECTURE.md schema to use `display_id` instead of `prefix` and `hashed_key` instead of `key_hash`):

```sql
-- v1.2: API key authentication
CREATE TABLE IF NOT EXISTS api_keys (
    id           TEXT PRIMARY KEY,          -- UUID v7
    name         TEXT NOT NULL DEFAULT '',  -- human label, e.g. "agent-prod"
    display_id   TEXT NOT NULL,             -- first 8 hex chars of BLAKE3(raw_key) — NOT a key prefix
    hashed_key   TEXT NOT NULL UNIQUE,      -- hex(BLAKE3(raw_key)), 64 chars
    agent_id     TEXT,                      -- NULL = wildcard; non-NULL = scoped to this agent
    created_at   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revoked_at   DATETIME                   -- NULL = active; non-NULL = revoked (soft delete)
);

CREATE INDEX IF NOT EXISTS idx_api_keys_hashed_key ON api_keys(hashed_key);
CREATE INDEX IF NOT EXISTS idx_api_keys_agent_id   ON api_keys(agent_id);
```

**Claude's discretion items resolved here:**
- `last_used_at` column: **defer to Phase 11**. The column would be updated by the validate function, which lives in Phase 11. Adding it now without the write path creates dead schema.
- Agent_id index: **add it** — Phase 12 middleware and Phase 13 scope queries will filter by agent_id. Zero cost now, required later.
- Column ordering: id, name, display_id, hashed_key, agent_id, created_at, revoked_at (identity fields first, then the security-critical hash, then scope, then timestamps).

**Integration point:** Insert after the `compact_runs` block and its index, before the `Ok(())`. The existing ALTER TABLE migration for `source_ids` follows the `execute_batch` call and is unaffected.

```rust
// Source: src/db.rs — existing execute_batch pattern
conn.call(|c| -> Result<(), rusqlite::Error> {
    c.execute_batch("
        /* ... existing DDL ... */

        -- v1.2: API key authentication
        CREATE TABLE IF NOT EXISTS api_keys (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL DEFAULT '',
            display_id  TEXT NOT NULL,
            hashed_key  TEXT NOT NULL UNIQUE,
            agent_id    TEXT,
            created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            revoked_at  DATETIME
        );
        CREATE INDEX IF NOT EXISTS idx_api_keys_hashed_key ON api_keys(hashed_key);
        CREATE INDEX IF NOT EXISTS idx_api_keys_agent_id   ON api_keys(agent_id);
    ")?;
    /* ... existing ALTER TABLE ... */
    Ok(())
})
```

### Pattern 2: Adding a Variant to `ApiError`

**What:** `ApiError` in `error.rs` is a `thiserror`-derived enum with an `IntoResponse` impl. Adding `Unauthorized(String)` follows the identical pattern as the existing `BadRequest(String)` variant.

**The variant and its match arm:**

```rust
// Source: src/error.rs — extend existing ApiError enum
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error("unauthorized: {0}")]
    Unauthorized(String),              // NEW — D-07
    #[error("internal error: {0}")]
    Internal(#[from] MnemonicError),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            ApiError::BadRequest(msg) =>
                (axum::http::StatusCode::BAD_REQUEST,
                 serde_json::json!({"error": msg})),
            ApiError::NotFound =>
                (axum::http::StatusCode::NOT_FOUND,
                 serde_json::json!({"error": "not found"})),
            ApiError::Unauthorized(msg) =>               // NEW — D-07, D-08
                (axum::http::StatusCode::UNAUTHORIZED,
                 serde_json::json!({
                     "error": "unauthorized",
                     "auth_mode": "active",
                     "hint": "Provide Authorization: Bearer mnk_..."
                 })),
            ApiError::Internal(e) => {
                tracing::error!(error = %e, "internal server error");
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                 serde_json::json!({"error": "internal server error"}))
            }
        };
        (status, axum::Json(body)).into_response()
    }
}
```

**Note on D-08:** The 401 body is hardcoded (not derived from the variant's String payload). The `msg` field in `Unauthorized(msg)` is used for the `tracing` context if desired, but the HTTP response body is always the structured `auth_mode` + `hint` form per D-08. The variant still carries a String for internal logging context, consistent with `BadRequest(String)`.

**Note on D-09:** No trace/request ID added. Consistent with existing patterns.

**Note on D-10:** `Forbidden` variant is NOT added in this phase.

### Pattern 3: `auth.rs` Module Skeleton

**What:** A new `src/auth.rs` file with real struct definitions and one real function (`KeyService::count_active_keys()`). All other `KeyService` methods return `todo!()` — they exist as stubs for Phase 11 to fill in.

**Struct designs** (per D-14 and ARCHITECTURE.md):

```rust
// Source: .planning/research/ARCHITECTURE.md — adapted per D-01, D-03, D-17

use std::sync::Arc;
use tokio_rusqlite::Connection;

/// A row from the api_keys table.
#[derive(Debug, Clone)]
pub struct ApiKey {
    pub id: String,               // UUID v7
    pub name: String,             // human label
    pub display_id: String,       // first 8 hex chars of BLAKE3(raw_key) — never a key prefix
    pub agent_id: Option<String>, // None = wildcard, Some = scoped
    pub created_at: String,       // ISO 8601 datetime
    pub revoked_at: Option<String>,
}

/// Per-request authentication result, injected into request extensions by the auth middleware.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub key_id: String,
    pub allowed_agent_id: Option<String>, // None = wildcard key
}

/// Business logic for API key management.
pub struct KeyService {
    conn: Arc<Connection>,
}

impl KeyService {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    /// Counts active (non-revoked) keys in the database.
    /// Returns 0 if the table is empty (open mode).
    /// Used by the startup log to determine auth mode.
    pub async fn count_active_keys(&self) -> Result<i64, crate::error::DbError> {
        self.conn.call(|c| -> Result<i64, rusqlite::Error> {
            let count: i64 = c.query_row(
                "SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL",
                [],
                |row| row.get(0),
            )?;
            Ok(count)
        })
        .await
        .map_err(crate::error::DbError::from)
    }

    // Phase 11 stubs — return todo!() to compile but not be callable
    pub async fn create(&self, _name: String, _agent_id: Option<String>) -> ! { todo!() }
    pub async fn list(&self) -> ! { todo!() }
    pub async fn revoke(&self, _id: &str) -> ! { todo!() }
    pub async fn validate(&self, _raw_token: &str) -> ! { todo!() }
}
```

**Note on stub signatures:** Using `-> !` (the never type) is one option, but it generates no warnings even when unimplemented. A better approach for compilation with zero warnings: use `unimplemented!()` inside a body with a concrete return type placeholder. However, since Phase 11 will replace these entirely, a clean approach is to simply leave the stubs with `todo!()` bodies and concrete placeholder return types (e.g., `Result<(), crate::error::DbError>`). The compiler won't warn about unreachable code at `todo!()` call sites.

**Recommended stub pattern:**

```rust
pub async fn create(
    &self,
    _name: String,
    _agent_id: Option<String>,
) -> Result<(ApiKey, String), crate::error::DbError> {
    todo!("Phase 11: KeyService::create")
}

pub async fn list(&self) -> Result<Vec<ApiKey>, crate::error::DbError> {
    todo!("Phase 11: KeyService::list")
}

pub async fn revoke(&self, _id: &str) -> Result<(), crate::error::DbError> {
    todo!("Phase 11: KeyService::revoke")
}

pub async fn validate(&self, _raw_token: &str) -> Result<AuthContext, crate::error::DbError> {
    todo!("Phase 11: KeyService::validate")
}
```

Using `todo!("Phase 11: ...")` descriptive messages makes it unmistakable in a panic what needs implementing.

### Pattern 4: `AppState` Addition and Startup Log

**What:** Add `key_service: Arc<KeyService>` to `AppState` in `server.rs`, construct it in `main.rs`, and call `count_active_keys()` once after DB open to log the auth mode.

**`AppState` change:**

```rust
// Source: src/server.rs — additive field
#[derive(Clone)]
pub struct AppState {
    pub service: std::sync::Arc<crate::service::MemoryService>,
    pub compaction: std::sync::Arc<crate::compaction::CompactionService>,
    pub key_service: std::sync::Arc<crate::auth::KeyService>,  // NEW — D-16
}
```

**`main.rs` additions** (after DB open, before embedding load):

```rust
// Source: src/main.rs — after db::open(), following D-11, D-12, D-13
let db_arc = std::sync::Arc::new(conn);
let key_service = std::sync::Arc::new(
    mnemonic::auth::KeyService::new(db_arc.clone())   // or crate::auth::KeyService::new
);

// Startup auth-mode log (D-11, D-12, D-13)
match key_service.count_active_keys().await {
    Ok(0) => tracing::info!(
        "Auth: OPEN (no keys) — run 'mnemonic keys create' to enable"
    ),
    Ok(n) => tracing::info!(
        keys = n,
        "Auth: ACTIVE ({n} keys)"
    ),
    Err(e) => tracing::warn!(
        error = %e,
        "Auth: could not determine mode (DB error)"
    ),
}
```

**Note:** The `AppState` now needs `key_service` field to be populated. This means the `AppState { service, compaction }` literal in `main.rs` and in test helpers (`tests/integration.rs`) must be updated to include `key_service`. This is a compile-time enforcement — the compiler will flag every construction site.

### Pattern 5: `lib.rs` Module Declaration

**What:** Add `pub mod auth;` to `src/lib.rs`. Single line, follows existing pattern.

```rust
// src/lib.rs — add one line
pub mod auth;          // NEW
pub mod compaction;
pub mod config;
pub mod db;
pub mod embedding;
pub mod error;
pub mod server;
pub mod service;
pub mod summarization;
```

### Recommended Project Structure (Phase 10 delta)

```
src/
├── main.rs          MODIFIED: construct KeyService, add startup auth log
├── server.rs        MODIFIED: add key_service field to AppState
├── db.rs            MODIFIED: add api_keys DDL block inside execute_batch
├── error.rs         MODIFIED: add Unauthorized variant to ApiError
├── lib.rs           MODIFIED: add pub mod auth
├── auth.rs          NEW: KeyService, ApiKey, AuthContext, count_active_keys()
├── config.rs        No change
├── service.rs       No change
├── embedding.rs     No change
├── compaction.rs    No change
└── summarization.rs No change
```

### Anti-Patterns to Avoid

- **Using `ALTER TABLE` for the new table:** `ALTER TABLE` is for adding columns to existing tables. New tables use `CREATE TABLE IF NOT EXISTS`. The existing ALTER TABLE migration for `source_ids` is a model for column additions, NOT for table creation.
- **Placing the DDL outside `execute_batch`:** The `execute_batch` is a single atomic SQL block. New tables must live inside it, not in a separate `execute_batch` call, to maintain the single-transaction initialization guarantee.
- **Adding `Forbidden` to `ApiError` now:** D-10 explicitly defers this to Phase 13. Adding it prematurely creates a variant with no callers, causing an unused-variant warning (Rust may warn on this with `dead_code`).
- **Implementing crypto in `auth.rs` now:** D-15 defers BLAKE3 hashing and token generation to Phase 11. The `auth.rs` skeleton should compile cleanly without `blake3` or `constant_time_eq` in Cargo.toml.
- **Querying `count_active_keys()` inside `build_router()`:** The startup log belongs in `main.rs`, not in router construction. Router construction should remain pure (no I/O).
- **Forgetting to update `AppState` construction in `tests/integration.rs`:** Adding `key_service` to `AppState` is a breaking struct change. `tests/integration.rs` line 9 imports `AppState` and constructs it. Every test that builds `AppState` directly will fail to compile until updated.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Idempotent table creation | Custom migration version tracking | `CREATE TABLE IF NOT EXISTS` | Already established pattern in this codebase; zero additional complexity |
| Error enum HTTP mapping | Custom error → status code translation | `thiserror` + `axum::response::IntoResponse` | Already established; `ApiError::Unauthorized` is the 4th variant following the existing pattern |
| Async SQLite in tokio context | Direct rusqlite calls in async functions | `conn.call(|c| ...)` from `tokio-rusqlite` | Established pattern; direct rusqlite in async blocks executor thread |
| Module declarations | Procedural macros | `pub mod auth;` in `lib.rs` | Single line, no magic |

**Key insight:** This phase has zero novel implementation challenges. Every sub-problem has been solved in the existing codebase. The work is mechanical addition following established patterns.

---

## Common Pitfalls

### Pitfall 1: `AppState` is a Struct Literal — Every Construction Site Breaks

**What goes wrong:** Rust struct literals must include all fields. Adding `key_service` to `AppState` means every location that writes `AppState { service, compaction }` will fail to compile. There are two known construction sites: `main.rs` (lines 121-124) and `tests/integration.rs` (the test helper that builds `AppState`).

**Why it happens:** Developers modify the struct definition and the `main.rs` construction, then forget the test file.

**How to avoid:** After modifying `AppState`, run `cargo check` immediately and fix all compiler errors before proceeding. The compiler is exhaustive.

**Warning signs:** `cargo check` errors mentioning "missing field `key_service`" in `tests/integration.rs` or any test helper.

### Pitfall 2: DDL Order Matters Inside `execute_batch`

**What goes wrong:** `execute_batch` is parsed as a sequence of SQL statements. If a DDL statement fails (e.g., due to a syntax error in the new block), the entire batch fails and the server won't start. SQLite is strict about semicolons between statements in batch mode.

**Why it happens:** Copy-paste errors in SQL strings, missing semicolons, or typos in column names.

**How to avoid:** Test the new DDL block in isolation first (write a unit test that opens `:memory:` and runs just the api_keys DDL). Then integrate into the full `execute_batch`.

**Warning signs:** `db::open()` returning `DbError::Schema(...)` on startup.

### Pitfall 3: Zero-Warning Requirement and Dead Code

**What goes wrong:** Adding struct fields, enum variants, or functions that are never used in Phase 10 scope will generate `dead_code` warnings. The phase success criterion requires "zero warnings."

**Why it happens:** `KeyService::create/list/revoke/validate` stubs are defined but never called in Phase 10. `AuthContext` struct is defined but not yet used in any handler. `ApiKey` struct is defined but no query returns it.

**How to avoid:** Annotate stubs with `#[allow(dead_code)]` where appropriate, OR accept that `todo!()` bodies will only produce warnings if the function itself is dead. In practice, Rust warns about dead code at the item level (struct fields, functions, variants) — not about `todo!()` bodies.

The cleanest solution: add `#[allow(dead_code)]` to `src/auth.rs` at the module level for Phase 10 only, with a comment: `// Phase 10 stubs — dead_code allowed until Phase 11 fills in implementations`. Remove the allow attribute in Phase 11.

Alternatively, use `_` prefix on parameters in stubs (e.g., `_name`, `_agent_id`) to suppress unused-parameter warnings.

**Warning signs:** `cargo build` output showing `warning: dead code: ...` for `AuthContext` or stub methods.

### Pitfall 4: Migration Test Must Use In-Memory DB, Not a File

**What goes wrong:** If the migration test uses a file-based DB, it may leave state between test runs (especially if tests run in parallel). Tests that check "table exists" are reliable only against a fresh `:memory:` database.

**Why it happens:** Developers copy the integration test pattern from `test_config()` which already uses `":memory:"` — this is correct but must be verified.

**How to avoid:** Confirm the migration test uses `db_path: ":memory:".to_string()`. The existing `test_config()` in `tests/integration.rs` already does this — new migration tests should use the same helper.

### Pitfall 5: `main.rs` Uses Module Paths, Not `crate::` in binary crates

**What goes wrong:** In `main.rs` (a binary), module paths use bare names (`auth::KeyService`), not `crate::auth::KeyService`. In `lib.rs` and other library modules, `crate::` is correct. In `main.rs`, since it's a binary root that declares modules with `mod auth;`, the path is just `auth::KeyService`.

**Why it happens:** Confusion between library crate path semantics (`crate::`) and binary module path semantics (bare module name).

**How to avoid:** In `main.rs`, use `auth::KeyService::new(...)`. In `server.rs` (which is part of the library crate), use `crate::auth::KeyService`. Check existing patterns: `main.rs` line 101 uses `service::MemoryService::new(...)` (bare), while `server.rs` uses `crate::service::MemoryService` in the struct definition.

**Warning signs:** `cargo check` error: "use of undeclared crate or module `crate`" in `main.rs`.

---

## Code Examples

### Example 1: Migration Test Pattern (from existing tests)

```rust
// Source: tests/integration.rs — test_schema_created() pattern
#[tokio::test]
async fn test_api_keys_table_created() {
    setup();
    let config = test_config(); // uses ":memory:"
    let conn = mnemonic::db::open(&config).await.unwrap();

    let table_exists = conn
        .call(|c| -> Result<bool, rusqlite::Error> {
            let mut stmt = c.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='api_keys'"
            )?;
            let mut rows = stmt.query([])?;
            Ok(rows.next()?.is_some())
        })
        .await
        .unwrap();

    assert!(table_exists, "api_keys table should exist after db::open()");
}

#[tokio::test]
async fn test_count_active_keys_empty_db() {
    setup();
    let config = test_config();
    let conn = std::sync::Arc::new(
        mnemonic::db::open(&config).await.unwrap()
    );
    let key_service = mnemonic::auth::KeyService::new(conn);
    let count = key_service.count_active_keys().await.unwrap();
    assert_eq!(count, 0, "empty DB should report 0 active keys");
}
```

### Example 2: Existing `IntoResponse` Pattern (from error.rs)

The current `IntoResponse` impl for reference when adding `Unauthorized`:

```rust
// Source: src/error.rs — existing IntoResponse impl (lines 89-101)
impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::BadRequest(msg) => (axum::http::StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound => (axum::http::StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::Internal(e) => {
                tracing::error!(error = %e, "internal server error");
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal server error".to_string())
            }
        };
        (status, axum::Json(serde_json::json!({"error": message}))).into_response()
    }
}
```

The `Unauthorized` variant needs a different body structure than the existing pattern (D-08 requires `auth_mode` and `hint` fields). This means the `Unauthorized` match arm cannot use the same `let (status, message)` path — it must produce a different JSON shape. Two approaches:

**Option A:** Restructure the entire `IntoResponse` to return `Json<Value>` directly in each arm (breaking the existing single variable approach). More verbose, cleaner type.

**Option B:** Keep the existing pattern for existing variants, add `Unauthorized` as a special case early in the match that returns directly:

```rust
impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        // Unauthorized has a richer body than the simple {"error": "..."} pattern
        if let ApiError::Unauthorized(_) = &self {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({
                    "error": "unauthorized",
                    "auth_mode": "active",
                    "hint": "Provide Authorization: Bearer mnk_..."
                })),
            ).into_response();
        }

        let (status, message) = match &self {
            ApiError::BadRequest(msg) => (axum::http::StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound => (axum::http::StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::Internal(e) => { /* ... */ }
            ApiError::Unauthorized(_) => unreachable!(), // handled above
        };
        (status, axum::Json(serde_json::json!({"error": message}))).into_response()
    }
}
```

**Recommendation:** Option A — refactor `IntoResponse` to handle each variant fully and produce its own body. The existing two-step `(status, message)` pattern is a premature abstraction that doesn't accommodate the richer body needed for `Unauthorized`. A clean refactor now prevents ugly workarounds in Phase 12+.

### Example 3: `conn.call()` Pattern for `count_active_keys`

```rust
// Source: pattern from src/service.rs — conn.call() closure
pub async fn count_active_keys(&self) -> Result<i64, crate::error::DbError> {
    self.conn
        .call(|c| -> Result<i64, rusqlite::Error> {
            c.query_row(
                "SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL",
                [],
                |row| row.get(0),
            )
        })
        .await
        .map_err(|e| crate::error::DbError::Query(e.to_string()))
}
```

Note: `tokio_rusqlite::Error` implements `Into<DbError>` via the `From` impl in `error.rs` (line 31-33), so `.map_err(crate::error::DbError::from)` works if calling `.await` returns `Result<_, tokio_rusqlite::Error>`. Verify the exact error type — `conn.call()` returns `Result<T, tokio_rusqlite::Error>`, so the conversion is correct.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test runner (`cargo test`) |
| Config file | None — `cargo test` is the runner |
| Quick run command | `cargo test --test integration -- test_api_keys` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| INFRA-01 | `api_keys` table exists after `db::open()` on v1.1 DB | integration | `cargo test --test integration -- test_api_keys_table_created` | ❌ Wave 0 |
| INFRA-01 | `api_keys` table is idempotent (run `open()` twice, no error) | integration | `cargo test --test integration -- test_api_keys_migration_idempotent` | ❌ Wave 0 |
| INFRA-03 | `count_active_keys()` returns 0 on empty DB | unit/integration | `cargo test --test integration -- test_count_active_keys_empty_db` | ❌ Wave 0 |
| D-07/D-08 | `ApiError::Unauthorized(_)` serializes to 401 with `auth_mode` + `hint` fields | unit | `cargo test -- test_unauthorized_response_shape` | ❌ Wave 0 |
| D-04 (compile) | Project compiles with zero warnings | build | `cargo build 2>&1 \| grep -c "^warning"` | n/a |

### Sampling Rate

- **Per task commit:** `cargo check` (fast, no model load)
- **Per wave merge:** `cargo test --test integration -- test_api_keys` (integration tests only, skip embedding)
- **Phase gate:** `cargo test` full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/integration.rs` — add `test_api_keys_table_created`, `test_api_keys_migration_idempotent`, `test_count_active_keys_empty_db`
- [ ] Unit test for `ApiError::Unauthorized` response shape — can live in `src/error.rs` as `#[cfg(test)]` module or in integration test file

*(Existing test infrastructure in `tests/integration.rs` and `tests/error_types.rs` covers the framework — no new test file setup needed)*

---

## State of the Art

| Old Approach (ARCHITECTURE.md) | Phase 10 Actual | Reason for Difference |
|-------------------------------|-----------------|----------------------|
| `prefix TEXT` (first N chars of raw key) | `display_id TEXT` (first 8 hex chars of BLAKE3(raw_key)) | D-03: hash-derived ID prevents enumeration (Auth Pitfall 7) |
| `hashed_key` using SHA-256 | `hashed_key` using BLAKE3 | D-01: faster, pure Rust, no OpenSSL dependency |
| `subtle::ConstantTimeEq` | `constant_time_eq::constant_time_eq_32()` | D-02: project decision (both are correct) |

**Deprecated/outdated in this project context:**
- The `prefix` column name from ARCHITECTURE.md §SQLite Schema: superseded by `display_id` per D-03.
- SHA-256 references in ARCHITECTURE.md §Anti-Patterns: superseded by BLAKE3 per D-01.

---

## Open Questions

1. **Zero-warning enforcement for `auth.rs` stubs**
   - What we know: `todo!()` stubs on public methods with no callers will likely trigger `dead_code` warnings in Rust.
   - What's unclear: Whether `#[allow(dead_code)]` at module level or per-item is the cleaner approach.
   - Recommendation: Add `#![allow(dead_code)]` as the first line of `auth.rs` for Phase 10, with a comment `// Stubs implemented in Phase 11 — remove this allow when complete`. The planner should include removing this attribute as a Phase 11 task.

2. **`AppState` construction in integration tests**
   - What we know: `tests/integration.rs` constructs `AppState` directly (line 9 import, and in test helpers). Adding `key_service` will break compilation.
   - What's unclear: Exactly which tests construct `AppState` — need to check the full test file.
   - Recommendation: The planner should include a task to update all `AppState { service, compaction }` literals in test files. A `cargo check` after modifying `server.rs` will identify all sites.

3. **`count_active_keys` error handling in startup log**
   - What we know: D-12 says "check active key count during server init via DB query." D-11/D-13 describe the log message format.
   - What's unclear: What to do if `count_active_keys()` fails (DB error at startup).
   - Recommendation: Treat a DB error as non-fatal for auth logging — log a warning but continue server startup. See Code Example in Pattern 4 above.

---

## Sources

### Primary (HIGH confidence)

- `src/db.rs` (direct inspection) — idempotent DDL pattern, `execute_batch` structure, ALTER TABLE error-swallowing
- `src/error.rs` (direct inspection) — `ApiError` enum, `IntoResponse` impl, `thiserror` usage
- `src/server.rs` (direct inspection) — `AppState` struct fields, `build_router()` structure
- `src/lib.rs` (direct inspection) — module declaration pattern
- `src/main.rs` (direct inspection) — `AppState` construction, startup log sequence
- `tests/integration.rs` (direct inspection) — test patterns for schema validation, `test_config()`, `setup()`
- `.planning/phases/10-auth-schema-foundation/10-CONTEXT.md` — locked decisions D-01 through D-17
- `.planning/research/ARCHITECTURE.md` — reference DDL, component responsibilities, anti-patterns
- `.planning/research/PITFALLS.md` §Auth Pitfalls 1-10 — security requirements driving decisions

### Secondary (MEDIUM confidence)

- `.planning/REQUIREMENTS.md` — INFRA-01, INFRA-03 requirement text
- `.planning/ROADMAP.md` §Phase 10 — success criteria

### Tertiary (LOW confidence)

- None — all findings verified against primary sources (existing codebase).

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies already in Cargo.toml; no new additions in Phase 10
- Architecture: HIGH — all patterns directly present in existing codebase
- Pitfalls: HIGH — verified against direct codebase inspection; pitfalls derived from project's own research documents

**Research date:** 2026-03-20
**Valid until:** 2026-06-20 (stable codebase, no external API dependencies in Phase 10)
