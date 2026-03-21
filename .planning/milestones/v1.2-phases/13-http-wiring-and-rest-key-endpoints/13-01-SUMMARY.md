---
phase: 13-http-wiring-and-rest-key-endpoints
plan: "01"
subsystem: auth-scope-enforcement
tags: [auth, scope, axum, handlers, api-error]
dependency_graph:
  requires: [12-01-auth-middleware]
  provides: [scope-enforcement-handlers, forbidden-error, memory-ownership-check]
  affects: [src/error.rs, src/service.rs, src/server.rs]
tech_stack:
  added: []
  patterns: [Option<Extension<AuthContext>>, enforce_scope helper, ownership-check-before-delete]
key_files:
  created: []
  modified:
    - src/error.rs
    - src/service.rs
    - src/server.rs
decisions:
  - enforce_scope free function (not method) keeps handlers uniform and testable in isolation
  - delete_memory_handler uses direct DB ownership lookup instead of enforce_scope — path param has no agent_id to compare against, so a pre-flight query is required
  - Optional<Extension<AuthContext>> (not required) means open mode and wildcard keys both work without changes to middleware behavior
metrics:
  duration: "2 minutes"
  completed_date: "2026-03-21"
  tasks_completed: 2
  files_modified: 3
---

# Phase 13 Plan 01: Scope Enforcement on Memory Handlers Summary

**One-liner:** Scoped API key enforcement via enforce_scope helper + ApiError::Forbidden(403) across all 5 memory/compaction handlers with ownership-check before DELETE.

## What Was Built

Added AUTH-04 scope enforcement to all 5 existing memory/compaction handlers:

1. **`ApiError::Forbidden(String)` variant** in `src/error.rs` — returns HTTP 403 with `{"error": "forbidden", "detail": "..."}` body, distinguishing it from 401 Unauthorized.

2. **`get_memory_agent_id(&str) -> Result<Option<String>, ApiError>`** in `src/service.rs` — lightweight SELECT to fetch only agent_id for ownership verification before DELETE, avoiding full memory fetch.

3. **`enforce_scope(auth, requested) -> Result<Option<String>, ApiError>`** free function in `src/server.rs` — centralizes the three-way logic:
   - Open mode (no AuthContext) → pass through, no enforcement
   - Wildcard key (allowed_agent_id = None) → pass through, no restriction
   - Scoped key + no agent_id in request → force key's scope as effective agent_id
   - Scoped key + mismatched agent_id in request → 403 Forbidden

4. **All 5 handlers modified** to accept `auth: Option<Extension<AuthContext>>` and call enforce_scope (create, search, list, compact) or perform direct ownership check (delete).

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| `enforce_scope` as free function (not method) | Uniform call site across handlers; pure function — easy to unit test if needed later |
| DELETE handler does NOT use enforce_scope | Path ID has no agent_id to compare; must DB-lookup the memory's owner before checking scope |
| `Option<Extension<AuthContext>>` (optional, not required) | Handles open mode and wildcard keys without changing middleware behavior; middleware injects context only when auth is active and key is valid |
| `if effective.is_some()` guard before overwriting agent_id | Preserves client-supplied agent_id in open mode and wildcard key scenarios |

## Verification Results

- `cargo check` — 0 errors, 5 existing warnings (pre-existing, not introduced here)
- `cargo test` — 45 tests pass, 0 failures, 0 regressions
- `grep -n "Forbidden" src/error.rs` — variant at line 88, IntoResponse arm at line 112
- `enforce_scope` called from 4 handlers (create, search, list, compact) — 6 occurrences total (definition + 4 call sites + 1 in compact)
- `get_memory_agent_id` called from delete handler and defined in service
- All 5 handlers have `Option<Extension<AuthContext>>` parameter

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check

- [x] `src/error.rs` modified — contains `Forbidden(String)` and `StatusCode::FORBIDDEN` arm
- [x] `src/service.rs` modified — contains `get_memory_agent_id` returning `Result<Option<String>, ApiError>`
- [x] `src/server.rs` modified — contains `enforce_scope`, all 5 handlers updated
- [x] Commit `0f10506` exists (task 1)
- [x] Commit `3ef3858` exists (task 2)
- [x] All 45 tests pass

## Self-Check: PASSED
