---
phase: 32-operational-actions
plan: "01"
subsystem: backend-api, dashboard-client
tags: [rest-api, auth-scope, typescript, integration-tests]
dependency_graph:
  requires: []
  provides: [GET /memories/{id} endpoint, compactMemories(), fetchMemoryById(), CompactParams, CompactResponse, ClusterMapping]
  affects: [src/server.rs, src/service.rs, tests/integration.rs, dashboard/src/api.ts]
tech_stack:
  added: []
  patterns: [scope-enforcement pattern for per-ID routes, apiFetch typed wrapper pattern]
key_files:
  created: []
  modified:
    - src/service.rs
    - src/server.rs
    - tests/integration.rs
    - dashboard/src/api.ts
decisions:
  - "GET /memories/{id} scope enforcement follows the same two-step pattern as DELETE: get_memory_agent_id() lookup first, then get_memory() only if ownership matches"
  - "fetchMemoryById JSDoc documents Promise.allSettled pattern for Plan 02 CompactTab preview fetches to degrade gracefully on 403"
metrics:
  duration: 334s
  completed_date: "2026-03-23T03:19:47Z"
  tasks_completed: 2
  files_changed: 4
---

# Phase 32 Plan 01: GET /memories/{id} Endpoint and Dashboard API Wrappers Summary

**One-liner:** GET /memories/{id} with scoped-key 403 enforcement, plus CompactParams/CompactResponse types and compactMemories()/fetchMemoryById() wrappers in api.ts.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add GET /memories/{id} backend handler with scope enforcement and integration tests | 05530bc | src/service.rs, src/server.rs, tests/integration.rs |
| 2 | Add compactMemories and fetchMemoryById typed wrappers to api.ts | 29945df | dashboard/src/api.ts |

## What Was Built

### Task 1: GET /memories/{id} Backend

**src/service.rs:** Added `get_memory(&self, id: &str) -> Result<Memory, ApiError>` method that calls `backend.get_by_id(id)` and maps `None` to `ApiError::NotFound`. Placed after the existing `get_memory_agent_id` helper.

**src/server.rs:**
- Added `get_memory_handler` following the exact scope enforcement pattern from `delete_memory_handler`: look up `get_memory_agent_id` first, return 403 if the memory belongs to a different agent, then call `get_memory()` only if ownership matches or no scope restriction applies.
- Updated route registration from `.route("/memories/{id}", delete(delete_memory_handler))` to `.route("/memories/{id}", get(get_memory_handler).delete(delete_memory_handler))`.

**tests/integration.rs:** Added 4 tests after `test_scoped_delete_own_memory_ok`:
- `get_memory_by_id_returns_created_memory` — creates via POST, fetches via GET, asserts 200 + field values
- `get_memory_by_id_returns_404_for_missing` — asserts 404 for a nil UUID
- `get_memory_by_id_scoped_key_wrong_owner_403` — creates memory for agent-B, scoped key for agent-A, asserts 403
- `get_memory_by_id_scoped_key_own_memory_200` — creates memory for agent-A, scoped key for agent-A, asserts 200

All 4 tests pass. All 58 existing tests continue to pass.

### Task 2: Dashboard API Wrappers

**dashboard/src/api.ts:** Added after `StatsResponse`:
- `CompactParams` interface: `agent_id`, optional `threshold`, optional `dry_run` (no `max_candidates` per D-03)
- `ClusterMapping` interface: `source_ids: string[]`, `new_id: string | null`
- `CompactResponse` interface: `run_id`, `clusters_found`, `memories_merged`, `memories_created`, `id_mapping`, `truncated`

Added at end of file:
- `compactMemories(token, params, signal)` — POSTs to `/memories/compact` with `Content-Type: application/json`
- `fetchMemoryById(token, id, signal)` — GETs `/memories/${id}`, with JSDoc noting the `Promise.allSettled` pattern for CompactTab preview callers

TypeScript compiles with zero errors (`npx tsc --noEmit` exits 0). Vite build succeeds (`dist/index.html 40.77 kB`).

## Verification Results

- `cargo test --test integration get_memory_by_id`: 4 passed, 0 failed
- `cargo test`: 58 passed (integration) + lib tests, 0 failed, 1 ignored
- `npx tsc --noEmit`: exit 0, no errors
- `npx vite build`: exit 0, `dist/index.html 40.77 kB`

## Decisions Made

1. **GET /memories/{id} scope enforcement** follows the same two-step pattern as DELETE: `get_memory_agent_id()` lookup first to verify ownership, then `get_memory()` only if ownership matches. This avoids a double backend call on the happy path for scoped keys (404 is short-circuited by the first lookup).

2. **fetchMemoryById JSDoc** explicitly documents the `Promise.allSettled` pattern so Plan 02's CompactTab knows to catch errors per-fetch and degrade gracefully (show memory ID instead of content) rather than triggering re-auth on a 403.

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None.

## Self-Check: PASSED

Files exist:
- FOUND: src/service.rs (contains `pub async fn get_memory`)
- FOUND: src/server.rs (contains `get_memory_handler` and `get(get_memory_handler).delete`)
- FOUND: tests/integration.rs (contains all 4 `get_memory_by_id_*` tests)
- FOUND: dashboard/src/api.ts (contains `compactMemories`, `fetchMemoryById`, `CompactParams`, `ClusterMapping`, `CompactResponse`)

Commits exist:
- FOUND: 05530bc (feat(32-01): add GET /memories/{id} endpoint)
- FOUND: 29945df (feat(32-01): add CompactParams, ClusterMapping, CompactResponse types)
