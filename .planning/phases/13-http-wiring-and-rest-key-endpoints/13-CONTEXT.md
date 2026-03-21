# Phase 13: HTTP Wiring and REST Key Endpoints - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Add REST key management endpoints (`POST /keys`, `GET /keys`, `DELETE /keys/:id`), enforce scope at the handler layer so scoped keys cannot access other agents' memories, and add the `Forbidden` (403) error variant. Auth middleware is already built (Phase 12) — this phase wires it to the router and adds scope enforcement logic to existing handlers.

</domain>

<decisions>
## Implementation Decisions

### Key endpoint protection
- **D-01:** `/keys` routes ARE protected by the auth middleware — they go in the same protected router group as `/memories` routes [auto: matches research guidance — "key management routes must require auth once any key exists"]
- **D-02:** In open mode (zero keys), middleware passes through unconditionally, so the first key can be created without having a key — no bootstrapping problem [auto: per-request COUNT check from Phase 12 D-04 handles this naturally]
- **D-03:** Once any key exists, only authenticated users can manage keys — wildcard or scoped keys can create/list/revoke other keys [auto: simplest model, no admin key distinction per REQUIREMENTS.md Out of Scope]

### Scope enforcement strategy
- **D-04:** Scope mismatch returns 403 Forbidden — NOT a silent override. If `AuthContext.allowed_agent_id` is `Some("agent-x")` and the request specifies `agent_id: "agent-y"`, the handler returns 403 [locked by SC1]
- **D-05:** Missing agent_id with scoped key: handler forces agent_id to the key's scope implicitly — no 403, no error. This provides good UX for scoped keys (don't have to repeat agent_id every request) [auto: recommended for ergonomics]
- **D-06:** Wildcard keys (`allowed_agent_id = None`): pass through, use client-supplied agent_id without restriction. All namespaces accessible [auto: matches architecture research §Scoped Keys]
- **D-07:** Open mode (no `AuthContext` in extensions): proceed exactly as before — no scope enforcement, backward-compatible [auto: recommended for zero-regression]

### Scope enforcement per handler
- **D-08:** `POST /memories` — agent_id is optional in body. Scoped key: force agent_id if missing, 403 if mismatched
- **D-09:** `GET /memories` — agent_id is optional in query. Scoped key: force agent_id if missing, 403 if mismatched
- **D-10:** `GET /memories/search` — agent_id is optional in query. Same as D-09
- **D-11:** `POST /memories/compact` — agent_id is required in body. Scoped key: 403 if mismatched (cannot be missing since it's required)
- **D-12:** `DELETE /memories/{id}` — no agent_id in request. Scoped key: look up the memory's agent_id before deleting, return 403 if it doesn't match the key's scope [auto: required for complete namespace isolation]

### 403 Forbidden error variant
- **D-13:** Add `ApiError::Forbidden(String)` variant to error.rs — maps to HTTP 403 [deferred from Phase 10 D-10]
- **D-14:** 403 response body: `{ "error": "forbidden", "detail": "key scoped to agent-x cannot access agent-y" }` — includes specific agent_ids for debuggability [auto: matches existing error pattern with actionable messages]

### Key endpoint response formats
- **D-15:** `POST /keys` accepts `{ "name": "...", "agent_id": "..." }` — name required, agent_id optional (NULL = wildcard). Returns 201 with `{ "key": { id, name, display_id, agent_id, created_at }, "raw_token": "mnk_..." }` [auto: follows existing POST /memories 201 pattern + show-once token]
- **D-16:** `GET /keys` takes no parameters. Returns 200 with `{ "keys": [ { id, name, display_id, agent_id, created_at, revoked_at }, ... ] }` — never includes raw token or hashed_key [auto: follows existing list pattern]
- **D-17:** `DELETE /keys/:id` revokes the key (soft delete). Returns 200 with `{ "revoked": true, "id": "..." }` [auto: matches Phase 11 D-14 soft delete, existing patterns]

### Router structure
- **D-18:** Key routes added to the protected router group alongside memory routes — single `route_layer()` covers both
- **D-19:** Route paths: `/keys` for POST and GET, `/keys/{id}` for DELETE — flat, no nesting under `/admin` or `/auth` [auto: simplest API surface]

### Claude's Discretion
- Handler function organization (scope enforcement as inline logic vs. extracted helper)
- Whether to use `Option<Extension<AuthContext>>` or manual extension extraction for optional auth context
- Test organization (integration tests for scope enforcement scenarios)
- Whether to deserialize key creation body into a dedicated struct or reuse inline serde

</decisions>

<specifics>
## Specific Ideas

- The scope enforcement helper (if extracted) could be a simple function: `fn enforce_scope(auth: &Option<AuthContext>, requested: &Option<String>) -> Result<Option<String>, ApiError>` — returns the effective agent_id or 403
- For `DELETE /memories/{id}` with a scoped key, the handler calls `service.get_memory(id)` (or equivalent) to check ownership before calling `service.delete_memory(id)` — two DB queries but ensures isolation
- The PITFALLS.md checklist item is explicit: "Test: use key for agent-A, send agent_id: agent-B in body, assert 403" — this exact test must exist
- Key creation response must NOT log the raw token via tracing (PITFALLS.md: "Log only key ID and agent_id; print full key only to stdout")

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Scope enforcement architecture
- `.planning/research/ARCHITECTURE.md` §Scoped Keys and Existing agent_id Filtering — Override pattern, wildcard keys, compaction scoping
- `.planning/research/ARCHITECTURE.md` §Pattern 2: AuthContext via Request Extensions — Extension extraction pattern
- `.planning/research/ARCHITECTURE.md` §Anti-Pattern 4: Scope Enforcement in Service Layer — Why scope stays in handlers

### Security pitfalls
- `.planning/research/PITFALLS.md` §Auth Pitfall 6 (Scope enforcement gap) — Handlers must use Extension, not request body agent_id
- `.planning/research/PITFALLS.md` §"Looks Done But Isn't" Checklist — Scope enforcement closes the loop, key creation logging

### Requirements and success criteria
- `.planning/REQUIREMENTS.md` — AUTH-04 (scoped key overrides client agent_id), INFRA-03 (startup log — already done)
- `.planning/ROADMAP.md` §Phase 13 — 5 success criteria

### Prior phase decisions (already built)
- `.planning/phases/10-auth-schema-foundation/10-CONTEXT.md` — D-07/D-08 (Unauthorized variant), D-10 (Forbidden deferred to Phase 13)
- `.planning/phases/11-keyservice-core/11-CONTEXT.md` — D-01 through D-19 (KeyService API, token format, validation)
- `.planning/phases/12-auth-middleware/12-CONTEXT.md` — D-01 through D-14 (middleware placement, open mode, header parsing)

### Existing code
- `src/auth.rs` — `KeyService` (create/list/revoke/validate), `AuthContext`, `auth_middleware`
- `src/server.rs` — `build_router()` with protected/public split, `AppState`, all handler functions
- `src/error.rs` — `ApiError` enum (BadRequest, NotFound, Unauthorized, Internal) — needs Forbidden variant
- `src/service.rs` — `CreateMemoryRequest`, `SearchParams`, `ListParams` with agent_id fields
- `src/compaction.rs` — `CompactRequest` with required agent_id field

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `KeyService::create(name, agent_id)` — Returns `(ApiKey, raw_token)`. Handler wraps this in JSON response.
- `KeyService::list()` — Returns `Vec<ApiKey>`. Handler serializes to JSON array.
- `KeyService::revoke(id)` — Returns `()`. Handler returns confirmation JSON.
- `AuthContext { key_id, allowed_agent_id }` — Injected by middleware into request extensions. Handlers extract this optionally.
- `ApiError::Unauthorized(String)` — Existing 401 pattern. `Forbidden(String)` follows same structure.

### Established Patterns
- All services accessed via `Arc<T>` in `AppState` — `key_service` already wired
- Handler functions follow `async fn handler(State(state): State<AppState>, ...) -> Result<..., ApiError>` pattern
- Protected routes use `route_layer(middleware::from_fn_with_state(...))` — new key routes added to same group
- Error responses return `(StatusCode, Json(json!({...})))` via `IntoResponse`

### Integration Points
- `server.rs::build_router()` — Add `/keys` and `/keys/{id}` routes to the `protected` router group
- `server.rs` — Add three new handler functions: `create_key_handler`, `list_keys_handler`, `revoke_key_handler`
- `server.rs` — Modify all five existing memory/compaction handlers to optionally extract `AuthContext` and enforce scope
- `error.rs::ApiError` — Add `Forbidden(String)` variant and update `IntoResponse` impl
- `service.rs` — May need a `get_memory(id)` or similar for delete ownership verification (or handler queries DB directly via service)

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 13-http-wiring-and-rest-key-endpoints*
*Context gathered: 2026-03-20*
*Mode: auto — all gray areas resolved with recommended defaults*
