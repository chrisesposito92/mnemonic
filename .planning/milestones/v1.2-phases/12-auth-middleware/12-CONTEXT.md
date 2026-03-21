# Phase 12: Auth Middleware - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Axum middleware that enforces authentication on all protected routes and injects `AuthContext` into request extensions. Includes open mode (zero keys = allow all), health-check exemption, and proper handling of malformed headers. No REST key endpoints, no CLI, no scope enforcement — just the middleware layer.

</domain>

<decisions>
## Implementation Decisions

### Middleware Placement
- **D-01:** Use `route_layer()` on the protected route group, NOT `layer()` on the entire Router — prevents 401 on unmatched routes (carried from STATE.md decision)
- **D-02:** Split `build_router()` into two groups: public routes (`/health`) registered first, then protected routes (all `/memories/*` endpoints) with auth middleware via `route_layer()`
- **D-03:** Middleware is an `axum::middleware::from_fn_with_state()` — receives `AppState` to access `key_service`

### Open Mode
- **D-04:** Open mode = per-request `COUNT(*)` query via `KeyService::count_active_keys()` — NOT a cached flag or startup check (carried from STATE.md decision)
- **D-05:** When `count_active_keys()` returns 0, middleware passes request through without checking Authorization header — no `AuthContext` injected
- **D-06:** When keys are created (Phase 13), auth activates on the next request automatically — no server restart needed

### Health Check Exemption
- **D-07:** `/health` is registered as a separate route group BEFORE the auth middleware layer — axum's `route_layer()` only applies to routes defined in the same Router segment
- **D-08:** No skip-logic inside the middleware — the route architecture handles exemption cleanly

### Header Parsing
- **D-09:** Malformed `Authorization` header (not `Bearer <token>` format) returns 400 Bad Request — distinguishes client errors from auth failures (per success criteria #5)
- **D-10:** Missing `Authorization` header when auth is active returns 401 Unauthorized
- **D-11:** Invalid or revoked token returns 401 Unauthorized (delegates to `KeyService::validate()`)
- **D-12:** On successful validation, inject `AuthContext` into request extensions via `request.extensions_mut().insert(auth_context)` — downstream handlers access it with `Extension<AuthContext>`

### Error Responses
- **D-13:** 401 response uses existing `ApiError::Unauthorized(String)` — detailed JSON body with `auth_mode` and `hint` fields (carried from Phase 10 D-08)
- **D-14:** 400 response for malformed headers uses existing `ApiError::BadRequest(String)` — consistent with other validation errors

### Claude's Discretion
- Exact middleware function signature and internal control flow
- Whether to extract header parsing into a helper function
- Test organization (inline `#[cfg(test)]` vs integration tests)
- Whether the `#[allow(dead_code)]` on `key_service` in AppState should be removed in this phase or Phase 13

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Auth middleware architecture
- `.planning/research/ARCHITECTURE.md` §v1.2 System Overview — Middleware position in request lifecycle
- `.planning/research/ARCHITECTURE.md` §Build Order — Phase 12 dependency on Phase 11
- `.planning/research/PITFALLS.md` §Auth Pitfall 10 — Open mode + invalid token edge case: must return 401, not passthrough

### Requirements and success criteria
- `.planning/REQUIREMENTS.md` — AUTH-01 (valid token auth), AUTH-02 (invalid token 401), AUTH-03 (open mode), AUTH-05 (health exemption)
- `.planning/ROADMAP.md` §Phase 12 — 5 success criteria

### Prior phase decisions
- `.planning/phases/10-auth-schema-foundation/10-CONTEXT.md` — D-07/D-08 (Unauthorized variant, 401 body format), D-11/D-12 (open mode detection)
- `.planning/phases/11-keyservice-core/11-CONTEXT.md` — D-07/D-08/D-10 (validate() behavior, revoked key handling)

### Existing code patterns
- `src/auth.rs` — `KeyService::validate()`, `KeyService::count_active_keys()`, `AuthContext` struct
- `src/server.rs` — `build_router()`, `AppState` with `key_service: Arc<KeyService>`
- `src/error.rs` — `ApiError::Unauthorized(String)`, `ApiError::BadRequest(String)`, `IntoResponse` impl

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `KeyService::validate(raw_token)` — Returns `Result<AuthContext, DbError>`. Middleware calls this with extracted Bearer token.
- `KeyService::count_active_keys()` — Returns `Result<i64, DbError>`. Middleware calls this to check open mode (0 = open).
- `ApiError::Unauthorized(String)` — Already has `IntoResponse` impl with 401 status and JSON body.
- `ApiError::BadRequest(String)` — Already has `IntoResponse` impl with 400 status.
- `AuthContext { key_id, allowed_agent_id }` — Struct to inject into request extensions.

### Established Patterns
- All services accessed via `Arc<T>` in `AppState` — middleware gets state via `from_fn_with_state()`
- Error responses return `(StatusCode, Json(json!({...})))` — consistent format
- `build_router()` returns `Router` — middleware attaches here via `route_layer()`

### Integration Points
- `server.rs::build_router()` — Restructure to separate public routes from protected routes with `route_layer()`
- `server.rs::AppState::key_service` — Remove `#[allow(dead_code)]` annotation once middleware uses it
- `auth.rs` — Add middleware function (e.g., `auth_middleware`) as a public async function

</code_context>

<specifics>
## Specific Ideas

- The middleware function extracts `Authorization` header, parses `Bearer <token>`, calls `validate()`, and either injects `AuthContext` or returns an error response
- Open mode check (`count_active_keys() == 0`) happens BEFORE header parsing — in open mode, the request passes through regardless of header presence
- Auth Pitfall 10: In open mode, a request with an INVALID token should still pass through — open mode means no auth enforcement at all
- The `route_layer()` approach means `/health` never hits the middleware — it's structurally impossible, not a conditional check

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 12-auth-middleware*
*Context gathered: 2026-03-21*
