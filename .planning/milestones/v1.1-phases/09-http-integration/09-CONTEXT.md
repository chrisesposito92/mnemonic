# Phase 9: HTTP Integration - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire the existing CompactionService into the HTTP layer via POST /memories/compact, add it to AppState, and verify the full HTTP round-trip with integration tests. All compaction logic is already implemented in Phase 8 — this phase is purely HTTP wiring, request validation, and HTTP-layer test coverage.

</domain>

<decisions>
## Implementation Decisions

### Request validation
- Handler rejects empty `agent_id` with 400 BadRequest before calling compact()  — consistent with how POST /memories rejects empty content
- Handler rejects `threshold` outside 0.0–1.0 range with 400 BadRequest — out-of-range cosine similarity thresholds are nonsensical
- Validation happens at the handler level, not inside CompactionService

### HTTP status codes
- 200 OK for all successful compactions, including 0 clusters found
- Response body contains `clusters_found`, `memories_merged`, `memories_created` counts — caller inspects those
- No 204 special case — simple and consistent with other endpoints

### AppState wiring
- Add `Arc<CompactionService>` to AppState struct
- In main.rs, rename `_compaction` to `compaction` and pass into AppState
- Remove `#![allow(dead_code)]` from compaction.rs — all items now consumed
- Add `POST /memories/compact` route to build_router()

### Handler pattern
- Follow existing pattern exactly: `State(state)` → validation → `state.compaction.compact(req).await?` → `Json(response)`
- CompactRequest deserialized from JSON body via `Json<CompactRequest>`
- CompactResponse serialized directly (already derives serde::Serialize)

### Integration test scope
- Test HTTP wiring + key scenarios, not re-test clustering logic (Phase 8 covers that)
- HTTP-layer tests:
  1. Successful compact returns 200 with correct JSON shape (run_id, clusters_found, id_mapping, etc.)
  2. dry_run via HTTP returns 200 with no DB changes (verify GET /memories count unchanged)
  3. Agent isolation via HTTP — compact Agent A, verify Agent B untouched
  4. Validation errors: missing agent_id → 400, empty agent_id → 400, threshold out of range → 400
- Use existing test infrastructure: build_test_state() extended with CompactionService, json_request(), response_json()

### Claude's Discretion
- max_candidates = 0 validation (likely reject with 400)
- Exact error message wording for validation failures
- Whether to extend build_test_state() or create a separate build_test_compact_state() helper
- Handler function naming (compact_handler vs compact_memories_handler)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — API-01 through API-04 define compact endpoint behavior, dry_run, response stats, and ID mapping

### Architecture patterns to follow
- `src/server.rs` — AppState struct, build_router(), existing handler pattern (create_memory_handler is the closest analog)
- `src/compaction.rs` — CompactRequest/CompactResponse types, CompactionService::compact() method signature
- `src/error.rs` — ApiError::BadRequest for validation, ApiError::Internal for service errors, IntoResponse impl
- `src/main.rs` lines 111-118 — CompactionService construction (currently `_compaction`, rename to `compaction`)

### Prior phase context
- `.planning/phases/08-compaction-core/08-CONTEXT.md` — CompactRequest/CompactResponse field definitions, CompactionService design
- `.planning/phases/06-foundation/06-CONTEXT.md` — Schema decisions (compact_runs table used by service)

### Test patterns
- `tests/integration.rs` lines 456-477 — build_test_state(), build_test_app() infrastructure
- `tests/integration.rs` lines 480-493 — json_request(), response_json() test helpers
- `tests/integration.rs` lines 840-858 — build_test_compaction() for service-layer testing (extend or mirror for HTTP-layer)

### Project decisions
- `.planning/PROJECT.md` §Key Decisions — axum for HTTP, tokio-rusqlite async pattern
- `.planning/STATE.md` §Accumulated Context — CompactionService is peer of MemoryService, agent_id required

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AppState` struct (server.rs:26): extend with `pub compaction: Arc<CompactionService>`
- `build_router()` (server.rs:31): add `.route("/memories/compact", post(compact_handler))`
- `create_memory_handler` (server.rs:46): closest analog for handler pattern — Json body extraction + service call + Json response
- `ApiError::BadRequest` (error.rs:81): ready for validation error responses
- `build_test_state()` (integration.rs:456): extend to include CompactionService for HTTP-layer tests
- `json_request()` / `response_json()` (integration.rs:480-493): reusable HTTP test helpers
- `MockEmbeddingEngine` (integration.rs:423): already in test infrastructure

### Established Patterns
- Handler signature: `async fn handler(State(state): State<AppState>, Json(body): Json<T>) -> Result<Json<Value>, ApiError>`
- All handlers return `Result<_, ApiError>` — errors auto-convert via IntoResponse
- Route registration: `.route("/path", post(handler))`
- Service access: `state.service.method()` pattern — compaction will be `state.compaction.compact()`
- Test pattern: insert data via service → call HTTP endpoint via `app.oneshot()` → assert status + JSON body

### Integration Points
- `src/server.rs` AppState: add `compaction` field
- `src/server.rs` build_router(): add compact route
- `src/main.rs` line 111: rename `_compaction` → `compaction`, pass to AppState
- `src/main.rs` line 121-123: extend AppState construction
- `src/compaction.rs` line 1: remove `#![allow(dead_code)]`
- `src/lib.rs`: compaction module already `pub mod compaction;` — no change needed
- `tests/integration.rs`: add HTTP-layer compaction tests

</code_context>

<specifics>
## Specific Ideas

No specific requirements — follow existing handler and test patterns exactly. The create_memory_handler is the blueprint for the compact handler.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 09-http-integration*
*Context gathered: 2026-03-20*
