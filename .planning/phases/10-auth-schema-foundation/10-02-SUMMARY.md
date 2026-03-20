---
phase: 10-auth-schema-foundation
plan: 02
subsystem: auth
tags: [auth, appstate, integration-tests, rust, tokio-rusqlite]

requires:
  - phase: 10-01
    provides: KeyService with count_active_keys(), api_keys DDL, ApiError::Unauthorized

provides:
  - AppState with key_service field (Arc<KeyService>) wired for Phase 12 middleware
  - Startup auth-mode log in main.rs (OPEN/ACTIVE based on DB key count)
  - mod auth declared in both main.rs and lib.rs
  - 5 new integration tests: table, indexes, idempotency, count, 401 shape
  - Full test suite green (39 tests)

affects:
  - phase-11-key-service (fills in KeyService stub methods)
  - phase-12-auth-middleware (reads key_service from AppState)

tech-stack:
  added: []
  patterns:
    - Per-item #[allow(dead_code)] for fields/methods unused until later phases
    - Startup DB query at boot (count_active_keys) to determine auth mode
    - bare module paths in main.rs (auth::KeyService, not crate::auth::KeyService)

key-files:
  created: []
  modified:
    - src/server.rs
    - src/main.rs
    - src/auth.rs
    - tests/integration.rs

key-decisions:
  - "Used #[allow(dead_code)] on key_service field in AppState (Phase 12 adds middleware) and on stub methods create/list/revoke/validate (Phase 11 implements them)"
  - "mod auth declared in main.rs using bare path (auth::KeyService) matching existing pattern for compaction, service, etc."

patterns-established:
  - "Startup auth-mode log: match on count_active_keys().await, INFO for OPEN/ACTIVE, WARN on DB error"
  - "AppState extensibility: add Arc<T> fields for new services, suppress dead_code until wired"

requirements-completed: [INFRA-03]

duration: 8min
completed: "2026-03-20"
---

# Phase 10 Plan 02: Wire AppState and Integration Tests Summary

**KeyService wired into AppState with startup auth-mode log (OPEN/ACTIVE) and 5 new integration tests verifying api_keys table, indexes, idempotency, key count, and 401 response shape — 39 tests green, zero new warnings.**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-03-20T19:50:30Z
- **Completed:** 2026-03-20T19:58:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added `key_service: Arc<KeyService>` field to `AppState` in server.rs (Phase 12 middleware will read it)
- Added `mod auth;` to main.rs and constructed `KeyService` after DB open, logging auth mode at startup
- Updated both `build_test_state()` and `build_test_compact_state()` in integration.rs with `key_service`
- Added 5 new integration tests: `test_api_keys_table_created`, `test_api_keys_indexes`, `test_api_keys_migration_idempotent`, `test_count_active_keys_empty_db`, `test_unauthorized_response_shape`
- Full test suite: 39 passed, 0 failed (was 34 before this plan)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add key_service to AppState, wire in main.rs with startup auth-mode log** - `e535d2c` (feat)
2. **Task 2: Update test AppState construction and add auth migration/count tests** - `f986171` (test)

**Plan metadata:** (docs commit to follow)

## Files Created/Modified

- `src/server.rs` - Added `key_service: Arc<KeyService>` field with `#[allow(dead_code)]`
- `src/main.rs` - Added `mod auth;`, KeyService construction, startup auth-mode log, updated AppState literal
- `src/auth.rs` - Added `#[allow(dead_code)]` to stub methods (create/list/revoke/validate)
- `tests/integration.rs` - Updated 2 AppState construction sites; added 5 new tests

## Decisions Made

1. **Per-item dead_code suppression for Phase 12 field** — Added `#[allow(dead_code)]` directly on the `key_service` field in AppState (consistent with Phase 10's per-item pattern). The field is wired but not yet read by any route handler until Phase 12 adds middleware.

2. **Bare module paths in main.rs** — Used `auth::KeyService::new()` (not `crate::auth::KeyService`) to match the existing convention in main.rs where all other services use bare paths (`service::MemoryService`, `compaction::CompactionService`, etc.).

3. **Stub method dead_code in auth.rs** — Added `#[allow(dead_code)]` to `create`, `list`, `revoke`, `validate` methods. These became newly visible as unused when KeyService was wired into the running binary.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality] Added #[allow(dead_code)] to KeyService stub methods**
- **Found during:** Task 1 verification (`cargo check`)
- **Issue:** When `auth::KeyService` became reachable via `AppState` in a binary context, the 4 unimplemented stub methods (`create`, `list`, `revoke`, `validate`) triggered `dead_code` warnings. Plan specifies zero warnings.
- **Fix:** Added per-item `#[allow(dead_code)]` on each stub method with a comment indicating which phase will implement it.
- **Files modified:** src/auth.rs
- **Verification:** `cargo check` produces only the pre-existing `MockSummarizer` warning
- **Committed in:** e535d2c (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 2 — missing critical: warning suppression for correctness)
**Impact on plan:** Required to meet the zero-warnings acceptance criterion. No scope creep.

## Issues Encountered

None — all tasks executed cleanly after the dead_code suppression fix.

## Known Stubs

| Stub | File | Reason |
|------|------|--------|
| `KeyService::create` | src/auth.rs | Phase 11 — crypto (BLAKE3, token gen) deferred |
| `KeyService::list` | src/auth.rs | Phase 11 — DB query deferred |
| `KeyService::revoke` | src/auth.rs | Phase 11 — soft-delete logic deferred |
| `KeyService::validate` | src/auth.rs | Phase 12 — hash comparison deferred |

These stubs are intentional. Phase 10's goal is the foundation; Phase 11 fills in the implementations.

## Next Phase Readiness

- AppState carries `key_service` ready for Phase 12 middleware to extract and use
- `KeyService::validate()` stub has the correct signature for Phase 12 to call
- `KeyService::count_active_keys()` proven working via test (returns 0 on fresh DB, confirming open mode)
- All 39 tests green — Phase 11 can build on this without regressions

## Self-Check: PASSED

Files exist:
- FOUND: src/server.rs (key_service field added)
- FOUND: src/main.rs (mod auth + KeyService construction + auth-mode log)
- FOUND: src/auth.rs (dead_code suppression on stubs)
- FOUND: tests/integration.rs (5 new tests + updated build helpers)

Commits exist:
- FOUND: e535d2c (feat(10-02): wire KeyService into AppState and add startup auth-mode log)
- FOUND: f986171 (test(10-02): update AppState construction and add auth migration/count tests)

---
*Phase: 10-auth-schema-foundation*
*Completed: 2026-03-20*
