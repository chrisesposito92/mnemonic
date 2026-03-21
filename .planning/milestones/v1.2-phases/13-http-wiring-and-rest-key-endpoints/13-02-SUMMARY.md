---
phase: 13-http-wiring-and-rest-key-endpoints
plan: "02"
subsystem: auth
tags: [rust, axum, api-keys, auth, integration-tests, scope-enforcement]

# Dependency graph
requires:
  - phase: 13-http-wiring-and-rest-key-endpoints
    plan: "01"
    provides: "enforce_scope helper, ApiError::Forbidden, auth middleware on /memories routes"
  - phase: 12-auth-middleware
    provides: "auth_middleware, AuthContext, KeyService wired in AppState"
  - phase: 11-keyservice-core
    provides: "KeyService::create/list/revoke, ApiKey struct, raw token generation"
provides:
  - "create_key_handler (POST /keys) returning 201 with raw_token shown once"
  - "list_keys_handler (GET /keys) returning key metadata sans raw/hashed values"
  - "revoke_key_handler (DELETE /keys/:id) soft-deleting by id"
  - "/keys and /keys/{id} routes in protected router group (auth-enforced)"
  - "8 integration tests proving AUTH-04 scope enforcement and key CRUD end-to-end"
affects: [cli-key-management, future-key-expiry-phases]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Key handlers use .map_err(|e| ApiError::Internal(MnemonicError::Db(e))) for DbError mapping"
    - "Key routes placed before .route_layer() in protected group — same pattern as /memories"
    - "raw_token never logged, only returned in JSON response once"
    - "Integration tests share AppState across multiple Router instances for multi-step scenarios"

key-files:
  created: []
  modified:
    - src/server.rs
    - tests/integration.rs

key-decisions:
  - "Key handlers do NOT call enforce_scope — any valid key (wildcard or scoped) can manage keys per D-03"
  - "POST /keys returns 201 (not 200) — creates a new addressable resource"
  - "DELETE /keys/:id returns 200 with {revoked: true, id} — consistent with DELETE /memories/:id returning body"
  - "Integration tests use build_scoped_auth_app helper to DRY up scoped key setup"
  - "test_delete_key_revokes_access keeps first key active to maintain auth mode after revoking second key"

patterns-established:
  - "Multi-step integration tests: create key via service, build router, make HTTP requests — avoids chicken-and-egg"
  - "Open-mode bootstrap: first key creation via service directly, subsequent keys via HTTP auth"

requirements-completed: [AUTH-04, INFRA-03]

# Metrics
duration: 15min
completed: 2026-03-21
---

# Phase 13 Plan 02: Key Management Endpoints Summary

**REST key management endpoints (POST/GET/DELETE /keys) added to protected router with 8 scope enforcement and key CRUD integration tests — AUTH-04 proved end-to-end**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-21T02:30:00Z
- **Completed:** 2026-03-21T02:45:00Z
- **Tasks:** 2 (Task 1: handlers + wiring, Task 2: 8 integration tests)
- **Files modified:** 2

## Accomplishments

- Three key management handlers compile and are wired behind auth middleware
- POST /keys returns 201 with raw_token (shown once, never logged), key metadata object
- GET /keys returns all key metadata without exposing raw_token or hashed_key
- DELETE /keys/:id soft-deletes by id, returns {revoked: true, id}
- 5 scope enforcement tests prove AUTH-04 mismatch/force/wildcard/ownership cases
- 3 key CRUD endpoint tests prove create/list/revoke HTTP surface
- Full test suite: 53 tests pass, 0 failures

## Task Commits

Each task was committed atomically:

1. **Task 1: Add key management handlers and wire into protected router** - `a484a24` (feat)
2. **Task 2: Add integration tests for scope enforcement and key endpoints** - `e9b22b0` (test)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `src/server.rs` - Added CreateKeyRequest struct, create_key_handler, list_keys_handler, revoke_key_handler; wired /keys and /keys/{id} into protected router group
- `tests/integration.rs` - Added build_scoped_auth_app helper and 8 new integration tests

## Decisions Made

- Key handlers skip enforce_scope — any authenticated key (wildcard or scoped) can manage keys; scope enforcement is for memory access only (per D-03)
- POST /keys returns 201 since it creates a new resource; DELETE /keys/:id returns 200 with a body for consistency with existing memory delete pattern
- Integration test strategy: use build_scoped_auth_app helper to keep scoped key tests DRY; for multi-step scenarios requiring both service operations and HTTP requests, create state via build_test_state() then build_router(state.clone()) per step

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- REST API surface for v1.2 Authentication / API Keys is complete (SC1–SC4 all implemented)
- Phase 13 complete: scope enforcement wired in HTTP layer, key CRUD endpoints live, 53 tests green
- Ready for CLI key management phase or milestone wrap-up

---
*Phase: 13-http-wiring-and-rest-key-endpoints*
*Completed: 2026-03-21*
