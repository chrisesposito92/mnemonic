---
phase: 11-keyservice-core
verified: 2026-03-20T00:00:00Z
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 11: KeyService Core Verification Report

**Phase Goal:** Admin can create, list, and revoke API keys with secure hashing — and keys can be validated without exposing the raw token
**Verified:** 2026-03-20
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | create() returns (ApiKey, raw_token) where raw_token starts with mnk_ and is 68 chars | VERIFIED | `format!("mnk_{}", hex)` at auth.rs:42; token is 4 prefix + 64 hex = 68 chars; test_create_returns_raw_token passes |
| 2 | create() stores only the BLAKE3 hash in the database — raw token never persisted | VERIFIED | INSERT uses `hashed_key` column (auth.rs:92-94); `hash_token()` called before INSERT; test_create_stores_hash_not_raw asserts `hashed_key != raw_token` |
| 3 | display_id is first 8 chars of BLAKE3 hash hex — not a prefix of the raw token | VERIFIED | `hashed_key[..8]` at auth.rs:81; test_display_id_is_hash_derived explicitly asserts this AND asserts `display_id != raw_token[4..12]` |
| 4 | list() returns all keys (active and revoked) ordered by created_at DESC — never includes raw token or hashed_key | VERIFIED | SELECT at auth.rs:129 explicitly lists id, name, display_id, agent_id, created_at, revoked_at — no hashed_key; ORDER BY created_at DESC, id DESC; test_list_returns_all_keys creates 1 revoked + 1 active and asserts len == 2 |
| 5 | revoke() sets revoked_at via UPDATE — is idempotent (non-existent or already-revoked key returns Ok) | VERIFIED | `UPDATE api_keys SET revoked_at = CURRENT_TIMESTAMP WHERE id = ?1` at auth.rs:154; rows_affected ignored; test_revoke_idempotent passes with non-existent id |
| 6 | validate() hashes incoming token with BLAKE3, queries for matching active key, uses constant_time_eq_32 for comparison | VERIFIED | `hash_token(raw_token)` at auth.rs:166; single SQL query at auth.rs:173; `constant_time_eq_32(&incoming_bytes, stored_bytes)` at auth.rs:188 |
| 7 | validate() rejects revoked keys (WHERE revoked_at IS NULL in query) | VERIFIED | SQL at auth.rs:173 includes `AND revoked_at IS NULL`; test_revoke_prevents_validate and test_validate_rejects_revoked_key both pass |
| 8 | validate() returns AuthContext with correct key_id and allowed_agent_id | VERIFIED | Returns `AuthContext { key_id, allowed_agent_id: agent_id }` at auth.rs:189-192; test_validate_returns_auth_context asserts key_id and allowed_agent_id match |

**Score:** 8/8 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` | blake3, constant_time_eq, and rand dependencies | VERIFIED | blake3 = "1.8" (line 34), constant_time_eq = "0.4" (line 35), rand = { version = "0.9", features = ["std", "std_rng", "os_rng"] } (line 36) |
| `src/auth.rs` | KeyService with create, list, revoke, validate and unit tests | VERIFIED | 373 lines (min_lines: 200 satisfied); exports KeyService, ApiKey, AuthContext; 11 unit tests in #[cfg(test)] mod tests |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| src/auth.rs::create() | blake3::hash() | hash raw token for storage and display_id derivation | WIRED | `blake3::hash(raw.as_bytes())` at auth.rs:47; called from create() at auth.rs:79 |
| src/auth.rs::validate() | constant_time_eq::constant_time_eq_32() | compare incoming hash bytes with stored hash bytes | WIRED | `constant_time_eq_32(&incoming_bytes, stored_bytes)` at auth.rs:188 |
| src/auth.rs::create() | rand_core::OsRng | generate 32 cryptographically random bytes for token | WIRED | `use rand::rngs::OsRng` at auth.rs:9; `OsRng.try_fill_bytes(&mut bytes)` at auth.rs:40; called from generate_raw_token() which is called from create() |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| KEY-01 | 11-01-PLAN.md | Admin can create an API key with optional name and optional agent_id scope, receiving the raw key (mnk_...) exactly once | SATISFIED | create() generates mnk_-prefixed 68-char token; stores only BLAKE3 hash; returns raw token in tuple; tests: test_create_returns_raw_token, test_create_stores_hash_not_raw, test_create_with_name_and_scope all pass |
| KEY-02 | 11-01-PLAN.md | Admin can list all API keys showing name, prefix, scope, and creation date — never the full key | SATISFIED | list() SELECT returns id, name, display_id, agent_id, created_at, revoked_at; no hashed_key or raw token; ORDER BY created_at DESC, id DESC; test_list_returns_all_keys passes |
| KEY-03 | 11-01-PLAN.md | Admin can revoke a key, immediately preventing its use on subsequent requests | SATISFIED | revoke() uses UPDATE SET revoked_at; validate() excludes revoked keys via WHERE revoked_at IS NULL; test_revoke_prevents_validate passes; test_revoke_idempotent passes |
| KEY-04 | 11-01-PLAN.md | API key can be scoped to a specific agent_id, restricting access to only that agent's memories | SATISFIED | agent_id stored on key record; validate() returns AuthContext.allowed_agent_id from DB; test_validate_returns_auth_context and test_create_with_name_and_scope pass (scope enforcement at handler layer in Phase 13) |
| INFRA-02 | 11-01-PLAN.md | Key hashes use BLAKE3 with constant-time comparison to prevent timing attacks | SATISFIED | blake3::hash at auth.rs:47; constant_time_eq_32 at auth.rs:188; no == on hash values (grep confirmed zero matches); blake3::Hash::from_hex used to get [u8;32] before comparison |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| src/auth.rs | 13, 25 | #[allow(dead_code)] on ApiKey and AuthContext structs | Info | Structs are pub but not yet used in binary target — Phase 13/14 pending. Plan explicitly documents this as acceptable. Not a blocker. |

No stub patterns found:
- Zero `todo!()` occurrences
- Zero placeholder return values
- All four methods contain real SQL and business logic
- No `== ` comparisons on hash values

### Human Verification Required

None. All correctness properties are verifiable programmatically:
- Token format and length: verified by test + grep
- Hash storage: verified by direct DB query in test
- Constant-time comparison: verified by code structure (constant_time_eq_32 call confirmed)
- Test suite: 11/11 auth tests pass; 89/89 total tests pass

## Summary

Phase 11 achieves its goal. All four KeyService methods are fully implemented with real business logic — zero stubs remain. Security properties are intact:

- BLAKE3 hashing with raw token never persisted to DB
- display_id derived from hash (not raw token prefix), preventing pitfall 7
- Single SQL query for validation with revoked_at IS NULL filter
- constant_time_eq_32 called on [u8; 32] byte arrays — no == on hash values
- Idempotent revoke that ignores rows_affected

11 unit tests cover all five requirements (KEY-01 through KEY-04 and INFRA-02). Full test suite of 89 tests passes with zero auth.rs warnings. No regressions from the new blake3, constant_time_eq, and rand dependencies.

---
_Verified: 2026-03-20_
_Verifier: Claude (gsd-verifier)_
