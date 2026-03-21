---
phase: 11-keyservice-core
plan: 01
subsystem: auth
tags: [blake3, constant_time_eq, rand, rusqlite, tokio-rusqlite, api-keys, hashing]

# Dependency graph
requires:
  - phase: 10-auth-schema-foundation
    provides: api_keys table DDL, KeyService struct stubs, ApiKey/AuthContext structs, count_active_keys()
provides:
  - KeyService::create() — generates mnk_ token, stores BLAKE3 hash, returns raw token once
  - KeyService::list() — all keys (active + revoked) ordered by created_at DESC
  - KeyService::revoke() — idempotent soft-delete via UPDATE revoked_at = CURRENT_TIMESTAMP
  - KeyService::validate() — BLAKE3 hash + constant_time_eq_32 comparison, rejects revoked keys
  - generate_raw_token() helper — OsRng 32 bytes encoded as mnk_ + 64 hex chars
  - hash_token() helper — BLAKE3 hash of raw token bytes
affects: [12-auth-middleware, 13-key-http-endpoints, 14-cli-key-management]

# Tech tracking
tech-stack:
  added:
    - blake3 1.8 — BLAKE3 hashing for key storage and display_id derivation
    - constant_time_eq 0.4 — constant_time_eq_32() for timing-safe hash comparison
    - rand 0.9 (features std, std_rng, os_rng) — OsRng cryptographic random bytes
  patterns:
    - Raw token never persisted — only BLAKE3 hash stored in hashed_key column
    - display_id derived from hash[..8], not from raw token prefix (avoids pitfall 7)
    - Single SQL query for validate (WHERE hashed_key = ?1 AND revoked_at IS NULL)
    - Constant-time comparison via constant_time_eq_32 on [u8; 32] — never == on hashes
    - Idempotent revoke — UPDATE ignores rows_affected, returns Ok(()) unconditionally

key-files:
  created: []
  modified:
    - Cargo.toml — blake3 1.8, constant_time_eq 0.4, rand 0.9 dependencies added
    - src/auth.rs — all four KeyService methods implemented plus 11 unit tests

key-decisions:
  - "rand::rand_core re-export path used (not standalone rand_core crate) for OsRng import with rand 0.9"
  - "ORDER BY created_at DESC, id DESC for deterministic list ordering when timestamps collide within same second"
  - "display_id = hashed_key[..8] (first 8 hex chars of BLAKE3 hash) — confirmed NOT raw_token[4..12]"

patterns-established:
  - "Token generation: OsRng.try_fill_bytes(&mut [0u8; 32]) -> hex -> format!(mnk_{})"
  - "Validation query: single WHERE hashed_key = ?1 AND revoked_at IS NULL, then constant_time_eq_32"
  - "Blake3 decode for comparison: blake3::Hash::from_hex(&stored_hex).unwrap().as_bytes()"

requirements-completed: [KEY-01, KEY-02, KEY-03, KEY-04, INFRA-02]

# Metrics
duration: 4min
completed: 2026-03-21
---

# Phase 11 Plan 01: KeyService Core Summary

**KeyService with BLAKE3 hashing, OsRng token generation, and constant_time_eq_32 validation — all four methods (create/list/revoke/validate) fully implemented with 11 unit tests**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-21T01:21:37Z
- **Completed:** 2026-03-21T01:25:46Z
- **Tasks:** 2
- **Files modified:** 3 (Cargo.toml, Cargo.lock, src/auth.rs)

## Accomplishments

- Implemented all four KeyService methods replacing todo!() stubs — zero stubs remain
- Cryptographic token generation: OsRng produces 32 random bytes encoded as mnk_ + 64 hex chars (68 total)
- BLAKE3 hashing for key storage — raw token never persisted, only 64-char hash in hashed_key column
- display_id derived from first 8 chars of BLAKE3 hash (not raw token prefix — avoids pitfall 7)
- constant_time_eq_32() for validate() comparison — never == on hash byte arrays (INFRA-02)
- Validate rejects revoked keys via single SQL query (WHERE revoked_at IS NULL)
- Revoke is idempotent — non-existent or already-revoked ID always returns Ok(())
- 11 unit tests covering all KEY-01..KEY-04 and INFRA-02 requirements — all passing
- Zero regressions: full suite 89 tests passing (46 lib + 39 integration + 4 unit)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add crate dependencies and implement KeyService methods with helpers** - `4f833de` (feat)
2. **Task 2: Verify full test suite passes and no regressions** — verified (no code changes needed)

**Plan metadata:** (committed in final docs commit)

## Files Created/Modified

- `Cargo.toml` — Added blake3 1.8, constant_time_eq 0.4, rand 0.9 dependencies
- `Cargo.lock` — Updated with new dependency trees
- `src/auth.rs` — Full KeyService implementation (generate_raw_token, hash_token, create, list, revoke, validate) plus 11 unit tests in #[cfg(test)] mod tests

## Decisions Made

- Used `rand::rand_core::{OsRng, TryRngCore}` re-export path (not standalone `rand_core` crate) since rand 0.9 re-exports rand_core internally
- Added `ORDER BY created_at DESC, id DESC` for deterministic list ordering — UUID v7 IDs are monotonically increasing so secondary sort on id handles same-second timestamps
- Kept `#[allow(dead_code)]` on structs (ApiKey, AuthContext) and helpers — methods are `pub` and exercised by tests but binary target hasn't wired them yet (Phase 13/14). Per plan: no dead_code on methods themselves.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed rand import path for rand 0.9**
- **Found during:** Task 1 (compilation)
- **Issue:** Plan specified `use rand_core::{OsRng, TryRngCore}` but rand 0.9 doesn't make rand_core a top-level dependency — it re-exports via `rand::rand_core`
- **Fix:** Changed import to `use rand::rand_core::{OsRng, TryRngCore}`
- **Files modified:** src/auth.rs
- **Verification:** cargo test --lib auth passes
- **Committed in:** 4f833de (Task 1 commit)

**2. [Rule 1 - Bug] Fixed list ordering for same-second timestamps**
- **Found during:** Task 1 (test_list_returns_all_keys failed)
- **Issue:** SQLite CURRENT_TIMESTAMP precision is 1 second — two keys created back-to-back get identical created_at, causing non-deterministic ORDER BY created_at DESC ordering
- **Fix:** Changed ORDER BY to `created_at DESC, id DESC` — UUID v7 IDs are time-ordered so secondary sort is deterministic
- **Files modified:** src/auth.rs
- **Verification:** test_list_returns_all_keys now passes consistently
- **Committed in:** 4f833de (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 1 - Bug)
**Impact on plan:** Both fixes necessary for correctness. No scope creep. All security properties intact (BLAKE3, constant-time, one-time token display).

## Issues Encountered

None beyond the two auto-fixed deviations above.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- KeyService fully operational — create/list/revoke/validate all work correctly
- Phase 12 (auth middleware) can call KeyService::validate() to check Bearer tokens
- Phase 13 (HTTP endpoints) can call create/list/revoke for key management API
- Phase 14 (CLI) can call the same KeyService methods for `mnemonic keys` commands
- No blockers

## Known Stubs

None — all KeyService methods fully implemented with real business logic.

---
*Phase: 11-keyservice-core*
*Completed: 2026-03-21*
