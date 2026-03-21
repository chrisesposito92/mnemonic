# Phase 13: HTTP Wiring and REST Key Endpoints - Research

**Researched:** 2026-03-20
**Domain:** Rust/axum HTTP handler layer — scope enforcement, REST key CRUD endpoints, 403 Forbidden error variant
**Confidence:** HIGH (entire codebase directly inspected; no external library discovery needed)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** `/keys` routes ARE protected by the auth middleware — same protected router group as `/memories`
- **D-02:** Open mode (zero keys): middleware passes through unconditionally, first key can be created without having a key — no bootstrapping problem
- **D-03:** Once any key exists, only authenticated users can manage keys — wildcard or scoped keys can create/list/revoke other keys
- **D-04:** Scope mismatch returns 403 Forbidden — NOT a silent override. If `AuthContext.allowed_agent_id` is `Some("agent-x")` and request specifies `agent_id: "agent-y"`, handler returns 403
- **D-05:** Missing agent_id with scoped key: handler forces agent_id to the key's scope implicitly — no 403, no error
- **D-06:** Wildcard keys (`allowed_agent_id = None`): pass through, use client-supplied agent_id without restriction
- **D-07:** Open mode (no `AuthContext` in extensions): proceed exactly as before — no scope enforcement, backward-compatible
- **D-08:** `POST /memories` — agent_id optional in body; scoped key: force if missing, 403 if mismatched
- **D-09:** `GET /memories` — agent_id optional in query; scoped key: force if missing, 403 if mismatched
- **D-10:** `GET /memories/search` — agent_id optional in query; same as D-09
- **D-11:** `POST /memories/compact` — agent_id required in body; scoped key: 403 if mismatched (cannot be missing)
- **D-12:** `DELETE /memories/{id}` — no agent_id in request; scoped key: look up memory's agent_id, return 403 if it doesn't match key scope
- **D-13:** Add `ApiError::Forbidden(String)` variant to error.rs — maps to HTTP 403
- **D-14:** 403 body: `{ "error": "forbidden", "detail": "key scoped to agent-x cannot access agent-y" }` — includes agent IDs for debuggability
- **D-15:** `POST /keys` accepts `{ "name": "...", "agent_id": "..." }` — name required, agent_id optional. Returns 201 with `{ "key": { id, name, display_id, agent_id, created_at }, "raw_token": "mnk_..." }`
- **D-16:** `GET /keys` returns 200 with `{ "keys": [ { id, name, display_id, agent_id, created_at, revoked_at }, ... ] }` — never includes raw token or hashed_key
- **D-17:** `DELETE /keys/:id` returns 200 with `{ "revoked": true, "id": "..." }`
- **D-18:** Key routes added to the protected router group — single `route_layer()` covers both
- **D-19:** Route paths: `/keys` for POST and GET, `/keys/{id}` for DELETE — flat, no nesting

### Claude's Discretion

- Handler function organization (scope enforcement as inline logic vs. extracted helper)
- Whether to use `Option<Extension<AuthContext>>` or manual extension extraction for optional auth context
- Test organization (integration tests for scope enforcement scenarios)
- Whether to deserialize key creation body into a dedicated struct or reuse inline serde

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AUTH-04 | A scoped key's agent_id overrides the client-supplied agent_id, preventing cross-agent access | D-04 through D-12 directly address this. Handler-layer scope enforcement using `AuthContext.allowed_agent_id`. The critical test: use key for agent-A, send `agent_id: agent-B` in body, assert 403. |
| INFRA-03 | Server startup log announces whether running in open or authenticated mode | Already implemented in Phase 10. No new work required — confirmed by inspecting auth.rs which calls `count_active_keys()` and server.rs `serve()` which logs startup. Verify no regression. |

</phase_requirements>

---

## Summary

Phase 13 operates entirely within the existing Rust/axum codebase. All infrastructure is already built (Phases 10-12): the `api_keys` table, `KeyService` CRUD methods, `AuthContext`, and `auth_middleware`. This phase adds three new REST handlers for key management and modifies five existing memory/compaction handlers to enforce namespace isolation using the `AuthContext` already injected by the middleware.

The core challenge is not discovery but precision: five handlers each need a distinct scope enforcement pattern depending on whether `agent_id` comes from body, query params, or must be fetched from the database (DELETE /memories/{id}). The `ApiError::Forbidden` variant must be added to error.rs and wired into all five enforcement paths consistently.

The secondary challenge is the `Option<Extension<AuthContext>>` extraction pattern. In authenticated mode the middleware always injects `AuthContext`, but in open mode it does not. Handlers must handle both cases: `Some(auth)` (auth active) and `None` (open mode, no enforcement). This requires `Extension<AuthContext>` to be optional in the handler signature or extracted manually.

**Primary recommendation:** Extract a `fn enforce_scope(auth: Option<&AuthContext>, requested: Option<&str>) -> Result<Option<String>, ApiError>` helper that centralizes the three-way logic (open mode / wildcard key / scoped key) and call it from each handler. This keeps handlers clean and the 403 message consistent with D-14.

---

## Standard Stack

No new dependencies required. All libraries are already in Cargo.toml.

### Core (already present)
| Library | Purpose | How Used |
|---------|---------|----------|
| `axum` | HTTP framework | `Extension<AuthContext>`, routing, `route_layer` |
| `serde` / `serde_json` | JSON serialization | Key endpoint request/response structs |
| `thiserror` | Error derivation | `ApiError::Forbidden` variant |
| `tokio` | Async runtime | All async handler functions |

### Key types already in codebase
| Type | Location | Phase 13 Use |
|------|----------|-------------|
| `AuthContext { key_id, allowed_agent_id: Option<String> }` | `src/auth.rs` | Extracted in all 5 modified handlers |
| `KeyService::create(name, agent_id)` | `src/auth.rs` | Called by `create_key_handler` |
| `KeyService::list()` | `src/auth.rs` | Called by `list_keys_handler` |
| `KeyService::revoke(id)` | `src/auth.rs` | Called by `revoke_key_handler` |
| `ApiKey { id, name, display_id, agent_id, created_at, revoked_at }` | `src/auth.rs` | Serialized in key endpoint responses |
| `ApiError::Unauthorized(String)` | `src/error.rs` | Pattern for `Forbidden` variant |

---

## Architecture Patterns

### Recommended Project Structure (changes only)

```
src/
├── error.rs        # ADD: ApiError::Forbidden(String) variant + IntoResponse arm
├── server.rs       # ADD: 3 key handlers, MODIFY: 5 memory/compaction handlers + router wiring
└── (no new files)
```

### Pattern 1: Optional AuthContext Extraction

**What:** In open mode the middleware does not inject `AuthContext`. Handlers on the protected router need to tolerate its absence.

**When to use:** All five modified memory/compaction handlers and all three new key handlers.

**Two valid approaches:**

Option A — `Option<Extension<AuthContext>>` parameter (axum built-in):
```rust
// Source: axum docs — Extension extractor returns None if not present
async fn create_memory_handler(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Json(mut body): Json<CreateMemoryRequest>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), ApiError> {
    let effective_agent_id = enforce_scope(
        auth.as_deref().map(|e| &e.0),  // Option<&AuthContext>
        body.agent_id.as_deref(),
    )?;
    body.agent_id = effective_agent_id;
    // ...
}
```

Option B — manual `req.extensions().get::<AuthContext>()` (requires `Request` parameter, heavier).

Option A is cleaner and preferred — axum's `Option<Extension<T>>` returns `None` if the extension is missing rather than returning a 500.

**Note on axum `Extension` extractor:** `Extension<T>` in axum returns a 500 if the extension is missing (by design — it's a programmer error). `Option<Extension<T>>` returns `None` gracefully. Use `Option<Extension<AuthContext>>` for handlers that must work in both open and authenticated mode.

### Pattern 2: Scope Enforcement Helper

**What:** Centralized function that implements the three-way decision for all five handlers.

**Signature:**
```rust
// Returns: Ok(Some(agent_id)) — use this agent_id
//          Ok(None)           — open mode, use body value as-is
//          Err(ApiError::Forbidden) — scope violation
fn enforce_scope(
    auth: Option<&AuthContext>,
    requested: Option<&str>,
) -> Result<Option<String>, ApiError> {
    match auth {
        None => Ok(None),  // Open mode: no enforcement (D-07)
        Some(ctx) => match &ctx.allowed_agent_id {
            None => Ok(requested.map(str::to_string)),  // Wildcard key: pass through (D-06)
            Some(allowed) => match requested {
                None => Ok(Some(allowed.clone())),  // Missing agent_id: force scope (D-05)
                Some(req_id) if req_id == allowed => Ok(Some(allowed.clone())),  // Match: ok
                Some(req_id) => Err(ApiError::Forbidden(format!(
                    "key scoped to {} cannot access {}",
                    allowed, req_id
                ))),  // Mismatch: 403 (D-04, D-14)
            },
        },
    }
}
```

This function covers D-04 through D-07 exhaustively. Each handler calls it once (or in the DELETE /memories/{id} case, after the DB lookup).

### Pattern 3: DELETE /memories/{id} Scope Verification

**What:** The delete handler has no `agent_id` in the request. Scope enforcement requires fetching the memory's `agent_id` from the DB before checking authorization.

**When to use:** Only `delete_memory_handler` needs this pattern (D-12).

**Implementation:**
```rust
async fn delete_memory_handler(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Scope enforcement for scoped keys (D-12)
    if let Some(Extension(ctx)) = &auth {
        if let Some(allowed_id) = &ctx.allowed_agent_id {
            // Fetch memory to check ownership before deleting
            let memory = state.service.get_memory_agent_id(&id).await?;
            match memory {
                None => return Err(ApiError::NotFound),
                Some(mem_agent_id) if &mem_agent_id != allowed_id => {
                    return Err(ApiError::Forbidden(format!(
                        "key scoped to {} cannot access {}",
                        allowed_id, mem_agent_id
                    )));
                }
                Some(_) => {}  // Match: proceed
            }
        }
    }
    let memory = state.service.delete_memory(id).await?;
    Ok(Json(serde_json::to_value(memory).unwrap()))
}
```

**Note:** `MemoryService` currently lacks a `get_memory_agent_id(id)` method. The service.rs `delete_memory()` already does a SELECT + DELETE. Two options:
1. Add `MemoryService::get_memory_agent_id(id) -> Result<Option<String>, ApiError>` — minimal DB query (SELECT agent_id FROM memories WHERE id = ?1)
2. Read the memory via existing `delete_memory` logic, but this deletes before checking auth — wrong order

Option 1 is correct: add a lightweight read method. This is a two-query pattern (first check ownership, then delete) — acceptable per the CONTEXT.md note: "Two DB queries but ensures isolation."

### Pattern 4: Key Endpoint Handlers

**What:** Three new handlers following the existing pattern for POST/GET/DELETE.

**POST /keys:**
```rust
#[derive(serde::Deserialize)]
struct CreateKeyRequest {
    name: String,
    agent_id: Option<String>,
}

async fn create_key_handler(
    State(state): State<AppState>,
    Json(body): Json<CreateKeyRequest>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), ApiError> {
    let (api_key, raw_token) = state.key_service.create(body.name, body.agent_id).await
        .map_err(|e| ApiError::Internal(e.into()))?;
    // CRITICAL: Do NOT log raw_token (PITFALLS.md Auth Pitfall 6)
    tracing::info!(key_id = %api_key.id, agent_id = ?api_key.agent_id, "API key created");
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({
            "key": {
                "id": api_key.id,
                "name": api_key.name,
                "display_id": api_key.display_id,
                "agent_id": api_key.agent_id,
                "created_at": api_key.created_at
            },
            "raw_token": raw_token  // shown once in response, never logged
        })),
    ))
}
```

**GET /keys:**
```rust
async fn list_keys_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let keys = state.key_service.list().await
        .map_err(|e| ApiError::Internal(e.into()))?;
    Ok(Json(serde_json::json!({
        "keys": keys.iter().map(|k| serde_json::json!({
            "id": k.id,
            "name": k.name,
            "display_id": k.display_id,
            "agent_id": k.agent_id,
            "created_at": k.created_at,
            "revoked_at": k.revoked_at
        })).collect::<Vec<_>>()
    })))
}
```

**DELETE /keys/{id}:**
```rust
async fn revoke_key_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.key_service.revoke(&id).await
        .map_err(|e| ApiError::Internal(e.into()))?;
    Ok(Json(serde_json::json!({ "revoked": true, "id": id })))
}
```

### Pattern 5: Router Wiring (D-18, D-19)

Add key routes to the existing protected router group. The current `build_router()` already has the split structure:

```rust
pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/memories", post(create_memory_handler).get(list_memories_handler))
        .route("/memories/search", get(search_memories_handler))
        .route("/memories/{id}", delete(delete_memory_handler))
        .route("/memories/compact", post(compact_memories_handler))
        // ADD THESE:
        .route("/keys", post(create_key_handler).get(list_keys_handler))
        .route("/keys/{id}", delete(revoke_key_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));
    // ...
}
```

### Pattern 6: ApiError::Forbidden Variant (D-13, D-14)

Add to `ApiError` enum in error.rs — follows exact same structure as `Unauthorized`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    // ADD:
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("internal error: {0}")]
    Internal(#[from] MnemonicError),
}
```

Add the `Forbidden` arm to the `IntoResponse` impl:
```rust
ApiError::Forbidden(detail) => (
    axum::http::StatusCode::FORBIDDEN,
    serde_json::json!({
        "error": "forbidden",
        "detail": detail
    }),
),
```

Note: D-14 specifies `"error": "forbidden"` (not the full message) and `"detail"` as a separate field — different shape from the existing variants that put the message in `"error"`. This is intentional per the CONTEXT.md.

### Anti-Patterns to Avoid

- **Using `body.agent_id` as the scope authority:** The 403/override decision MUST use `AuthContext.allowed_agent_id`, not the client-supplied value. This is the core of AUTH-04. See PITFALLS.md §Auth Pitfall 4.
- **Logging raw_token in `create_key_handler`:** `tracing::info!` must NEVER format the raw token string. Log only `key_id` and `agent_id`. See PITFALLS.md §Auth Pitfall 6.
- **Placing scope enforcement in `MemoryService`:** Services remain auth-unaware per the established architecture (ARCHITECTURE.md §Anti-Pattern 4).
- **Returning 404 for scope violation on DELETE /memories/{id}:** If a memory exists but belongs to a different agent, return 403 not 404 — leaking existence information via 404 is a lesser concern than returning the wrong status code for an authorization failure. The CONTEXT.md explicitly requires 403.
- **Using `Extension<AuthContext>` (non-optional) in handlers:** This causes a 500 in open mode. Use `Option<Extension<AuthContext>>`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Key creation/list/revoke | Custom DB queries | `KeyService::create()`, `list()`, `revoke()` | Already built and tested in Phase 11 |
| Token validation | Custom hash comparison | `KeyService::validate()` | Already does BLAKE3 + constant-time comparison |
| Auth middleware | Re-implementing | Existing `auth_middleware` already running | Phase 12 complete |
| JSON error responses | Custom response builders | `ApiError::IntoResponse` impl | Already handles status code + body mapping |

---

## Common Pitfalls

### Pitfall 1: Using `Extension<AuthContext>` Instead of `Option<Extension<AuthContext>>`

**What goes wrong:** Axum's `Extension<T>` extractor panics (returns 500) if the extension is not present. In open mode, the middleware does NOT insert `AuthContext`. Any handler using `Extension<AuthContext>` directly will 500 on every open-mode request.

**Why it happens:** The non-optional form looks cleaner. Developers add it expecting the middleware to always inject it.

**How to avoid:** Use `Option<Extension<AuthContext>>` in all five modified handlers and all three key handlers. The `None` branch means open mode.

**Warning signs:** Open-mode requests to `/memories` return 500 instead of passing through.

### Pitfall 2: Scope Enforcement on Key Endpoints Creates Circular Bootstrap Problem

**What goes wrong:** If key endpoint handlers enforce scope (checking `AuthContext.allowed_agent_id` against some resource's agent_id), the first key cannot be created — open mode has no `AuthContext`, so the scope check would error.

**Why it happens:** Developers apply the same scope enforcement helper to key handlers as they do to memory handlers.

**How to avoid:** Key handlers (create/list/revoke) do NOT apply the `enforce_scope` helper. They are protected by the auth middleware (any valid key can manage keys, per D-03) but do not have per-agent scoping. This is intentional per D-03.

### Pitfall 3: Raw Token in `create_key_handler` Tracing

**What goes wrong:** `tracing::info!("Created key: {}", raw_token)` or similar logs the full `mnk_...` value to any log aggregator. The key is now in logs forever, defeating the one-time display security model.

**Why it happens:** The create handler naturally wants to log what it created. Developers log the full response value without realizing `raw_token` is in scope.

**How to avoid:** Only log `key_id` and `agent_id`. The raw token goes into the response body only, never into tracing. See PITFALLS.md §Auth Pitfall 6.

### Pitfall 4: `enforce_scope` Called Before Fetching Memory for DELETE /memories/{id}

**What goes wrong:** Without fetching the memory first, there is no `agent_id` to compare against for `DELETE /memories/{id}`. Using `enforce_scope(auth, None)` would always force the body's agent_id (which doesn't exist for DELETE) or skip enforcement.

**Why it happens:** Developers apply the same helper signature to DELETE as to other handlers.

**How to avoid:** For DELETE /memories/{id} with a scoped key, fetch the memory's agent_id from the DB first (separate query), then compare directly against `ctx.allowed_agent_id`. Only then call `delete_memory()`. Two DB queries is correct per D-12 and the CONTEXT.md note.

### Pitfall 5: DbError Not Mapped to ApiError in Key Handlers

**What goes wrong:** `KeyService` methods return `Result<_, DbError>`. If the handler returns `Result<_, ApiError>` and uses `?` directly on a `DbError`, it won't compile — there's no `From<DbError> for ApiError` impl.

**Why it happens:** Memory/compaction handlers use `ApiError::Internal(#[from] MnemonicError)` which wraps `DbError`. But `DbError` alone does not convert.

**How to avoid:** Map explicitly: `.map_err(|e| ApiError::Internal(MnemonicError::Db(e)))?` or add a `From<DbError>` impl. Looking at existing code, `impl From<tokio_rusqlite::Error> for ApiError` already exists (converts to `Internal`). Since `DbError::from(tokio_rusqlite::Error)` also exists, the chain works if the service returns `tokio_rusqlite::Error`. But `KeyService` methods wrap into `DbError` first. Safest: use `.map_err(|e| ApiError::Internal(crate::error::MnemonicError::Db(e)))?`.

### Pitfall 6: 403 Body Shape Diverges from Other Error Variants

**What goes wrong:** Other error variants in the existing `IntoResponse` use `"error": "<message>"`. D-14 specifies `"error": "forbidden"` + `"detail": "<detail>"`. If the Forbidden arm uses the same single-field shape, the planner's specified response body is wrong.

**How to avoid:** The Forbidden `IntoResponse` arm must use TWO fields: `"error": "forbidden"` (literal string) and `"detail": <the String parameter>`. This is intentional and correct per D-14.

---

## Code Examples

### enforce_scope helper (canonical scope logic)

```rust
// All three-way logic in one place: open mode / wildcard / scoped
// Returns the effective agent_id to use, or Err(Forbidden)
fn enforce_scope(
    auth: Option<&AuthContext>,
    requested: Option<&str>,
) -> Result<Option<String>, ApiError> {
    match auth {
        None => Ok(None),  // D-07: open mode, no enforcement
        Some(ctx) => match &ctx.allowed_agent_id {
            None => Ok(requested.map(str::to_string)),  // D-06: wildcard key
            Some(allowed) => match requested {
                None => Ok(Some(allowed.clone())),  // D-05: missing agent_id, force scope
                Some(req_id) if req_id == allowed.as_str() => Ok(Some(allowed.clone())),
                Some(req_id) => Err(ApiError::Forbidden(format!(
                    "key scoped to {} cannot access {}",
                    allowed, req_id
                ))),  // D-04, D-14
            },
        },
    }
}
```

### How handlers call enforce_scope

```rust
// POST /memories (D-08) — body agent_id optional
async fn create_memory_handler(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Json(mut body): Json<CreateMemoryRequest>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), ApiError> {
    let auth_ctx = auth.as_ref().map(|Extension(ctx)| ctx);
    let effective = enforce_scope(auth_ctx, body.agent_id.as_deref())?;
    if effective.is_some() {
        body.agent_id = effective;
    }
    let memory = state.service.create_memory(body).await?;
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::to_value(memory).unwrap())))
}

// GET /memories/search (D-10) — query agent_id optional
async fn search_memories_handler(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Query(mut params): Query<SearchParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let auth_ctx = auth.as_ref().map(|Extension(ctx)| ctx);
    let effective = enforce_scope(auth_ctx, params.agent_id.as_deref())?;
    if effective.is_some() {
        params.agent_id = effective;
    }
    let response = state.service.search_memories(params).await?;
    Ok(Json(serde_json::to_value(response).unwrap()))
}

// POST /memories/compact (D-11) — agent_id required in body
async fn compact_memories_handler(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Json(mut body): Json<CompactRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Existing validations first...
    if body.agent_id.trim().is_empty() {
        return Err(ApiError::BadRequest("agent_id must not be empty".to_string()));
    }
    // Scope enforcement: D-11 says scoped key returns 403 if mismatched
    // enforce_scope called with Some(agent_id) — "missing" case never happens since required
    let auth_ctx = auth.as_ref().map(|Extension(ctx)| ctx);
    let effective = enforce_scope(auth_ctx, Some(body.agent_id.as_str()))?;
    if let Some(forced) = effective {
        body.agent_id = forced;
    }
    // ...
}
```

### MemoryService helper needed for DELETE scope check

```rust
// Add to service.rs MemoryService impl
pub async fn get_memory_agent_id(&self, id: &str) -> Result<Option<String>, ApiError> {
    let id = id.to_string();
    let result = self.db.call(move |c| -> Result<Option<String>, rusqlite::Error> {
        let mut stmt = c.prepare(
            "SELECT agent_id FROM memories WHERE id = ?1"
        )?;
        stmt.query_row(rusqlite::params![id], |row| row.get(0))
            .optional()
    }).await?;
    Ok(result)
}
```

---

## Runtime State Inventory

> Phase 13 is not a rename/refactor phase. This section is not applicable.

---

## Validation Architecture

`nyquist_validation: true` in .planning/config.json — section required.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in + tokio-test (no pytest/jest) |
| Config file | Cargo.toml (test harness built-in) |
| Quick run command | `cargo test --test integration 2>&1 | tail -20` |
| Full suite command | `cargo test 2>&1 | tail -30` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUTH-04-a | Scoped key for agent-A + body `agent_id: agent-B` → 403 | integration | `cargo test test_scope_mismatch_returns_403 2>&1` | ❌ Wave 0 |
| AUTH-04-b | Scoped key for agent-A + no agent_id in body → forces agent-A | integration | `cargo test test_scope_forces_agent_id 2>&1` | ❌ Wave 0 |
| AUTH-04-c | Wildcard key + body `agent_id: any` → passes through | integration | `cargo test test_wildcard_key_passes_through 2>&1` | ❌ Wave 0 |
| AUTH-04-d | Scoped key for agent-A + DELETE memory owned by agent-B → 403 | integration | `cargo test test_scoped_delete_wrong_owner_403 2>&1` | ❌ Wave 0 |
| AUTH-04-e | Scoped key for agent-A + DELETE memory owned by agent-A → 200 | integration | `cargo test test_scoped_delete_own_memory_ok 2>&1` | ❌ Wave 0 |
| KEY-endpoint-a | POST /keys creates key, returns 201 + raw_token | integration | `cargo test test_post_keys_creates_key 2>&1` | ❌ Wave 0 |
| KEY-endpoint-b | GET /keys returns key metadata, no raw token | integration | `cargo test test_get_keys_no_raw_token 2>&1` | ❌ Wave 0 |
| KEY-endpoint-c | DELETE /keys/:id revokes key, subsequent requests return 401 | integration | `cargo test test_delete_key_revokes_access 2>&1` | ❌ Wave 0 |
| INFRA-03 | Startup log confirms open vs authenticated mode | manual-only | Inspect log output during server startup | Existing behavior |

Manual-only justification for INFRA-03: Already implemented in Phase 10/12. The `count_active_keys()` → log path exists. A regression would surface in AUTH-04 and KEY endpoint tests (if auth mode is broken, those tests fail). No isolated test needed.

### Sampling Rate

- **Per task commit:** `cargo test --test integration -- --nocapture 2>&1 | tail -20`
- **Per wave merge:** `cargo test 2>&1 | tail -30`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/integration.rs` — add 8 new tests covering AUTH-04-a through AUTH-04-e and KEY-endpoint-a through KEY-endpoint-c
- [ ] `src/service.rs` — add `get_memory_agent_id()` method (needed by delete scope check)
- [ ] `src/error.rs` — add `ApiError::Forbidden(String)` variant (needed by all scope enforcement)

*(No new test files needed — add to existing `tests/integration.rs` following Phase 12 pattern)*

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Scope enforcement in service layer | Handler layer only (ARCHITECTURE.md §Anti-Pattern 4) | v1.2 design | Services remain auth-unaware; handlers own the 403 logic |
| `layer()` for middleware | `route_layer()` per protected router group | Phase 12 | Non-matching routes return 404 not 401 |

---

## Open Questions

1. **`enforce_scope` placement: free function vs. method on `AuthContext`**
   - What we know: The function only needs `AuthContext` data, no service state. A free function in server.rs or a method on `AuthContext` in auth.rs both work.
   - What's unclear: Whether auth.rs is the better home (keeps auth logic together) vs. server.rs (keeps it with handlers).
   - Recommendation: Free function in `server.rs` — it's handler-layer logic, not auth-layer logic. No import changes needed.

2. **`DbError` → `ApiError` mapping in key handlers**
   - What we know: `KeyService` returns `Result<_, DbError>`. `ApiError::Internal` wraps `MnemonicError`, which wraps `DbError`.
   - What's unclear: Whether the existing `From<tokio_rusqlite::Error> for ApiError` chain covers the `DbError` produced by `KeyService` (which wraps the tokio_rusqlite error).
   - Recommendation: Use explicit mapping `.map_err(|e| ApiError::Internal(crate::error::MnemonicError::Db(e)))?` in key handlers rather than relying on `?` auto-conversion. Verify it compiles.

3. **`enforce_scope` return for open mode with None requested**
   - What we know: `enforce_scope(None, None)` → `Ok(None)`. The handler then does `if effective.is_some() { body.agent_id = effective; }` — no change to body.
   - This is correct: open mode with no agent_id in body works as before (agent_id defaults to empty string in service).
   - No open question — just confirming this matches D-07.

---

## Sources

### Primary (HIGH confidence)

- Direct inspection of `src/auth.rs` — `AuthContext`, `KeyService`, all method signatures
- Direct inspection of `src/server.rs` — `build_router()`, handler signatures, `AppState`
- Direct inspection of `src/error.rs` — `ApiError` enum, `IntoResponse` impl
- Direct inspection of `src/service.rs` — `CreateMemoryRequest`, `SearchParams`, `ListParams`, `delete_memory()`
- Direct inspection of `src/compaction.rs` — `CompactRequest` struct
- `.planning/phases/13-http-wiring-and-rest-key-endpoints/13-CONTEXT.md` — all locked decisions
- `.planning/research/ARCHITECTURE.md` §Scoped Keys, §Pattern 2, §Anti-Pattern 4 — scope enforcement architecture
- `.planning/research/PITFALLS.md` §Auth Pitfall 4 (scope gap), §Auth Pitfall 6 (key logging) — security constraints

### Secondary (MEDIUM confidence)

- axum docs — `Option<Extension<T>>` returns None when extension missing (verified by axum behavior, not re-checked via Context7 since behavior is well-established and used in Phase 12 already)

### Tertiary (LOW confidence)

None — all findings are from direct source inspection.

---

## Metadata

**Confidence breakdown:**

- Implementation patterns: HIGH — entire codebase directly inspected, all types and methods known
- Test strategy: HIGH — test infrastructure already established in Phase 12, patterns clear
- `enforce_scope` helper: HIGH — derived directly from D-04 through D-07 in CONTEXT.md
- DbError mapping: MEDIUM — needs compile verification (open question 2 above)

**Research date:** 2026-03-20
**Valid until:** Indefinite — no external dependencies, all findings from direct codebase inspection
