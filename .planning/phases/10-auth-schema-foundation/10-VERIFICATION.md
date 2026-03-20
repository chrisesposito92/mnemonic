---
phase: 10-auth-schema-foundation
verified: 2026-03-20T20:15:00Z
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 10: Auth Schema Foundation Verification Report

**Phase Goal:** Create the api_keys table, error variant, auth module skeleton, and wire into AppState with startup auth-mode logging. Foundation for all downstream auth phases (11-14).
**Verified:** 2026-03-20T20:15:00Z
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Server starts cleanly on an existing v1.1 database with no migration errors | VERIFIED | `CREATE TABLE IF NOT EXISTS api_keys` inside `execute_batch` in `src/db.rs:77`; `CREATE INDEX IF NOT EXISTS idx_api_keys_agent_id` at line 87; idempotency test `test_api_keys_migration_idempotent` passes |
| 2 | api_keys table exists with 7 columns after db::open() | VERIFIED | All 7 columns present in `src/db.rs:78-85`: id, name, display_id, hashed_key, agent_id, created_at, revoked_at; `test_api_keys_table_created` asserts exact column count and names |
| 3 | ApiError::Unauthorized returns HTTP 401 with structured JSON body | VERIFIED | `src/error.rs:103-109` returns `StatusCode::UNAUTHORIZED` with `{"error":"unauthorized","auth_mode":"active","hint":"Provide Authorization: Bearer mnk_..."}` body; `test_unauthorized_response_shape` passes |
| 4 | auth module compiles with AuthContext, ApiKey, KeyService structs and count_active_keys() | VERIFIED | `src/auth.rs` contains all three structs; `count_active_keys()` is a real SQL query at lines 42-53; `cargo check` passes; zero new warnings |
| 5 | Server startup log prints whether running in open mode or authenticated mode | VERIFIED | `src/main.rs:109-121` matches on `count_active_keys().await` and logs `"Auth: OPEN (no keys)"` or `"Auth: ACTIVE ({n} keys)"` or warns on DB error |
| 6 | Server starts cleanly on an existing v1.1 database (AppState includes key_service) | VERIFIED | `src/server.rs:31` has `pub key_service: std::sync::Arc<crate::auth::KeyService>`; `src/main.rs:142-146` constructs AppState with all three fields; full 39-test suite passes |
| 7 | count_active_keys() returns 0 on a fresh database | VERIFIED | `test_count_active_keys_empty_db` asserts count equals 0; passes |
| 8 | Existing tests pass after AppState gains key_service field | VERIFIED | Both `build_test_state()` (line 624) and `build_test_compact_state()` (line 1311) include `key_service`; `cargo test` reports 39 passed, 0 failed |

**Score:** 8/8 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/db.rs` | api_keys DDL block inside execute_batch | VERIFIED | Lines 76-87: 7-column table + idx_api_keys_agent_id; UNIQUE constraint on hashed_key provides implicit index; no `last_used_at`; no `idx_api_keys_hashed_key` |
| `src/error.rs` | Unauthorized variant on ApiError | VERIFIED | Lines 85-87: `#[allow(dead_code)] Unauthorized(String)`; IntoResponse returns 401 with `auth_mode` and `hint` fields; no `Forbidden` |
| `src/auth.rs` | KeyService, ApiKey, AuthContext structs with count_active_keys() real impl | VERIFIED | 85 lines; all three structs present; per-item `#[allow(dead_code)]` on ApiKey and AuthContext; `count_active_keys()` runs `SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL` via `conn.call()`; four stub methods with `todo!("Phase 11: ...")` |
| `src/lib.rs` | auth module declaration | VERIFIED | Line 1: `pub mod auth;` — first entry, alphabetical |
| `src/server.rs` | AppState with key_service field | VERIFIED | Line 31: `pub key_service: std::sync::Arc<crate::auth::KeyService>` with `#[allow(dead_code)]` |
| `src/main.rs` | KeyService construction and startup auth-mode log | VERIFIED | Lines 104-121: constructs `auth::KeyService::new(db_arc.clone())`, matches on `count_active_keys().await`, logs INFO for OPEN/ACTIVE and WARN on DB error; bare path `auth::KeyService` (not `crate::auth::KeyService`) matching existing convention |
| `tests/integration.rs` | Updated AppState construction and new auth migration tests | VERIFIED | `build_test_state()` line 624 and `build_test_compact_state()` line 1311 include `key_service`; all 5 new tests present |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `src/auth.rs` | `src/db.rs` | `conn.call()` for count_active_keys query | VERIFIED | `src/auth.rs:44-50`: `self.conn.call(|c| -> Result<i64, rusqlite::Error> { c.query_row("SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL", ...) })` |
| `src/error.rs` | axum::response::IntoResponse | Unauthorized match arm returning 401 | VERIFIED | `src/error.rs:103-110`: `ApiError::Unauthorized(_) => (axum::http::StatusCode::UNAUTHORIZED, serde_json::json!({...}))` with `StatusCode::UNAUTHORIZED` |
| `src/main.rs` | `src/auth.rs` | `KeyService::new()` and `count_active_keys()` | VERIFIED | `src/main.rs:105-121`: `auth::KeyService::new(db_arc.clone())` then `.count_active_keys().await` |
| `src/main.rs` | startup log | `tracing::info!` based on count_active_keys result | VERIFIED | Lines 110-120: `tracing::info!("Auth: OPEN...")` and `tracing::info!(keys = n, "Auth: ACTIVE...")` |
| `src/server.rs` | `src/auth.rs` | `Arc<KeyService>` field on AppState | VERIFIED | `src/server.rs:31`: `pub key_service: std::sync::Arc<crate::auth::KeyService>` |
| `tests/integration.rs` | `src/auth.rs` | `KeyService::new()` in build_test_state | VERIFIED | Lines 624 and 1311: `Arc::new(mnemonic::auth::KeyService::new(db.clone()))` |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| INFRA-01 | 10-01-PLAN.md | api_keys table created via idempotent SQLite migration on startup | SATISFIED | `CREATE TABLE IF NOT EXISTS api_keys` in `src/db.rs:77`; `test_api_keys_table_created` and `test_api_keys_migration_idempotent` pass |
| INFRA-03 | 10-02-PLAN.md | Server startup log announces open or authenticated mode | SATISFIED | `src/main.rs:109-121` logs "Auth: OPEN" or "Auth: ACTIVE" based on live DB query at startup |

No orphaned requirements: REQUIREMENTS.md traceability table maps exactly INFRA-01 and INFRA-03 to Phase 10. Both are marked Complete in REQUIREMENTS.md.

---

## Anti-Patterns Found

None. Scanned `src/db.rs`, `src/error.rs`, `src/auth.rs`, `src/server.rs`, `src/main.rs`, `tests/integration.rs` for:
- Module-level `#![allow(dead_code)]` — not found; only per-item `#[allow(dead_code)]` used (on `ApiKey`, `AuthContext`, `Unauthorized`, `key_service` field, and the 4 stub methods)
- Excluded columns/indexes (`last_used_at`, `idx_api_keys_hashed_key`) — not found
- Excluded crypto deps (`blake3`, `constant_time_eq`) — not found
- Excluded variant (`Forbidden`) — not found
- Pre-existing warning: `MockSummarizer` in `src/summarization.rs:169` — pre-exists Phase 10, not introduced by this phase; the SUMMARY documents this explicitly

The only `todo!()` occurrences in phase files are the four intentional Phase 11 stubs (`KeyService::create`, `list`, `revoke`, `validate`) — these are documented as known stubs in both SUMMARYs and are not blockers for Phase 10's goal.

---

## Human Verification Required

None. All phase 10 behaviors are verifiable programmatically:
- DDL presence and idempotency verified by integration tests
- HTTP 401 response shape verified by `test_unauthorized_response_shape`
- Startup log verified by code inspection (branches on count_active_keys result)
- Compilation and zero-regression confirmed by `cargo test` (39 passed, 0 failed)

The one item that requires runtime observation — "startup log actually prints at boot" — is covered structurally: the code path is not guarded by a feature flag or config, executes unconditionally after DB open, and `test_count_active_keys_empty_db` proves the underlying query works.

---

## Summary

Phase 10 goal fully achieved. All 8 observable truths verified, all 7 artifacts substantive and wired, all 6 key links confirmed, both requirement IDs (INFRA-01, INFRA-03) satisfied. The 4 `todo!()` stubs in `src/auth.rs` are intentional — Phase 10's explicit goal is the foundation skeleton; Phase 11 fills in the crypto implementations. The full 39-test suite is green with zero new warnings introduced by this phase.

---

_Verified: 2026-03-20T20:15:00Z_
_Verifier: Claude (gsd-verifier)_
