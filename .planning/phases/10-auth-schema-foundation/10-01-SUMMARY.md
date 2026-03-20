---
phase: 10-auth-schema-foundation
plan: 01
subsystem: auth
tags: [auth, schema, migration, error-handling, rust]
dependency_graph:
  requires: []
  provides:
    - api_keys DDL (idempotent, safe on v1.1 databases)
    - ApiError::Unauthorized(String) variant with structured 401 response
    - KeyService struct with count_active_keys() real implementation
    - AuthContext and ApiKey structs
    - pub mod auth in lib.rs
  affects:
    - src/db.rs (schema init)
    - src/error.rs (ApiError enum and IntoResponse)
    - src/lib.rs (module declarations)
tech_stack:
  added: []
  patterns:
    - idempotent DDL via CREATE TABLE IF NOT EXISTS inside execute_batch
    - per-item #[allow(dead_code)] for Phase 10 stubs (not module-level)
    - per-variant IntoResponse body handling (Option A refactor)
    - conn.call() pattern for async SQLite
key_files:
  created:
    - src/auth.rs
  modified:
    - src/db.rs
    - src/error.rs
    - src/lib.rs
decisions:
  - "Used per-item #[allow(dead_code)] on ApiKey and AuthContext (not module-level) per plan spec"
  - "Added #[allow(dead_code)] to Unauthorized variant in error.rs (no callers until Phase 12)"
  - "Refactored IntoResponse to per-variant body handling (Option A) — cleaner than if-let workaround"
  - "Deferred last_used_at column and idx_api_keys_hashed_key index per plan — UNIQUE constraint provides implicit index"
metrics:
  duration: 115s
  completed: "2026-03-20"
  tasks_completed: 2
  files_modified: 4
---

# Phase 10 Plan 01: Auth Schema Foundation Summary

Auth schema foundation with api_keys DDL, Unauthorized error variant (HTTP 401 with auth_mode+hint JSON), and KeyService skeleton with real count_active_keys() query using conn.call() pattern.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add api_keys DDL to db.rs and Unauthorized variant to error.rs | 6bd9f82 | src/db.rs, src/error.rs |
| 2 | Create auth.rs module skeleton and declare in lib.rs | bfeb428 | src/auth.rs, src/lib.rs |

## Verification Results

- `cargo check` exits 0 (only pre-existing `MockSummarizer` warning, introduced before this plan)
- `src/db.rs` contains `CREATE TABLE IF NOT EXISTS api_keys` inside `execute_batch`
- `src/db.rs` has 7 columns: id, name, display_id, hashed_key (UNIQUE), agent_id, created_at, revoked_at
- `src/db.rs` has 1 explicit index (`idx_api_keys_agent_id`); hashed_key uniqueness via UNIQUE constraint (no redundant explicit index)
- `src/db.rs` does NOT contain `last_used_at` or `idx_api_keys_hashed_key`
- `src/error.rs` has `Unauthorized(String)` variant returning HTTP 401 with `error`, `auth_mode`, `hint` fields
- `src/error.rs` does NOT contain `Forbidden`
- `src/auth.rs` exists with `ApiKey`, `AuthContext`, `KeyService` structs
- `src/auth.rs` has per-item `#[allow(dead_code)]` on `ApiKey` and `AuthContext` (not module-level `#![]`)
- `src/auth.rs` has real `count_active_keys()` using `SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL`
- `src/auth.rs` has four `todo!("Phase 11: ...")` stub methods with concrete return types
- `src/auth.rs` does NOT contain `blake3` or `constant_time_eq`
- `src/lib.rs` has `pub mod auth;` as first declaration (alphabetical)

## Decisions Made

1. **Per-item dead_code suppression in error.rs** — Added `#[allow(dead_code)]` to the `Unauthorized` variant itself (not the whole enum) since it has no callers in Phase 10. This matches the plan's per-item pattern specified for auth.rs.

2. **IntoResponse refactor (Option A)** — Rewrote the match to handle each variant individually and produce its own JSON body. The original two-step `(status, message)` pattern could not accommodate the richer `auth_mode` + `hint` body for `Unauthorized` without awkward workarounds.

3. **No last_used_at column** — Deferred to Phase 11 per plan spec and research resolution. Adding the column without the write path (validate function) creates dead schema.

4. **No explicit idx_api_keys_hashed_key index** — The `UNIQUE` constraint on `hashed_key` creates an implicit index in SQLite. Adding an explicit index would be redundant, waste storage, and slow writes.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality] Added #[allow(dead_code)] to Unauthorized variant**
- **Found during:** Task 1 verification (`cargo check`)
- **Issue:** `ApiError::Unauthorized` variant generated a dead_code warning since it has no callers in Phase 10. The plan specified zero warnings.
- **Fix:** Added `#[allow(dead_code)]` attribute directly on the `Unauthorized` variant (per-item suppression, consistent with the per-item pattern mandated for auth.rs structs).
- **Files modified:** src/error.rs
- **Commit:** 6bd9f82

## Known Stubs

| Stub | File | Reason |
|------|------|--------|
| `KeyService::create` | src/auth.rs:61 | Phase 11 implementation — crypto (BLAKE3, token gen) deferred |
| `KeyService::list` | src/auth.rs:69 | Phase 11 implementation — DB query deferred |
| `KeyService::revoke` | src/auth.rs:75 | Phase 11 implementation — soft-delete logic deferred |
| `KeyService::validate` | src/auth.rs:81 | Phase 11 implementation — hash comparison deferred |

These stubs are intentional. They define the contract (return types, signatures) that Phase 11 will implement. No plan goal is blocked by these stubs — Phase 10's goal is exactly to establish this foundation.

## Self-Check: PASSED

Files exist:
- FOUND: src/auth.rs
- FOUND: src/db.rs (modified)
- FOUND: src/error.rs (modified)
- FOUND: src/lib.rs (modified)

Commits exist:
- FOUND: 6bd9f82 (feat(10-01): add api_keys DDL and Unauthorized error variant)
- FOUND: bfeb428 (feat(10-01): create auth.rs skeleton and declare pub mod auth in lib.rs)
