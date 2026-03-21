---
phase: 12-auth-middleware
plan: 01
subsystem: auth
tags: [auth, middleware, axum, route_layer, bearer-token, integration-tests]
dependency_graph:
  requires: [src/auth.rs::KeyService, src/error.rs::ApiError]
  provides: [src/auth.rs::auth_middleware, src/server.rs::build_router-with-route_layer]
  affects: [all protected /memories/* routes, tests/integration.rs]
tech_stack:
  added: []
  patterns: [axum route_layer for middleware scoping, from_fn_with_state for stateful middleware]
key_files:
  created: []
  modified:
    - src/auth.rs
    - src/server.rs
    - src/error.rs
    - tests/integration.rs
decisions:
  - "Open mode = zero active keys = pass through unconditionally (D-05 from RESEARCH.md)"
  - "route_layer() not layer() — prevents 401 on unmatched routes (D-01)"
  - "Test for revoked-token requires a second active key to keep auth mode on; revoking the only key drops to open mode by design"
metrics:
  duration: 185s
  completed: "2026-03-21"
  tasks_completed: 2
  files_modified: 4
---

# Phase 12 Plan 01: Auth Middleware Implementation Summary

**One-liner:** Axum auth middleware with open-mode bypass, Bearer token validation, and health-route exemption via route_layer split.

## What Was Built

Implemented `auth_middleware` in `src/auth.rs` — a stateful axum middleware function that gates all protected `/memories/*` routes behind API key authentication. Restructured `build_router()` in `src/server.rs` to split routes into two groups: protected (with `route_layer`) and public (`/health` only). Added 6 integration tests proving all 5 success criteria.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Implement auth_middleware and restructure build_router | 789a573 | src/auth.rs, src/server.rs, src/error.rs |
| 2 | Add auth integration tests covering all 5 success criteria | e764bbd | tests/integration.rs |

## Implementation Details

### src/auth.rs — auth_middleware

Three-phase control flow:
1. **Open mode check**: `count_active_keys()` — if 0, pass through with no header inspection.
2. **Header parsing**: Extract `Authorization: Bearer <token>`, return 400 on malformed/missing.
3. **Token validation**: `key_service.validate()` — inject `AuthContext` on success, return 401 on failure.

DB error on count → 401 (fail safe, not passthrough).

### src/server.rs — build_router restructure

```rust
let protected = Router::new()
    .route("/memories", ...)
    .route("/memories/search", ...)
    .route("/memories/{id}", ...)
    .route("/memories/compact", ...)
    .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

let public = Router::new()
    .route("/health", get(health_handler));

Router::new().merge(protected).merge(public).with_state(state)
```

### Dead Code Cleanup

Removed `#[allow(dead_code)]` from:
- `key_service` field in `AppState` (server.rs)
- `Unauthorized` variant in `ApiError` (error.rs)
- `ApiKey` struct (auth.rs)
- `AuthContext` struct (auth.rs)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed revoked-token test to account for open mode semantics**
- **Found during:** Task 2 test execution
- **Issue:** `test_auth_revoked_token_rejects` created one key, revoked it, then expected 401. But revoking the only key returns `count_active_keys() = 0`, entering open mode — so the request passes (correct per D-05). The test was wrong, not the implementation.
- **Fix:** Create two keys. Revoke the first. Keep the second active so auth mode stays on. Revoked token now correctly gets 401.
- **Files modified:** tests/integration.rs
- **Commit:** e764bbd
- **RESEARCH.md reference:** Auth Pitfall 10 and D-05 confirm: open mode = zero enforcement.

## Verification Results

1. `cargo check` — exits 0, zero dead_code warnings on any auth-related items
2. `cargo test test_auth_` — all 6 new tests pass
3. `cargo test` (full suite) — 45 passed, 0 failed, 1 ignored (openai live test)
4. No `#[allow(dead_code)]` on `key_service`, `Unauthorized`, `ApiKey`, or `AuthContext`
5. `/health` returns 200 without auth via structural exemption (separate Router, route_layer not applied)
6. Protected endpoints require auth when keys exist (proven by test_auth_invalid_token_rejects, test_auth_revoked_token_rejects)

## Known Stubs

None — all behaviors are fully wired. Auth middleware is live, not mocked.

## Self-Check: PASSED
