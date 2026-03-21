---
phase: 13-http-wiring-and-rest-key-endpoints
verified: 2026-03-20T00:00:00Z
status: passed
score: 13/13 must-haves verified
re_verification: false
---

# Phase 13: HTTP Wiring and REST Key Endpoints — Verification Report

**Phase Goal:** Key management is accessible via REST, auth middleware is attached to all protected routes, and scoped keys enforce namespace isolation at the handler layer
**Verified:** 2026-03-20
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

All truths are drawn from the combined must_haves of Plan 01 and Plan 02.

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A scoped key for agent-x used with agent_id agent-y in the request returns 403 Forbidden | VERIFIED | `test_scope_mismatch_returns_403` passes; `enforce_scope` in `src/server.rs:88` returns `ApiError::Forbidden`; `ApiError::Forbidden` maps to `StatusCode::FORBIDDEN` at `src/error.rs:112-118` |
| 2 | A scoped key with no agent_id in the request forces the key's scope as agent_id | VERIFIED | `test_scope_forces_agent_id` passes; `enforce_scope` line 86 returns `Ok(Some(allowed.clone()))` when `requested` is `None` |
| 3 | A wildcard key passes through any client-supplied agent_id without restriction | VERIFIED | `test_wildcard_key_passes_through` passes; `enforce_scope` line 84 returns `Ok(requested.map(str::to_string))` when `allowed_agent_id` is `None` |
| 4 | Open mode (no AuthContext) behaves exactly as before with no scope enforcement | VERIFIED | `enforce_scope` line 82 returns `Ok(None)` when `auth` is `None`; all pre-existing 45 tests continue to pass |
| 5 | DELETE /memories/{id} with a scoped key checks memory ownership and returns 403 if mismatched | VERIFIED | `test_scoped_delete_wrong_owner_403` passes; `delete_memory_handler` calls `get_memory_agent_id` and returns `ApiError::Forbidden` on mismatch at `src/server.rs:158-161` |
| 6 | POST /keys creates a key and returns 201 with raw_token shown once | VERIFIED | `test_post_keys_creates_key` passes; `create_key_handler` returns `StatusCode::CREATED` with `raw_token` field; token never logged |
| 7 | GET /keys returns all key metadata with no raw token values | VERIFIED | `test_get_keys_no_raw_token` passes; `list_keys_handler` constructs response with only `id`, `name`, `display_id`, `agent_id`, `created_at`, `revoked_at` fields |
| 8 | DELETE /keys/:id revokes a key and subsequent requests with that key return 401 | VERIFIED | `test_delete_key_revokes_access` passes; `revoke_key_handler` calls `key_service.revoke`; subsequent request with revoked token yields 401 |
| 9 | Key endpoints are protected by auth middleware (same route_layer as /memories) | VERIFIED | `/keys` and `/keys/{id}` routes appear before `.route_layer(` at `src/server.rs:54-56`; same `route_layer` wraps all protected routes |
| 10 | Scoped key + mismatched agent_id returns 403 in integration test | VERIFIED | `test_scope_mismatch_returns_403` passes with `StatusCode::FORBIDDEN` assertion and body `{"error": "forbidden", "detail": "..."}` |
| 11 | Scoped key + missing agent_id forces scope in integration test | VERIFIED | `test_scope_forces_agent_id` passes; memory saved with `agent_id = "agent-A"` |
| 12 | Wildcard key passes through any agent_id in integration test | VERIFIED | `test_wildcard_key_passes_through` passes; response `agent_id == "any-agent"` |
| 13 | Scoped key DELETE of another agent's memory returns 403 in integration test | VERIFIED | `test_scoped_delete_wrong_owner_403` passes with `StatusCode::FORBIDDEN` |

**Score:** 13/13 truths verified

---

### Required Artifacts

#### Plan 01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/error.rs` | `ApiError::Forbidden(String)` variant with 403 status and `{error, detail}` body | VERIFIED | Lines 87-88: `Forbidden(String)` variant; lines 112-118: `StatusCode::FORBIDDEN` with `json!({"error": "forbidden", "detail": detail})` |
| `src/service.rs` | `get_memory_agent_id` method for ownership check | VERIFIED | Line 298: `pub async fn get_memory_agent_id(&self, id: &str) -> Result<Option<String>, ApiError>`; queries `SELECT agent_id FROM memories WHERE id = ?1` |
| `src/server.rs` | `enforce_scope` helper + all 5 modified handlers with `Option<Extension<AuthContext>>` | VERIFIED | Lines 77-94: `fn enforce_scope`; lines 104, 119, 134, 149, 176: all 5 handlers have `auth: Option<Extension<AuthContext>>` parameter |

#### Plan 02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/server.rs` | `create_key_handler`, `list_keys_handler`, `revoke_key_handler` + router wiring | VERIFIED | Lines 203-252: all three handlers exist and compile; lines 54-55: `/keys` and `/keys/{id}` wired before `route_layer` |
| `tests/integration.rs` | 8 new integration tests for scope enforcement and key endpoints | VERIFIED | Lines 1720, 1747, 1790, 1815, 1855, 1895, 1919, 1976: all 8 test functions present; all 8 pass |

---

### Key Link Verification

#### Plan 01 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/server.rs::enforce_scope` | `src/error.rs::ApiError::Forbidden` | `returns Err(ApiError::Forbidden(...))` | WIRED | `src/server.rs:88`: `Err(crate::error::ApiError::Forbidden(format!(...)))` |
| `src/server.rs::delete_memory_handler` | `src/service.rs::get_memory_agent_id` | ownership lookup before delete | WIRED | `src/server.rs:156`: `state.service.get_memory_agent_id(&id).await?` |
| `src/server.rs::*_handler` | `src/auth.rs::AuthContext` | `Option<Extension<AuthContext>>` extractor | WIRED | All 5 handlers use `auth: Option<Extension<AuthContext>>` parameter; `use crate::auth::{auth_middleware, AuthContext}` at line 13 |

#### Plan 02 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/server.rs::create_key_handler` | `src/auth.rs::KeyService::create` | `state.key_service.create(body.name, body.agent_id)` | WIRED | `src/server.rs:207`: `state.key_service.create(body.name, body.agent_id).await` |
| `src/server.rs::build_router` | `src/server.rs::create_key_handler` | `.route("/keys", post(create_key_handler).get(list_keys_handler))` | WIRED | `src/server.rs:54`: exact pattern present |
| `tests/integration.rs` | `src/server.rs::enforce_scope` | HTTP requests with scoped tokens triggering 403 | WIRED | `test_scope_mismatch_returns_403` and `test_scoped_delete_wrong_owner_403` assert `StatusCode::FORBIDDEN` |

---

### Requirements Coverage

Both plans declare `requirements: [AUTH-04, INFRA-03]`.

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| AUTH-04 | 13-01, 13-02 | A scoped key's agent_id overrides the client-supplied agent_id, preventing cross-agent access | SATISFIED | `enforce_scope` implements the three-way logic (open/wildcard/scoped); 5 integration tests prove all branches; `test_scope_mismatch_returns_403`, `test_scope_forces_agent_id`, `test_wildcard_key_passes_through`, `test_scoped_delete_wrong_owner_403`, `test_scoped_delete_own_memory_ok` all pass |
| INFRA-03 | 13-01, 13-02 | Server startup log announces whether running in open or authenticated mode | SATISFIED (no regression) | `src/main.rs:109-116`: startup log `"Auth: OPEN (no keys)"` / `"Auth: ACTIVE ({n} keys)"` remains untouched; phase 13 made no changes to `src/main.rs`; full 53-test suite passes with no regressions |

**REQUIREMENTS.md traceability table** maps AUTH-04 to Phase 13 and INFRA-03 to Phase 10. Phase 13 claims INFRA-03 as a no-regression check, not as the primary implementation phase — this is consistent with REQUIREMENTS.md which marks INFRA-03 as Complete under Phase 10.

No orphaned requirements: both AUTH-04 and INFRA-03 are fully accounted for.

---

### Anti-Patterns Found

No anti-patterns detected.

Scanned files: `src/error.rs`, `src/service.rs`, `src/server.rs`, `tests/integration.rs`

- No TODO/FIXME/XXX/HACK/PLACEHOLDER comments
- No stub implementations (`return null`, empty handlers, hardcoded empty arrays)
- No logging of `raw_token` (comment explicitly warns against it at `src/server.rs:209`)
- No `hashed_key` exposed in list response (test asserts its absence)

---

### Human Verification Required

None — all behaviors are covered by integration tests that run against the actual HTTP stack, including 403/401/201 status codes, response body shape, and token revocation behavior. No UI, real-time, or external-service concerns in this phase.

---

### Commits Verified

All commits claimed in SUMMARY files exist in git log:

| Commit | Description | Plan |
|--------|-------------|------|
| `0f10506` | feat(13-01): add ApiError::Forbidden variant and get_memory_agent_id method | 13-01 Task 1 |
| `3ef3858` | feat(13-01): add enforce_scope helper and scope enforcement to all 5 handlers | 13-01 Task 2 |
| `a484a24` | feat(13-02): add key management handlers and wire into protected router | 13-02 Task 1 |
| `e9b22b0` | test(13-02): add 8 integration tests for scope enforcement and key endpoints | 13-02 Task 2 |

---

### Test Suite Results

- Full suite: **53 passed, 0 failed, 1 ignored** (8.10s)
- Targeted scope/key tests: **8 passed, 0 failed**
- No regressions vs Phase 12 baseline (45 tests)

---

_Verified: 2026-03-20_
_Verifier: Claude (gsd-verifier)_
