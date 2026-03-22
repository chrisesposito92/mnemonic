# Phase 29: StorageBackend Routing Fix - Research

**Researched:** 2026-03-22
**Domain:** Rust CLI refactor — trait-based dispatch, async init helpers, unit test wiring
**Confidence:** HIGH

## Summary

Phase 29 is a focused tech debt fix. The `mnemonic recall` CLI command currently bypasses the `StorageBackend` trait and queries SQLite directly via raw SQL inside `cmd_list_memories()` and `cmd_get_memory()`. This means recall silently reads from the SQLite file even when the user has configured Qdrant or Postgres as their storage provider.

The fix is mechanical and well-scoped: three function signatures change (one public, two private), two bodies of raw SQL are replaced with trait method calls, one init helper is created or extended, and the call site in `main.rs` is updated. No new external dependencies, no new CLI flags, no output format changes. The `StorageBackend` trait already has both `list()` and `get_by_id()` methods with the exact semantics needed. The `create_backend()` factory already exists and handles all three backends. The pattern is already established in `init_compaction()`.

The test strategy is conservative: keep existing CLI integration tests (they use the binary, which will transparently use `SqliteBackend` after the refactor), update unit test helpers to construct `SqliteBackend` instead of raw `Arc<Connection>`, and add one delegation assertion to confirm the trait routing is live.

**Primary recommendation:** Follow the `init_compaction()` pattern exactly — call `init_db()` then `create_backend()` in a new `init_recall()` helper. Pass `Arc<dyn StorageBackend>` through the call chain. Replace raw SQL in both handlers with trait method calls. Keep all output formatting identical.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Create a new init helper (or extend `init_db`) that calls `init_db()` + `create_backend()` to return an `Arc<dyn StorageBackend>`. For SQLite this adds negligible overhead. For Qdrant/Postgres it establishes the required network connection.
- **D-02:** Recall must NOT load the embedding model — it only needs the storage backend. Keep the fast init tier (~50ms for SQLite, slightly more for remote backends due to connection).
- **D-03:** `run_recall()` signature changes from `Arc<tokio_rusqlite::Connection>` to `Arc<dyn StorageBackend>` — this is the minimal interface needed (list + get_by_id).
- **D-04:** Do NOT use `MemoryService` for recall — it wraps embedding + backend, but recall needs no embedding. Using `MemoryService` would require loading the ~22MB model and add 2-3s startup time.
- **D-05:** `main.rs` recall branch updates to call `create_backend()` after `init_db()` and passes the backend to `run_recall()`.
- **D-06:** `cmd_list_memories()` replaces raw SQLite SQL with `backend.list(ListParams { ... })` — uses the trait's existing `list()` method which returns `ListResponse { memories, total }`.
- **D-07:** `cmd_get_memory()` replaces raw SQLite SQL with `backend.get_by_id(id)` — uses the trait's existing `get_by_id()` method.
- **D-08:** All output formatting (table headers, truncation, JSON mode, footer "Showing X of Y") stays identical — only the data source changes from raw SQL to trait calls.
- **D-09:** Use `ListResponse.total` from `StorageBackend::list()` for the footer count — every backend implementation already computes this. Remove the separate COUNT(*) query.
- **D-10:** Keep SQLite-only tests using the `seed_memory()` direct-insert pattern — the trait routing is already tested per-backend in phases 21-24.
- **D-11:** Update existing recall tests to construct an `SqliteBackend` and pass it as `Arc<dyn StorageBackend>` instead of raw `Arc<Connection>`.
- **D-12:** Add at least one test verifying that `run_recall` delegates to `StorageBackend::list()` (not raw SQL) — this can be a simple assertion that results match `backend.list()` output.

### Claude's Discretion

- Whether to create a dedicated `init_recall()` helper or reuse/extend `init_db()` with backend creation inline in main.rs
- Exact parameter mapping from RecallArgs to ListParams fields (offset default, tag handling)
- Whether to add the `config` import to main.rs recall branch or restructure init_db to also return config

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DEBT-01 | recall CLI routes all operations through StorageBackend trait instead of raw SQLite (fixes v1.4 known gap) | Trait interface verified (`list()`, `get_by_id()`), factory verified (`create_backend()`), init pattern verified (`init_compaction()`), parameter mapping documented below |
</phase_requirements>

## Standard Stack

### Core (no new dependencies required)

| Component | Current Version | Purpose | Already Present |
|-----------|----------------|---------|-----------------|
| `StorageBackend` trait | — | `list()` + `get_by_id()` replace raw SQL | `src/storage/mod.rs:73-98` |
| `create_backend()` | — | Factory returning `Arc<dyn StorageBackend>` | `src/storage/mod.rs:112-115` |
| `ListParams` | — | Input to `backend.list()` | `src/service.rs:46-54` |
| `ListResponse` | — | Output from `backend.list()` with `total` | `src/service.rs:69-72` |
| `Memory` | — | Output from `backend.get_by_id()` | `src/service.rs:56-66` |
| `tokio_rusqlite::Connection` | 0.7 | Still needed for SQLite init, removed from recall internals | `Cargo.toml:28` |
| `SqliteBackend` | — | Direct-construct for tests | `src/storage/sqlite.rs` |

This phase adds **zero new Cargo dependencies**.

**Installation:** None required.

## Architecture Patterns

### Recommended Structure

The changes are confined to two files:

```
src/
├── cli.rs          # run_recall(), cmd_list_memories(), cmd_get_memory(), init_recall() [all modified]
└── main.rs         # Recall branch: init_recall() call replaces init_db() [lines 40-43 modified]
```

### Pattern 1: New `init_recall()` helper (follow `init_compaction()` verbatim)

**What:** A fast-path init that calls `init_db()` then `create_backend()` — returns `(Arc<dyn StorageBackend>, Config)`.

**When to use:** Recall command dispatch in `main.rs`. Mirrors how `init_compaction()` at `src/cli.rs:318-354` calls `init_db()` internals followed by `create_backend()`.

**Reference pattern from `init_db_and_embedding()` (src/cli.rs:270-315):**
```rust
// This is the EXACT pattern for create_backend() inside an init helper:
let backend: std::sync::Arc<dyn crate::storage::StorageBackend> =
    crate::storage::create_backend(&config, conn_arc).await
        .map_err(|e| anyhow::anyhow!("backend creation failed: {}", e))?;
```

**New `init_recall()` skeleton:**
```rust
/// Fast-path init for `mnemonic recall` — DB + backend, no embedding.
/// Returns Arc<dyn StorageBackend> for trait-based list/get_by_id.
pub async fn init_recall(
    db_override: Option<String>,
) -> anyhow::Result<(std::sync::Arc<dyn crate::storage::StorageBackend>, crate::config::Config)> {
    let (conn_arc, config) = init_db(db_override).await?;
    let backend = crate::storage::create_backend(&config, conn_arc).await
        .map_err(|e| anyhow::anyhow!("backend creation failed: {}", e))?;
    Ok((backend, config))
}
```

This is the recommended approach (creates a named helper, keeps main.rs clean, follows existing convention).

### Pattern 2: Updated `run_recall()` signature

**What:** Change parameter from `Arc<tokio_rusqlite::Connection>` to `Arc<dyn StorageBackend>`.

**Before (src/cli.rs:457):**
```rust
pub async fn run_recall(args: RecallArgs, conn: std::sync::Arc<tokio_rusqlite::Connection>, json: bool)
```

**After:**
```rust
pub async fn run_recall(args: RecallArgs, backend: std::sync::Arc<dyn crate::storage::StorageBackend>, json: bool)
```

The dispatch body stays identical — only type changes propagate to `cmd_get_memory` and `cmd_list_memories` calls.

### Pattern 3: Replace `cmd_list_memories()` raw SQL with `backend.list()`

**What:** The raw SQL block (src/cli.rs:580-622) is replaced by a single trait call. The parameter mapping from `RecallArgs` fields to `ListParams` fields:

| RecallArgs field | ListParams field | Notes |
|-----------------|-----------------|-------|
| `agent_id` | `agent_id` | Direct pass |
| `session_id` | `session_id` | Direct pass |
| `limit` | `limit` | Wrap in `Some(limit)` |
| (not in args) | `tag` | `None` (no tag filter on recall) |
| (not in args) | `after` | `None` |
| (not in args) | `before` | `None` |
| (not in args) | `offset` | `None` (page from beginning) |

**After (replacing lines 576-622):**
```rust
async fn cmd_list_memories(
    backend: std::sync::Arc<dyn crate::storage::StorageBackend>,
    agent_id: Option<String>,
    session_id: Option<String>,
    limit: u32,
    json: bool,
) {
    let params = crate::service::ListParams {
        agent_id,
        session_id,
        limit: Some(limit),
        tag: None,
        after: None,
        before: None,
        offset: None,
    };
    let result = backend.list(params).await;
    // ... rest of output formatting unchanged
}
```

The `result` becomes `Result<ListResponse, ApiError>` instead of the current `Result<(Vec<Memory>, u64), rusqlite::Error>`. The destructuring becomes `Ok(resp)` with `resp.memories` and `resp.total`.

**Error handling:** `ApiError` maps to a string via `Display`. The existing `Err(e)` branch calls `eprintln!("error: failed to list memories: {}", e)` — this continues to work unchanged since `ApiError` implements `Display`.

### Pattern 4: Replace `cmd_get_memory()` raw SQL with `backend.get_by_id()`

**Before (src/cli.rs:668-722):** Raw SQLite query returning `Option<Memory>`.

**After:**
```rust
async fn cmd_get_memory(
    backend: std::sync::Arc<dyn crate::storage::StorageBackend>,
    id: String,
    json: bool,
) {
    let result = backend.get_by_id(&id).await;
    // result is Result<Option<Memory>, ApiError>
    // All existing Ok(Some(mem)) / Ok(None) / Err(e) arms remain identical
}
```

### Pattern 5: `main.rs` recall branch update

**Before (main.rs:40-43):**
```rust
Some(cli::Commands::Recall(recall_args)) => {
    let (conn_arc, _config) = cli::init_db(db_override).await?;
    cli::run_recall(recall_args, conn_arc, json).await;
    return Ok(());
}
```

**After:**
```rust
Some(cli::Commands::Recall(recall_args)) => {
    let (backend, _config) = cli::init_recall(db_override).await?;
    cli::run_recall(recall_args, backend, json).await;
    return Ok(());
}
```

### Pattern 6: Test helper migration (D-11)

Existing unit tests in `cli.rs #[cfg(test)]` that currently use `Arc<Connection>` for recall must instead construct `SqliteBackend`. Model from existing `test_key_service()` helper at `src/cli.rs:925-934`:

```rust
async fn test_backend() -> std::sync::Arc<dyn crate::storage::StorageBackend> {
    crate::db::register_sqlite_vec();
    let config = crate::config::Config {
        port: 0,
        db_path: ":memory:".to_string(),
        ..crate::config::Config::default()
    };
    let conn = crate::db::open(&config).await.unwrap();
    let conn_arc = std::sync::Arc::new(conn);
    crate::storage::create_backend(&config, conn_arc).await.unwrap()
}
```

### Anti-Patterns to Avoid

- **Using `MemoryService` in recall init:** Triggers 22MB model download, 2-3s startup time. Recall needs only `Arc<dyn StorageBackend>`. See D-04.
- **Calling `validate_config()` in `init_recall()`:** The `init_db()` helper deliberately skips this for fast-path commands. `create_backend()` does its own validation. Do not add a `validate_config()` call.
- **Keeping a raw `Arc<Connection>` parameter anywhere in the recall call chain:** The entire chain must use `Arc<dyn StorageBackend>` so non-SQLite backends work.
- **Adding `--storage-provider` as a new CLI arg:** Not required. The success criteria tests via the `MNEMONIC_STORAGE_PROVIDER` env var, which flows through `load_config()` (already called inside `init_db()`).
- **Changing output formatting:** The table headers, truncation widths, footer message format, and JSON shape must remain identical. Users and scripts depend on this.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Listing memories with filter + pagination | New SQL in cmd_list_memories | `StorageBackend::list(ListParams)` | Already handles agent_id, session_id, limit, offset, total count for all three backends |
| Fetching single memory by ID | New SQL in cmd_get_memory | `StorageBackend::get_by_id(&str)` | Returns `Option<Memory>` — zero raw SQL needed |
| Backend selection | Manual `match config.storage_provider` in recall | `create_backend(&config, conn)` | Factory already handles all three backends + feature-gate errors |
| Total count for footer | Separate `COUNT(*)` query | `ListResponse.total` | Every backend already computes this atomically with the list query |

## Common Pitfalls

### Pitfall 1: `ApiError` vs `rusqlite::Error` in match arms

**What goes wrong:** The existing error arms `Err(e)` in `cmd_list_memories()` and `cmd_get_memory()` currently pattern-match `rusqlite::Error`. After the refactor, the error type is `ApiError` (from `src/error.rs`).

**Why it happens:** Mechanical find-and-replace of query code doesn't update the error type annotation, causing a compile error.

**How to avoid:** The `Err(e)` arm does not need to name the type — `eprintln!("error: ...: {}", e)` works for any type implementing `Display`. Just let the compiler infer. No code change needed in the error arms beyond removing type annotations if any exist.

**Warning signs:** `E0308: mismatched types` pointing at the `Err(e)` arm.

### Pitfall 2: `conn.call()` closure removal

**What goes wrong:** The raw SQL uses `conn.call(move |c| -> Result<..., rusqlite::Error> { ... }).await` — a `tokio_rusqlite` pattern. The backend trait call is a direct `.await`, not a closure. Leaving the `conn.call()` wrapper with the new trait call inside it will not compile.

**Why it happens:** Refactoring the body without removing the outer `conn.call()` wrapper.

**How to avoid:** Replace the entire `conn.call(...).await` block with `backend.list(params).await` or `backend.get_by_id(&id).await`. There is no closure wrapper in the new code.

**Warning signs:** `move closure` errors, or `c.prepare()` / `c.query_row()` calls that no longer have a `c: &rusqlite::Connection` in scope.

### Pitfall 3: Stale `conn` variable in the call chain

**What goes wrong:** After updating `run_recall()` to accept `Arc<dyn StorageBackend>`, the internal calls to `cmd_list_memories(conn, ...)` and `cmd_get_memory(conn, ...)` still pass `conn`. If `conn` no longer exists in scope, it's a compile error.

**Why it happens:** Partial update — the outer function signature is updated but the inner call sites are not.

**How to avoid:** Update all three functions in a single edit pass: `run_recall()`, `cmd_list_memories()`, and `cmd_get_memory()`. The type at every boundary must be `Arc<dyn StorageBackend>`.

### Pitfall 4: `let limit_i64 = limit as i64` leftover

**What goes wrong:** The current `cmd_list_memories()` has `let limit_i64 = limit as i64;` and `let agent_id_c = agent_id.clone(); let session_id_c = session_id.clone();` for the closure captures. These become dead code after the SQL removal and will cause `unused variable` warnings (or errors with `#[deny(warnings)]`).

**How to avoid:** Remove all variables that existed solely to feed the raw SQL. The `agent_id`, `session_id`, and `limit` values go directly into `ListParams`.

### Pitfall 5: `init_recall()` must NOT call `validate_config()`

**What goes wrong:** If `init_recall()` calls `crate::config::validate_config(&config)`, recall will fail when `storage_provider = "sqlite"` but `MNEMONIC_EMBEDDING_PROVIDER` is not set (or similar missing-field validations).

**Why it happens:** Copying from `init_db_and_embedding()` which calls `validate_config()` because embedding provider validation is required there.

**How to avoid:** Follow `init_db()` and `init_compaction()`'s pattern — `init_db()` does NOT call `validate_config()`. The `create_backend()` factory does its own config validation for the storage provider fields only.

### Pitfall 6: `agent_id.is_empty()` display check still needed

**What goes wrong:** After the refactor, `cmd_list_memories()` still has `if mem.agent_id.is_empty() { "(none)" }` in the output loop. The `Memory` struct's `agent_id` field is `String` (not `Option<String>`), so an empty agent is represented as `""` not `None`. This check must stay.

**Why it happens:** Assuming the trait returns `None` for absent agent_ids. The `Memory` struct uses empty string.

**How to avoid:** Keep the `mem.agent_id.is_empty()` check. The `Memory` struct definition at `src/service.rs:56-66` confirms `agent_id: String` and `session_id: String`.

## Code Examples

### Verified: `StorageBackend::list()` signature (src/storage/mod.rs:81)
```rust
// Source: src/storage/mod.rs:81
async fn list(&self, params: ListParams) -> Result<ListResponse, ApiError>;
```

### Verified: `StorageBackend::get_by_id()` signature (src/storage/mod.rs:78)
```rust
// Source: src/storage/mod.rs:78
async fn get_by_id(&self, id: &str) -> Result<Option<Memory>, ApiError>;
```

### Verified: `ListParams` struct (src/service.rs:46-54)
```rust
// Source: src/service.rs:46-54
pub struct ListParams {
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub tag: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
```

### Verified: `ListResponse` struct (src/service.rs:69-72)
```rust
// Source: src/service.rs:69-72
pub struct ListResponse {
    pub memories: Vec<Memory>,
    pub total: u64,
}
```

### Verified: `create_backend()` signature (src/storage/mod.rs:112-115)
```rust
// Source: src/storage/mod.rs:112-115
pub async fn create_backend(
    config: &Config,
    sqlite_conn: Arc<Connection>,
) -> Result<Arc<dyn StorageBackend>, ApiError>
```

### Verified: Existing `init_compaction()` pattern to follow (src/cli.rs:320-354)

The `init_compaction()` function at `src/cli.rs:320` demonstrates the established pattern:
1. Call `init_db()` (DB-only init, no validate_config)
2. Call `create_backend(&config, conn_arc)`
3. Return a service wrapping the backend

`init_recall()` should be a simplified version (no embedding engine, no LLM, just backend + config).

### Verified: D-12 delegation test pattern

```rust
// Unit test verifying run_recall delegates to StorageBackend::list() (not raw SQL)
#[tokio::test]
async fn test_run_recall_delegates_to_backend_list() {
    let backend = test_backend().await;
    // Seed via backend.store() (not raw SQL)
    // Call run_recall() with a RecallArgs
    // Assert results match backend.list() output
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Raw SQL in `cmd_list_memories()` | `backend.list(ListParams)` | Phase 29 | Enables Qdrant/Postgres recall |
| Raw SQL in `cmd_get_memory()` | `backend.get_by_id(&str)` | Phase 29 | Enables Qdrant/Postgres recall |
| `init_db()` → raw conn → recall | `init_recall()` → backend → recall | Phase 29 | Backend-agnostic init tier |
| Separate COUNT(*) query | `ListResponse.total` | Phase 29 | Removes redundant DB round-trip |

## Environment Availability

Step 2.6: SKIPPED — This phase is purely a Rust source code refactor. No external services, databases, or CLI tools beyond the project's own build chain are required. The test suite uses in-memory SQLite (`:memory:`) which is bundled (rusqlite features = ["bundled"]).

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | none — standard Cargo test runner |
| Quick run command | `cargo test --lib -- cli::tests 2>&1` |
| Full suite command | `cargo test 2>&1` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DEBT-01 | `run_recall` delegates to `StorageBackend::list()` not raw SQL | unit | `cargo test --lib -- cli::tests::test_run_recall_delegates 2>&1` | No — Wave 0 |
| DEBT-01 | `run_recall` with get_by_id delegates to `backend.get_by_id()` | unit | `cargo test --lib -- cli::tests::test_run_recall_by_id_delegates 2>&1` | No — Wave 0 |
| DEBT-01 | Existing recall CLI integration tests still pass (SQLite regression) | integration | `cargo test --test cli_integration -- recall 2>&1` | Yes (existing) |
| DEBT-01 | Existing library unit tests still compile and pass | unit | `cargo test --lib 2>&1` | Yes (85 tests passing) |

### Sampling Rate

- **Per task commit:** `cargo test --lib -- cli::tests 2>&1`
- **Per wave merge:** `cargo test 2>&1`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `src/cli.rs #[cfg(test)] mod tests` — add `test_run_recall_delegates_to_backend_list()` and `test_run_recall_by_id_delegates_to_backend_get()` — covers DEBT-01 delegation assertion (D-12)
- [ ] `async fn test_backend()` helper in `src/cli.rs tests` — needed for above tests; constructs `Arc<dyn StorageBackend>` via `create_backend()` (D-11 pattern)

The existing integration test file `tests/cli_integration.rs` has full recall coverage (RCL-01 through RCL-03). These tests run the binary end-to-end with SQLite and will continue to pass unchanged after the refactor — no modifications needed to the integration test file.

## Sources

### Primary (HIGH confidence)

- `src/cli.rs` lines 457-722 — `run_recall()`, `cmd_list_memories()`, `cmd_get_memory()` — current implementation directly read
- `src/cli.rs` lines 253-354 — `init_db()`, `init_db_and_embedding()`, `init_compaction()` — init patterns directly read
- `src/main.rs` lines 40-43 — Recall command dispatch directly read
- `src/storage/mod.rs` lines 73-115 — `StorageBackend` trait and `create_backend()` directly read
- `src/service.rs` lines 46-72 — `ListParams`, `ListResponse`, `Memory` directly read
- `tests/cli_integration.rs` lines 385-714 — All recall tests directly read; `seed_memory()` helper at line 387

### Secondary (MEDIUM confidence)

- None required — all research grounded in direct codebase reads

### Tertiary (LOW confidence)

- None

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH — no new dependencies; all components directly verified in source
- Architecture: HIGH — patterns read directly from codebase; init_compaction() is exact template
- Pitfalls: HIGH — identified from direct code reading of existing function bodies
- Test strategy: HIGH — existing test infrastructure directly inspected; 85 lib tests pass, integration tests use binary

**Research date:** 2026-03-22
**Valid until:** Until Phase 29 implementation begins (code is the source of truth)
