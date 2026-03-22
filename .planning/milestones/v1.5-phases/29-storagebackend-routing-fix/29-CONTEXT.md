# Phase 29: StorageBackend Routing Fix - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Route the `mnemonic recall` CLI through the `StorageBackend` trait so that list and get-by-id operations work correctly regardless of which backend is configured (SQLite, Qdrant, Postgres). This is v1.4 tech debt — recall currently bypasses the trait and queries SQLite directly.

</domain>

<decisions>
## Implementation Decisions

### Init tier for recall
- **D-01:** Create a new init helper (or extend `init_db`) that calls `init_db()` + `create_backend()` to return an `Arc<dyn StorageBackend>`. For SQLite this adds negligible overhead. For Qdrant/Postgres it establishes the required network connection.
- **D-02:** Recall must NOT load the embedding model — it only needs the storage backend. Keep the fast init tier (~50ms for SQLite, slightly more for remote backends due to connection).

### Signature refactor
- **D-03:** `run_recall()` signature changes from `Arc<tokio_rusqlite::Connection>` to `Arc<dyn StorageBackend>` — this is the minimal interface needed (list + get_by_id).
- **D-04:** Do NOT use `MemoryService` for recall — it wraps embedding + backend, but recall needs no embedding. Using `MemoryService` would require loading the ~22MB model and add 2-3s startup time.
- **D-05:** `main.rs` recall branch updates to call `create_backend()` after `init_db()` and passes the backend to `run_recall()`.

### Internal function refactor
- **D-06:** `cmd_list_memories()` replaces raw SQLite SQL with `backend.list(ListParams { ... })` — uses the trait's existing `list()` method which returns `ListResponse { memories, total }`.
- **D-07:** `cmd_get_memory()` replaces raw SQLite SQL with `backend.get_by_id(id)` — uses the trait's existing `get_by_id()` method.
- **D-08:** All output formatting (table headers, truncation, JSON mode, footer "Showing X of Y") stays identical — only the data source changes from raw SQL to trait calls.

### Total count for list
- **D-09:** Use `ListResponse.total` from `StorageBackend::list()` for the footer count — every backend implementation already computes this. Remove the separate COUNT(*) query.

### Test strategy
- **D-10:** Keep SQLite-only tests using the `seed_memory()` direct-insert pattern — the trait routing is already tested per-backend in phases 21-24.
- **D-11:** Update existing recall tests to construct an `SqliteBackend` and pass it as `Arc<dyn StorageBackend>` instead of raw `Arc<Connection>`.
- **D-12:** Add at least one test verifying that `run_recall` delegates to `StorageBackend::list()` (not raw SQL) — this can be a simple assertion that results match `backend.list()` output.

### Claude's Discretion
- Whether to create a dedicated `init_recall()` helper or reuse/extend `init_db()` with backend creation inline in main.rs
- Exact parameter mapping from RecallArgs to ListParams fields (offset default, tag handling)
- Whether to add the `config` import to main.rs recall branch or restructure init_db to also return config

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Recall CLI (code being refactored)
- `src/cli.rs` lines 457-468 — `run_recall()` function: current signature with raw `Arc<Connection>`, dispatches to cmd_get_memory/cmd_list_memories
- `src/cli.rs` lines 569-665 — `cmd_list_memories()`: raw SQLite queries that must be replaced with `backend.list()`
- `src/cli.rs` lines 668-710 — `cmd_get_memory()`: raw SQLite query that must be replaced with `backend.get_by_id()`

### Main.rs recall branch
- `src/main.rs` lines 40-43 — Recall command dispatch: currently calls `init_db()` and passes raw connection

### StorageBackend trait (target interface)
- `src/storage/mod.rs` lines 73-98 — Full `StorageBackend` trait with `get_by_id()` and `list()` methods
- `src/storage/mod.rs` lines 112-115 — `create_backend()` factory function signature

### Service types (used by trait)
- `src/service.rs` lines 46-54 — `ListParams` struct (agent_id, session_id, tag, after, before, limit, offset)
- `src/service.rs` lines 69-72 — `ListResponse` struct (memories, total)
- `src/service.rs` lines 56-66 — `Memory` struct returned by `get_by_id()`

### Init helpers (patterns to follow)
- `src/cli.rs` lines 253-268 — `init_db()`: DB-only init, returns `(Arc<Connection>, Config)`
- `src/cli.rs` lines 270-316 — `init_db_and_embedding()`: DB+embedding init, returns `(MemoryService, Config)` — recall should NOT use this

### Requirements
- `.planning/REQUIREMENTS.md` line 44 — DEBT-01: recall CLI routes through StorageBackend trait

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `StorageBackend::list(ListParams)` — Already handles agent_id, session_id, limit filtering and returns total count; direct replacement for the raw SQL in cmd_list_memories
- `StorageBackend::get_by_id(&str)` — Returns `Option<Memory>`; direct replacement for the raw SQL in cmd_get_memory
- `create_backend()` — Factory already exists; recall just needs to call it
- `SqliteBackend` — For tests, can be constructed directly to replace raw connection in test setup

### Established Patterns
- `run_remember` and `run_search` both receive `MemoryService` which wraps `Arc<dyn StorageBackend>` — recall follows the same pattern but at the backend level (no embedding needed)
- `init_compaction()` (cli.rs:318) creates backend via `create_backend()` — can be used as reference for recall's backend init
- All output formatting uses the same `truncate()` helper and consistent table widths — retain these exactly

### Integration Points
- `main.rs:40-43` — Recall command branch needs to call `create_backend()` after `init_db()`
- `cli.rs:457` — `run_recall` signature change propagates to main.rs caller
- `cli.rs:569,668` — Internal functions change from `Arc<Connection>` to `Arc<dyn StorageBackend>`

</code_context>

<specifics>
## Specific Ideas

No specific requirements — this is a straightforward tech debt fix. The refactor must be invisible to users: same CLI flags, same output format, same behavior. The only difference is that recall now works with Qdrant and Postgres backends, not just SQLite.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 29-storagebackend-routing-fix*
*Context gathered: 2026-03-22*
