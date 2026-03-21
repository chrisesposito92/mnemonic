# Phase 12: Auth Middleware - Research

**Researched:** 2026-03-20
**Domain:** Axum middleware — Bearer token authentication with open mode, health exemption, and AuthContext injection
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-01:** Use `route_layer()` on the protected route group, NOT `layer()` on the entire Router — prevents 401 on unmatched routes.

**D-02:** Split `build_router()` into two groups: public routes (`/health`) registered first, then protected routes (all `/memories/*` endpoints) with auth middleware via `route_layer()`.

**D-03:** Middleware is an `axum::middleware::from_fn_with_state()` — receives `AppState` to access `key_service`.

**D-04:** Open mode = per-request `COUNT(*)` query via `KeyService::count_active_keys()` — NOT a cached flag or startup check.

**D-05:** When `count_active_keys()` returns 0, middleware passes request through without checking Authorization header — no `AuthContext` injected.

**D-06:** When keys are created (Phase 13), auth activates on the next request automatically — no server restart needed.

**D-07:** `/health` is registered as a separate route group BEFORE the auth middleware layer — axum's `route_layer()` only applies to routes defined in the same Router segment.

**D-08:** No skip-logic inside the middleware — the route architecture handles exemption cleanly.

**D-09:** Malformed `Authorization` header (not `Bearer <token>` format) returns 400 Bad Request.

**D-10:** Missing `Authorization` header when auth is active returns 401 Unauthorized.

**D-11:** Invalid or revoked token returns 401 Unauthorized (delegates to `KeyService::validate()`).

**D-12:** On successful validation, inject `AuthContext` into request extensions via `request.extensions_mut().insert(auth_context)`.

**D-13:** 401 response uses existing `ApiError::Unauthorized(String)`.

**D-14:** 400 response for malformed headers uses existing `ApiError::BadRequest(String)`.

**Auth Pitfall 10 (locked):** In open mode, a request with an INVALID token should still pass through — open mode means no auth enforcement at all. Only malformed headers return 400. Missing header in open mode passes through.

### Claude's Discretion

- Exact middleware function signature and internal control flow
- Whether to extract header parsing into a helper function
- Test organization (inline `#[cfg(test)]` vs integration tests)
- Whether the `#[allow(dead_code)]` on `key_service` in AppState should be removed in this phase or Phase 13

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AUTH-01 | Requests with a valid Bearer token in the Authorization header are authenticated | `KeyService::validate()` already implemented; middleware calls it and injects `AuthContext` into extensions |
| AUTH-02 | Requests with an invalid or revoked token receive 401 Unauthorized | `validate()` returns `Err` for invalid/revoked; middleware converts to `ApiError::Unauthorized` |
| AUTH-03 | When no API keys exist in the database, all requests are allowed (open mode) | `count_active_keys()` returns `i64`; 0 = open mode; middleware passes through without any header inspection |
| AUTH-05 | GET /health is accessible without authentication regardless of auth mode | `route_layer()` only applies to routes in the same Router segment; `/health` in a separate segment never hits auth middleware |
</phase_requirements>

---

## Summary

Phase 12 adds an axum authentication middleware to `mnemonic`. All the foundational pieces are already in place from Phases 10 and 11: `KeyService::validate()` and `KeyService::count_active_keys()` are implemented and tested in `src/auth.rs`; `ApiError::Unauthorized` and `ApiError::BadRequest` have their `IntoResponse` impls in `src/error.rs`; `AppState.key_service` is wired in `src/server.rs` (with `#[allow(dead_code)]`); and the integration test harness in `tests/integration.rs` already creates `KeyService` in `build_test_state()`.

The core work is: (1) implement `auth_middleware` as an `async fn` in `src/auth.rs`, (2) restructure `build_router()` in `src/server.rs` to separate `/health` (public) from `/memories/*` (protected via `route_layer()`), and (3) write integration tests covering all five success criteria. No new dependencies are needed — `axum::middleware`, `axum::Extension`, and the existing error types cover everything.

The one subtle behavior to get right is the open-mode-with-token edge case (PITFALLS.md Auth Pitfall 10): open mode means zero auth enforcement — even a syntactically valid but wrong token passes through. Only malformed headers (not `Bearer <token>` format) return 400, regardless of auth mode. This is different from the description in some research docs that says "attempt validation on wrong keys even in open mode" — the CONTEXT.md decision D-05 is authoritative: when count is 0, pass through without checking the header at all.

**Primary recommendation:** Add `auth_middleware` to `src/auth.rs`, restructure `build_router()` with nested Router + `route_layer()`, then add 5 targeted integration tests to `tests/integration.rs`.

---

## Standard Stack

### Core (already in Cargo.toml — no new dependencies needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `axum` | 0.8 | `middleware::from_fn_with_state`, `Extension`, `route_layer` | Already in project; official axum middleware APIs |
| `tower` | 0.5 (dev-dep) | `ServiceExt::oneshot` for integration tests | Already a dev-dependency |
| `http-body-util` | 0.1 (dev-dep) | `BodyExt::collect` for response body reading in tests | Already a dev-dependency |

### No New Dependencies

Everything required is already in `Cargo.toml`. Do NOT add any new crates.

- `axum::middleware::from_fn_with_state` — available in axum 0.8
- `axum::Extension` extractor — available in axum 0.8
- `axum::Router::route_layer` — available in axum 0.8
- `KeyService::count_active_keys()` — already in `src/auth.rs`
- `KeyService::validate()` — already in `src/auth.rs`
- `ApiError::Unauthorized(String)` — already in `src/error.rs`
- `ApiError::BadRequest(String)` — already in `src/error.rs`
- `AuthContext { key_id, allowed_agent_id }` — already in `src/auth.rs`

---

## Architecture Patterns

### Recommended Project Structure (Phase 12 delta)

```
src/
├── auth.rs          # ADD: pub async fn auth_middleware(...)
├── server.rs        # MODIFY: build_router() — split into public + protected; remove #[allow(dead_code)] on key_service
└── (all other files unchanged)

tests/
└── integration.rs   # ADD: 5 new auth integration tests
```

### Pattern 1: `from_fn_with_state` Middleware Signature

**What:** The middleware function receives `State<AppState>`, `Request`, and `Next` as arguments. The `State` extractor is the first argument, followed by the request and next handler.

**When to use:** Whenever middleware needs to access `AppState` (e.g., to call `key_service`).

**Verified signature (axum 0.8 official pattern):**

```rust
// src/auth.rs
use axum::{
    extract::State,
    http::Request,
    middleware::Next,
    response::Response,
};
use crate::server::AppState;
use crate::error::ApiError;

pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    // Implementation here
}
```

Note: `Request` in axum 0.8 is `axum::http::Request<axum::body::Body>` when fully qualified, but `Request` alone works when `axum::http::Request` is imported.

### Pattern 2: Middleware Control Flow (open mode + auth active)

**What:** The middleware implements a two-phase check. Phase 1: open mode detection. Phase 2: header parsing and token validation. The open mode check comes first and short-circuits entirely.

**Decision-aligned control flow:**

```rust
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    // Phase 1: Open mode check (D-04, D-05)
    match state.key_service.count_active_keys().await {
        Ok(0) => {
            // Open mode — pass through unconditionally (D-05)
            // No AuthContext injected, no header inspection
            return next.run(req).await;
        }
        Err(e) => {
            // DB error reading key count — fail safe by blocking
            tracing::error!(error = %e, "auth: failed to count active keys");
            return ApiError::Unauthorized("auth service unavailable".to_string())
                .into_response();
        }
        Ok(_) => {
            // Auth is active — proceed to header parsing
        }
    }

    // Phase 2: Extract Authorization header (D-09, D-10)
    let auth_header = req.headers().get(axum::http::header::AUTHORIZATION);
    let bearer_token = match auth_header {
        None => {
            // Missing header when auth active → 401 (D-10)
            return ApiError::Unauthorized("missing Authorization header".to_string())
                .into_response();
        }
        Some(value) => {
            // Parse "Bearer <token>" format
            let raw = value.to_str().unwrap_or("");
            if let Some(token) = raw.strip_prefix("Bearer ") {
                token.to_string()
            } else {
                // Malformed (not "Bearer <token>") → 400 (D-09)
                return ApiError::BadRequest(
                    "Authorization header must be: Bearer <token>".to_string()
                ).into_response();
            }
        }
    };

    // Phase 3: Validate token (D-11, D-12)
    match state.key_service.validate(&bearer_token).await {
        Ok(auth_context) => {
            req.extensions_mut().insert(auth_context);
            next.run(req).await
        }
        Err(_) => {
            // Invalid or revoked token → 401 (D-11)
            ApiError::Unauthorized("invalid or revoked API key".to_string())
                .into_response()
        }
    }
}
```

### Pattern 3: Router Split with `route_layer` (D-01, D-02, D-07, D-08)

**What:** `build_router()` is restructured into two `Router` instances merged together. The protected router has `route_layer()` applied to it. The public router has no middleware. They are merged before `with_state()`.

**Critical axum 0.8 behavior:** `route_layer()` applies middleware only to routes defined on the same `Router` instance. When two Routers are merged with `.merge()`, each retains its own layers. The merge does NOT propagate one router's layers onto the other.

```rust
// src/server.rs
use axum::middleware;

pub fn build_router(state: AppState) -> Router {
    // Protected routes — auth middleware applies
    let protected = Router::new()
        .route("/memories", post(create_memory_handler).get(list_memories_handler))
        .route("/memories/search", get(search_memories_handler))
        .route("/memories/{id}", delete(delete_memory_handler))
        .route("/memories/compact", post(compact_memories_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Public routes — no middleware
    let public = Router::new()
        .route("/health", get(health_handler));

    // Merge and attach state
    Router::new()
        .merge(protected)
        .merge(public)
        .with_state(state)
}
```

**Why `route_layer()` not `layer()`:** `layer()` runs on ALL requests including 404s — a request to `/nonexistent` would hit the auth middleware and return 401 instead of 404. `route_layer()` only runs when a route matches. This is the project's established decision (D-01) and is a confirmed axum 0.8 behavior.

### Pattern 4: `AuthContext` Injection via Request Extensions

**What:** The middleware inserts `AuthContext` into `req.extensions_mut()` on successful validation. Downstream handlers (Phase 13) extract it with `Extension<AuthContext>`.

```rust
// In middleware (successful auth):
req.extensions_mut().insert(auth_context);
next.run(req).await

// In handler (Phase 13 usage — not this phase):
async fn create_memory_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateMemoryRequest>,
) -> ...
```

**This phase:** Only the middleware injection is in scope. Handlers are NOT modified to extract `Extension<AuthContext>` — that is Phase 13.

### Pattern 5: `IntoResponse` for Middleware Error Returns

**What:** The middleware returns `Response` (not `Result<Response, ...>`). All error paths must convert to `Response`. `ApiError` already implements `IntoResponse`, so `ApiError::Unauthorized(...).into_response()` works directly.

```rust
use axum::response::IntoResponse;

// In middleware error path:
return ApiError::Unauthorized("...".to_string()).into_response();
```

### Anti-Patterns to Avoid

- **`layer()` on the whole router:** Returns 401 for 404 requests. Use `route_layer()` on the protected sub-router only.
- **Skip-logic inside the middleware for `/health`:** D-08 forbids this. The route architecture handles it structurally.
- **Returning early in open mode after parsing header:** D-05 is explicit — no header inspection in open mode. Pass through immediately after `count == 0`.
- **Holding `#[allow(dead_code)]` after middleware uses `key_service`:** Remove it in this phase since the middleware now uses `key_service`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async middleware with state access | Custom `tower::Service` impl | `axum::middleware::from_fn_with_state` | Official idiomatic approach; `from_fn_with_state` handles the `Clone` requirement for state automatically |
| Bearer header parsing | Custom regex or split logic | `str::strip_prefix("Bearer ")` | Sufficiently correct for `Bearer <token>` format; no external dep needed |
| Error responses in middleware | Custom JSON construction | `ApiError::BadRequest/Unauthorized + .into_response()` | Existing `IntoResponse` impl already produces correct status codes and JSON bodies |
| Key validation | Custom hash lookup | `KeyService::validate()` already exists | Phase 11 completed this; use it directly |
| Open mode detection | Config flag or startup cache | `KeyService::count_active_keys()` already exists | Phase 11 completed this; D-04 mandates per-request query |

**Key insight:** This phase is almost entirely wiring — all the hard components (key validation, error formatting, `AppState`) are already built. The implementation surface is small: one middleware function (~30 lines) and one router restructuring (~15 lines).

---

## Common Pitfalls

### Pitfall 1: `route_layer` vs `layer` ordering on merged routers

**What goes wrong:** Applying `route_layer()` to a `Router` AFTER calling `.merge()` applies the layer to ALL routes including those from the merged router.

**Why it happens:** Developer calls `.merge(public)` then `.route_layer(auth_middleware)` on the combined router.

**How to avoid:** Apply `route_layer()` to the protected router BEFORE merging. The merge then preserves each router's independent layer configuration.

**Warning signs:** `/health` returns 401 when no `Authorization` header is provided.

### Pitfall 2: `use axum::response::IntoResponse` missing in auth.rs

**What goes wrong:** Calling `.into_response()` on `ApiError` in `auth.rs` fails to compile because `IntoResponse` trait is not in scope.

**Why it happens:** `IntoResponse` is a trait from `axum::response` — it must be imported explicitly.

**How to avoid:** Add `use axum::response::IntoResponse;` (or `use axum::response::Response;`) at the top of `src/auth.rs` alongside the other axum imports.

### Pitfall 3: Circular dependency between `auth.rs` and `server.rs`

**What goes wrong:** `auth_middleware` in `auth.rs` needs `AppState` from `server.rs`. `server.rs` imports `auth::auth_middleware`. This creates `auth → server → auth` circular dependency.

**Why it happens:** `AppState` is defined in `server.rs`, and middleware needs it.

**How to avoid:** The middleware function takes `State<AppState>` — axum extracts this at runtime. The import `use crate::server::AppState;` in `auth.rs` is fine because `server.rs` only calls `auth::auth_middleware` by name (no circular type dependency). Rust resolves module-level `use` differently from type references — this is a one-way type reference. Alternatively, move `AppState` to a separate `state.rs` module to make the dependency graph explicit. The existing pattern (server imports auth types) works because auth.rs uses `crate::server::AppState` as a type parameter, not a trait impl.

**Simpler alternative:** Place `auth_middleware` in `server.rs` instead of `auth.rs`. The CONTEXT.md says `auth.rs` is the canonical location, but `server.rs` is also acceptable per the architecture doc ("prefer `auth.rs` so that `server.rs` has no knowledge of the cryptographic details"). If the circular import becomes a real compile error, moving the function to `server.rs` is the escape hatch.

**Expected resolution:** In practice, Rust allows `auth.rs` to reference `server::AppState` and `server.rs` to reference `auth::auth_middleware` — this is module cross-referencing, not a circular dependency (both modules are part of the same crate and Rust resolves intra-crate cycles at the type level, not the module level). The build should succeed.

### Pitfall 4: `Bearer ` prefix includes trailing space

**What goes wrong:** `strip_prefix("Bearer")` without the trailing space leaves a leading space in the extracted token (`" mnk_..."`). `validate()` fails because the hash of `" mnk_..."` != the stored hash.

**How to avoid:** Use `strip_prefix("Bearer ")` with one space. The standard HTTP Bearer format is `Bearer <single-space><token>`.

**Warning signs:** Integration test with a valid token returns 401.

### Pitfall 5: `next.run(req)` not called after inserting extension

**What goes wrong:** The middleware inserts `AuthContext` into extensions but forgets to call `next.run(req).await`, so the handler never executes.

**How to avoid:** The call to `next.run(req).await` must be the final expression in the success branch. Review every code path to confirm `next.run(req)` is called exactly once on success and not called on error paths.

### Pitfall 6: Test DB has keys from a prior test leaking into open-mode tests

**What goes wrong:** Integration tests that create keys into a shared DB contaminate the open-mode test — the DB now has keys, so `count_active_keys()` > 0 and open mode is not triggered.

**How to avoid:** Each auth test that requires a specific DB state (open mode = 0 keys, or active mode = 1+ keys) must use a fresh `build_test_state()` call with an in-memory DB (`:memory:`). The existing test harness already does this — each `build_test_state()` call creates a new in-memory DB. Do not share state across tests that make different assumptions about key count.

### Pitfall 7: `DbError` from `count_active_keys` — unhandled case

**What goes wrong:** The middleware only matches `Ok(0)` for open mode and `Ok(n)` for active mode, but omits the `Err(_)` case. A DB error causes a panic or compile error because the `match` is non-exhaustive.

**How to avoid:** Handle the `Err` branch explicitly. The recommended behavior is to fail safe (return 401 or 500 rather than allowing the request through). The research recommends 401 with "auth service unavailable" — failing open (allowing the request on DB error) would be a security vulnerability.

---

## Code Examples

Verified patterns from official sources and existing codebase:

### Full Middleware Function

```rust
// src/auth.rs — add below existing KeyService impl block
use axum::{
    extract::State,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use crate::server::AppState;

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    // Open mode check: per-request COUNT query (D-04)
    match state.key_service.count_active_keys().await {
        Ok(0) => {
            // Open mode: zero keys → pass through unconditionally (D-05)
            return next.run(req).await;
        }
        Err(e) => {
            tracing::error!(error = %e, "auth: DB error counting active keys");
            return crate::error::ApiError::Unauthorized(
                "auth service unavailable".to_string()
            ).into_response();
        }
        Ok(_) => {} // Auth active — continue below
    }

    // Extract Authorization header
    let auth_header = req.headers().get(axum::http::header::AUTHORIZATION);

    let bearer_token = match auth_header {
        None => {
            // Missing header when auth active → 401 (D-10)
            return crate::error::ApiError::Unauthorized(
                "missing Authorization header".to_string()
            ).into_response();
        }
        Some(value) => {
            let raw = match value.to_str() {
                Ok(s) => s,
                Err(_) => {
                    return crate::error::ApiError::BadRequest(
                        "Authorization header contains invalid characters".to_string()
                    ).into_response();
                }
            };
            match raw.strip_prefix("Bearer ") {
                Some(token) if !token.is_empty() => token.to_string(),
                _ => {
                    // Malformed (not "Bearer <token>") → 400 (D-09)
                    return crate::error::ApiError::BadRequest(
                        "Authorization header must use format: Bearer <token>".to_string()
                    ).into_response();
                }
            }
        }
    };

    // Validate token (D-11, D-12)
    match state.key_service.validate(&bearer_token).await {
        Ok(auth_context) => {
            req.extensions_mut().insert(auth_context);
            next.run(req).await
        }
        Err(_) => {
            // Invalid or revoked token → 401 (D-11)
            crate::error::ApiError::Unauthorized(
                "invalid or revoked API key".to_string()
            ).into_response()
        }
    }
}
```

### Restructured `build_router`

```rust
// src/server.rs — replace existing build_router function
use axum::middleware;
use crate::auth::auth_middleware;

pub fn build_router(state: AppState) -> Router {
    // Protected routes: auth middleware applies via route_layer (D-01, D-02)
    let protected = Router::new()
        .route("/memories", post(create_memory_handler).get(list_memories_handler))
        .route("/memories/search", get(search_memories_handler))
        .route("/memories/{id}", delete(delete_memory_handler))
        .route("/memories/compact", post(compact_memories_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Public routes: no auth (D-07, D-08)
    let public = Router::new()
        .route("/health", get(health_handler));

    Router::new()
        .merge(protected)
        .merge(public)
        .with_state(state)
}
```

### Integration Test Structure (reference pattern from existing tests)

```rust
// tests/integration.rs — new auth tests follow existing helper pattern

/// Helper: builds a test app with one active API key; returns (Router, raw_token).
async fn build_auth_app() -> (axum::Router, String) {
    let (state, _) = build_test_state().await;
    let raw_token = {
        let (_key, token) = state.key_service
            .create("test-key".to_string(), None)
            .await
            .unwrap();
        token
    };
    (build_router(state), raw_token)
}

/// AUTH-01: Valid Bearer token allows the request through.
#[tokio::test]
async fn test_auth_valid_token_allows() {
    let (app, token) = build_auth_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/memories")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `layer()` for all middleware | `route_layer()` for route-specific middleware | axum 0.4+ | Prevents 401 on 404s; health check exemption is structural not conditional |
| Custom `tower::Service` impl for middleware with state | `from_fn_with_state()` | axum 0.6+ | Dramatically simpler; no tower boilerplate |
| `Extension<T>` as `AppState` field | Per-request `Request::extensions()` | Always | Correct pattern: per-request auth context must live in request extensions, not shared state |

**Deprecated/outdated:**
- `axum::middleware::from_fn` without state: Still works but requires wrapping `AppState` in a separate closure capture. `from_fn_with_state` is the canonical way to pass state into middleware in axum 0.6+.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust's built-in `#[tokio::test]` (no additional test framework) |
| Config file | None — uses `cargo test` directly |
| Quick run command | `cargo test auth 2>&1 \| grep -v "^warning"` |
| Full suite command | `cargo test 2>&1 \| grep -v "^warning"` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUTH-01 | Valid Bearer token → 200 + AuthContext injected | integration | `cargo test test_auth_valid_token_allows` | Wave 0 |
| AUTH-02 | Invalid/revoked token → 401 | integration | `cargo test test_auth_invalid_token_rejects` | Wave 0 |
| AUTH-02 | Revoked token → 401 (not passthrough) | integration | `cargo test test_auth_revoked_token_rejects` | Wave 0 |
| AUTH-03 | Zero keys in DB → all requests pass through (open mode) | integration | `cargo test test_auth_open_mode_allows` | Wave 0 |
| AUTH-05 | GET /health → 200 with no Authorization, auth active | integration | `cargo test test_auth_health_no_token` | Wave 0 |

Additional tests (supporting success criteria #5):
| Behavior | Test Type | Command |
|----------|-----------|---------|
| Malformed Authorization header (not `Bearer <token>`) → 400 | integration | `cargo test test_auth_malformed_header_400` | Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test auth 2>&1 | grep -v "^warning"` (runs only auth-prefixed tests, fast)
- **Per wave merge:** `cargo test 2>&1 | grep -v "^warning"` (full suite)
- **Phase gate:** Full suite green (40 existing + 6 new auth tests) before `/gsd:verify-work`

### Wave 0 Gaps

The following test functions must be created in `tests/integration.rs` before implementation:

- [ ] `tests/integration.rs::test_auth_valid_token_allows` — covers AUTH-01
- [ ] `tests/integration.rs::test_auth_invalid_token_rejects` — covers AUTH-02
- [ ] `tests/integration.rs::test_auth_revoked_token_rejects` — covers AUTH-02 (revocation)
- [ ] `tests/integration.rs::test_auth_open_mode_allows` — covers AUTH-03
- [ ] `tests/integration.rs::test_auth_health_no_token` — covers AUTH-05
- [ ] `tests/integration.rs::test_auth_malformed_header_400` — covers success criterion #5

Helper needed: `async fn build_auth_app() -> (axum::Router, String)` — creates a test app with one active key and returns the raw token. This enables AUTH-01, AUTH-02 tests without duplicating key creation logic.

---

## Open Questions

1. **Circular import `auth.rs` ↔ `server.rs`**
   - What we know: `auth.rs` needs `AppState` from `server.rs`; `server.rs` needs `auth_middleware` from `auth.rs`. Rust resolves intra-crate cycles at the type level.
   - What's unclear: Whether this specific cross-reference compiles without issue in this codebase's structure.
   - Recommendation: Attempt `auth.rs` as the location first. If compile fails with a circular dependency error, move `auth_middleware` to `server.rs` as the fallback.

2. **`#[allow(dead_code)]` cleanup on `key_service` field**
   - What we know: `AppState.key_service` has `#[allow(dead_code)]` because Phase 12 hasn't used it yet.
   - What's unclear: Whether the planner should include removing this annotation as an explicit task or treat it as part of the router restructuring task.
   - Recommendation: Remove `#[allow(dead_code)]` from `key_service` field in the same task that restructures `build_router()`, since the middleware will now use it.

3. **`#[allow(dead_code)]` on `ApiError::Unauthorized` in `error.rs`**
   - What we know: `ApiError::Unauthorized` has `#[allow(dead_code)]` since no callers existed before Phase 12.
   - Recommendation: Remove it in the task that adds `auth_middleware`, since the middleware will call it.

---

## Sources

### Primary (HIGH confidence)

- Direct source inspection: `src/auth.rs`, `src/server.rs`, `src/error.rs`, `src/main.rs`, `src/lib.rs`, `tests/integration.rs` — HIGH (direct codebase inspection, authoritative)
- `.planning/phases/12-auth-middleware/12-CONTEXT.md` — HIGH (user decisions, locked)
- `.planning/research/ARCHITECTURE.md` §Pattern 1 (auth middleware with from_fn_with_state) — HIGH (axum 0.8 official pattern, previously researched)
- `.planning/research/PITFALLS.md` §Auth Pitfall 10 (open mode + invalid token) — HIGH (project canonical reference)
- `Cargo.toml` — HIGH (verified no new dependencies needed; all required crates present)
- `.planning/config.json` — HIGH (nyquist_validation: true confirmed)

### Secondary (MEDIUM confidence)

- [axum middleware docs — from_fn_with_state](https://docs.rs/axum/latest/axum/middleware/fn.from_fn_with_state.html) — verified against axum 0.8 (from prior phase research)
- [axum route_layer vs layer behavior](https://docs.rs/axum/latest/axum/struct.Router.html#method.route_layer) — verified: applies only to matched routes

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new deps; all required pieces verified by direct code inspection
- Architecture: HIGH — all patterns established in prior phase research; confirmed against actual axum 0.8 source files
- Pitfalls: HIGH — verified from PITFALLS.md (authoritative project research) and direct code inspection
- Test infrastructure: HIGH — verified by reading full `tests/integration.rs` and confirming helper patterns

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (axum 0.8 is stable; no breaking changes expected in 30 days)
