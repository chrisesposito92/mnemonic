# Phase 9: HTTP Integration - Research

**Researched:** 2026-03-20
**Domain:** Axum HTTP handler wiring, request validation, AppState extension, integration testing
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Request validation**
- Handler rejects empty `agent_id` with 400 BadRequest before calling compact() — consistent with how POST /memories rejects empty content
- Handler rejects `threshold` outside 0.0–1.0 range with 400 BadRequest — out-of-range cosine similarity thresholds are nonsensical
- Validation happens at the handler level, not inside CompactionService

**HTTP status codes**
- 200 OK for all successful compactions, including 0 clusters found
- Response body contains `clusters_found`, `memories_merged`, `memories_created` counts
- No 204 special case — simple and consistent with other endpoints

**AppState wiring**
- Add `Arc<CompactionService>` to AppState struct
- In main.rs, rename `_compaction` to `compaction` and pass into AppState
- Remove `#![allow(dead_code)]` from compaction.rs — all items now consumed
- Add `POST /memories/compact` route to build_router()

**Handler pattern**
- Follow existing pattern exactly: `State(state)` → validation → `state.compaction.compact(req).await?` → `Json(response)`
- CompactRequest deserialized from JSON body via `Json<CompactRequest>`
- CompactResponse serialized directly (already derives serde::Serialize)

**Integration test scope**
- Test HTTP wiring + key scenarios, not re-test clustering logic (Phase 8 covers that)
- HTTP-layer tests:
  1. Successful compact returns 200 with correct JSON shape (run_id, clusters_found, id_mapping, etc.)
  2. dry_run via HTTP returns 200 with no DB changes (verify GET /memories count unchanged)
  3. Agent isolation via HTTP — compact Agent A, verify Agent B untouched
  4. Validation errors: missing agent_id → 400, empty agent_id → 400, threshold out of range → 400

- Use existing test infrastructure: build_test_state() extended with CompactionService, json_request(), response_json()

### Claude's Discretion

- max_candidates = 0 validation (likely reject with 400)
- Exact error message wording for validation failures
- Whether to extend build_test_state() or create a separate build_test_compact_state() helper
- Handler function naming (compact_handler vs compact_memories_handler)

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| API-01 | Agent can trigger memory compaction via POST /memories/compact with required agent_id | Handler wiring in server.rs, AppState extension, route registration |
| API-02 | Agent can preview compaction results without committing via dry_run parameter | dry_run already implemented in CompactionService.compact(); handler passes through via deserialized CompactRequest |
| API-03 | Compaction response includes stats (clusters_found, memories_merged, memories_created) | CompactResponse already has these fields and derives Serialize; handler returns Json(response) |
| API-04 | Compaction response includes old-to-new ID mapping for each merged cluster | id_mapping: Vec<ClusterMapping> already in CompactResponse; handler passes through automatically |
</phase_requirements>

---

## Summary

Phase 9 is a pure HTTP wiring phase. The CompactionService is fully implemented and tested at the service layer (Phase 8). This phase adds exactly three things: (1) an `Arc<CompactionService>` field on `AppState`, (2) a `POST /memories/compact` handler that validates and delegates to it, and (3) HTTP-layer integration tests covering the four locked scenarios. No new external dependencies are required.

The existing code establishes a clear, mechanical pattern for adding handlers: `create_memory_handler` in `server.rs` is the direct analog. The validation logic mirrors how `EmbeddingError::EmptyInput` is converted to `ApiError::BadRequest` — except validation here is explicit pre-service-call checks rather than a From conversion. AppState is a simple struct clone, so adding a field is a localized change with a predictable ripple to `build_test_state()` and `main.rs`.

The integration test strategy must use `build_test_compact_state()` (a new helper returning both `AppState` with `CompactionService` and a bare `MemoryService` for seeding). Each HTTP-layer test follows: seed data via service → call via `app.oneshot()` → assert status + JSON body. Agent isolation and dry_run tests also do a follow-up `GET /memories` assertion to confirm DB state.

**Primary recommendation:** Follow `create_memory_handler` as the exact template. Add `compaction` to `AppState`. Wire the route. Write four integration test functions. Remove `#![allow(dead_code)]`.

---

## Standard Stack

### Core (already present — no new dependencies needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.8 | HTTP routing and handler extraction | Already in use; all handler infrastructure present |
| serde / serde_json | 1 | JSON body deserialization/serialization | Already derives on CompactRequest/CompactResponse |
| tokio | 1 (full) | Async runtime | Already in use |
| tower | 0.5 | `ServiceExt::oneshot` for test request dispatch | Already in dev-dependencies |
| http-body-util | 0.1 | `BodyExt::collect` for test response body reading | Already in dev-dependencies |

**No new dependencies.** All required crates are already in `Cargo.toml`.

---

## Architecture Patterns

### Established Handler Pattern (from server.rs:46-52)

The `create_memory_handler` is the direct blueprint:

```rust
// Source: src/server.rs lines 46-52
async fn create_memory_handler(
    State(state): State<AppState>,
    Json(body): Json<CreateMemoryRequest>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), ApiError> {
    let memory = state.service.create_memory(body).await?;
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::to_value(memory).unwrap())))
}
```

The compact handler differs in two ways: it returns 200 (not 201), and it has explicit pre-service validation. Template:

```rust
// Pattern for compact_handler
async fn compact_handler(
    State(state): State<AppState>,
    Json(body): Json<CompactRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Validation happens HERE before calling service
    if body.agent_id.trim().is_empty() {
        return Err(ApiError::BadRequest("agent_id must not be empty".to_string()));
    }
    if let Some(t) = body.threshold {
        if !(0.0..=1.0).contains(&t) {
            return Err(ApiError::BadRequest("threshold must be between 0.0 and 1.0".to_string()));
        }
    }
    let response = state.compaction.compact(body).await?;
    Ok(Json(serde_json::to_value(response).unwrap()))
}
```

### AppState Extension Pattern

```rust
// Source: src/server.rs lines 25-28 — current state
#[derive(Clone)]
pub struct AppState {
    pub service: std::sync::Arc<crate::service::MemoryService>,
    // ADD:
    pub compaction: std::sync::Arc<crate::compaction::CompactionService>,
}
```

### Route Registration Pattern

```rust
// Source: src/server.rs lines 31-38
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/memories", post(create_memory_handler).get(list_memories_handler))
        .route("/memories/search", get(search_memories_handler))
        .route("/memories/{id}", delete(delete_memory_handler))
        // ADD:
        .route("/memories/compact", post(compact_handler))
        .with_state(state)
}
```

### main.rs Wiring (lines 111-123)

```rust
// Current (src/main.rs lines 110-123):
let _compaction = std::sync::Arc::new(
    compaction::CompactionService::new(
        db_arc.clone(), embedding.clone(), llm_engine, embedding_model.clone(),
    )
);
let state = server::AppState { service };

// After Phase 9:
let compaction = std::sync::Arc::new(
    compaction::CompactionService::new(
        db_arc.clone(), embedding.clone(), llm_engine, embedding_model.clone(),
    )
);
let state = server::AppState { service, compaction };
```

### Test Infrastructure Extension

The existing `build_test_state()` returns `(AppState, Arc<MemoryService>)`. Phase 9 needs a variant that includes `CompactionService`. The cleanest approach (Claude's discretion) is a new `build_test_compact_state()` helper that mirrors `build_test_compaction()` from the service-layer tests but returns `AppState` instead of bare services:

```rust
// Pattern mirrors tests/integration.rs lines 840-858
async fn build_test_compact_state() -> (AppState, Arc<MemoryService>, Arc<CompactionService>) {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();
    let db = Arc::new(conn);
    let embedding: Arc<dyn mnemonic::embedding::EmbeddingEngine> = Arc::new(MockEmbeddingEngine);
    let service = Arc::new(MemoryService::new(db.clone(), embedding.clone(), "mock-model".to_string()));
    let compaction = Arc::new(CompactionService::new(db.clone(), embedding.clone(), None, "mock-model".to_string()));
    let state = AppState { service: service.clone(), compaction: compaction.clone() };
    (state, service, compaction)
}
```

### Integration Test Pattern (HTTP layer)

```rust
// Pattern: seed via service → HTTP call via oneshot → assert status + JSON body
#[tokio::test]
async fn test_compact_http_basic() {
    let (state, service, _) = build_test_compact_state().await;
    // Seed data via service (bypasses HTTP layer for setup)
    service.create_memory(CreateMemoryRequest { ... }).await.unwrap();
    // Route the HTTP request
    let app = build_router(state);
    let response = app
        .oneshot(json_request("POST", "/memories/compact", serde_json::json!({
            "agent_id": "agent-1",
            "threshold": 0.5
        })))
        .await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert!(json["run_id"].is_string());
    assert!(json["clusters_found"].is_number());
    assert!(json["id_mapping"].is_array());
}
```

### Anti-Patterns to Avoid

- **Testing clustering logic in HTTP tests:** Phase 8 already covers DEDUP-01 through DEDUP-04 at the service level. HTTP tests verify the HTTP contract (status codes, JSON shape, wiring) only.
- **Re-seeding with `build_test_app()`:** `build_test_app()` discards the service handle. HTTP tests that need to seed data MUST use `build_test_compact_state()` to retain the `Arc<MemoryService>`.
- **Calling `build_router(state.clone())` after oneshot:** axum's `oneshot()` consumes the router. Each test call needs a fresh `build_router(state.clone())` or restructure with `state.clone()` before the call.
- **Routing conflict with `/memories/{id}`:** Adding `/memories/compact` BEFORE or AFTER the parameterized route does not matter in axum 0.8 because literal segments take precedence over path params. However, register the new route and verify with a test.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON request body extraction | Custom body parsing | `Json<CompactRequest>` extractor | axum handles content-type checks, deserialization errors (returns 422 automatically) |
| JSON error responses | Manual response building | `ApiError::BadRequest(msg).into_response()` | IntoResponse impl already in error.rs; format: `{"error": "..."}` |
| HTTP test client | reqwest or custom client | `tower::ServiceExt::oneshot` | Already used throughout; zero network overhead |
| Response body parsing in tests | Manual bytes → string → json | `response_json()` helper (integration.rs:490-493) | Already exists, reusable |
| 22-field request struct | Custom parser | Deserialize from `Json<CompactRequest>` | CompactRequest already derives Deserialize; serde handles missing optional fields |

**Key insight:** axum's `Json<T>` extractor returns HTTP 422 for malformed JSON automatically before the handler even runs. The handler's explicit validation only handles semantic errors (empty string, out-of-range float), not structural JSON errors.

---

## Common Pitfalls

### Pitfall 1: `agent_id` Field Missing vs Empty

**What goes wrong:** If `agent_id` is absent from JSON, serde will fail deserialization (CompactRequest.agent_id is `String`, not `Option<String>`) — axum returns 422 automatically. The handler's 400 check is for the empty-string case (`"agent_id": ""`), which serde allows.

**Why it happens:** Confusing "missing field" (serde/axum error) with "empty field" (business rule).

**How to avoid:** Test both cases explicitly — one test with no `agent_id` key (expect 422 or 400 depending on axum version behavior) and one with `"agent_id": ""` (expect 400 from handler validation).

**Warning signs:** Test for missing agent_id passes but returns 422 instead of 400 — acceptable, but document the distinction.

### Pitfall 2: AppState Clone and Router Ownership

**What goes wrong:** `app.oneshot()` takes ownership of the router. A second call on the same `app` variable won't compile.

**Why it happens:** Tower's `oneshot()` consumes `self`.

**How to avoid:** Build a fresh router per call: `build_router(state.clone())`. Since `AppState` is `#[derive(Clone)]`, this is cheap (only Arcs are cloned).

### Pitfall 3: Route Ordering for `/memories/compact` vs `/memories/{id}`

**What goes wrong:** Some developers assume parameterized routes might capture the literal string "compact" as an `id`.

**Why it happens:** Unfamiliarity with axum's routing priority.

**How to avoid (verified):** axum 0.8 gives literal path segments priority over parameterized ones. No special ordering required. A smoke test (`GET /memories/compact` returning 405 Method Not Allowed, not delegating to delete_memory_handler) confirms correctness.

### Pitfall 4: `#![allow(dead_code)]` Removal Exposes Real Warnings

**What goes wrong:** Removing the dead_code allow from compaction.rs may reveal actual unused items that need to be addressed.

**Why it happens:** The allow attribute was a blanket suppression, not targeted. Once removed, the compiler may flag individual items.

**How to avoid:** After removing the allow, run `cargo build` and address any new warnings before proceeding. Expected: zero new warnings since the handler will use all public items.

### Pitfall 5: `threshold` Validation Range as f32

**What goes wrong:** Floating-point comparison with `contains(&t)` on a range may produce surprising behavior at exact boundary values (0.0 and 1.0) due to f32 representation.

**Why it happens:** f32 precision.

**How to avoid:** Use `!(0.0..=1.0).contains(&t)` — this is correct Rust range syntax and handles boundaries as expected for IEEE 754.

---

## Code Examples

### Correct ApiError::BadRequest Usage

```rust
// Source: src/error.rs lines 89-100 — IntoResponse impl
// Returns: HTTP 400 with body {"error": "<message>"}
return Err(ApiError::BadRequest("agent_id must not be empty".to_string()));
```

### Correct Response Serialization (from server.rs)

```rust
// Source: src/server.rs lines 58-61 — list_memories_handler
// Pattern: serde_json::to_value() + unwrap() is safe because types are known good
Ok(Json(serde_json::to_value(response).unwrap()))
```

### Seeding + HTTP Call Test Pattern

```rust
// Pattern from tests/integration.rs lines 556-598 — test_list_memories
let (state, service) = build_test_state().await;
service.create_memory(CreateMemoryRequest { ... }).await.unwrap();
let app = build_router(state.clone());
let response = app.oneshot(request).await.unwrap();
```

---

## State of the Art

| Old Approach | Current Approach | Status |
|--------------|------------------|--------|
| `_compaction` unused in main.rs | `compaction` wired into AppState | Phase 9 completes this |
| `#![allow(dead_code)]` blanket suppress | All compaction items consumed | Phase 9 removes this |
| CompactionService tests: service-layer only | HTTP-layer integration tests added | Phase 9 adds HTTP layer |

---

## Open Questions

1. **max_candidates = 0 validation**
   - What we know: CompactionService defaults max_candidates to 100 when None; a value of 0 would fetch only the 0+1=1 candidate but then truncate to 0 candidates, producing empty result
   - What's unclear: Should the handler reject 0 explicitly with 400, or let the service handle it gracefully (returning 0 clusters)?
   - Recommendation: Reject max_candidates = 0 with 400 — it is semantically nonsensical and consistent with the threshold validation approach. Use `if let Some(m) = body.max_candidates { if m == 0 { return Err(ApiError::BadRequest(...)) } }`

2. **Handler function name**
   - What we know: Existing handlers use the pattern `<verb>_<resource>_handler` (create_memory_handler, list_memories_handler, delete_memory_handler)
   - Recommendation: Use `compact_memories_handler` — more specific than `compact_handler` and consistent with naming convention

3. **Whether to extend build_test_state() or create build_test_compact_state()**
   - What we know: Existing `build_test_state()` returns `(AppState, Arc<MemoryService>)` — extending it would require changing its return type, breaking all callers
   - Recommendation: Create a separate `build_test_compact_state()` returning `(AppState, Arc<MemoryService>, Arc<CompactionService>)` — no existing tests are broken, the helper is purpose-built for HTTP compaction tests

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test runner + tokio-test |
| Config file | Cargo.toml [dev-dependencies] |
| Quick run command | `cargo test --test integration compact -- --nocapture` |
| Full suite command | `cargo test` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| API-01 | POST /memories/compact with agent_id returns 200 JSON | integration | `cargo test --test integration test_compact_http_basic` | ❌ Wave 0 |
| API-02 | dry_run=true returns 200, GET /memories count unchanged | integration | `cargo test --test integration test_compact_http_dry_run` | ❌ Wave 0 |
| API-03 | Response JSON has clusters_found, memories_merged, memories_created | integration | `cargo test --test integration test_compact_http_basic` | ❌ Wave 0 |
| API-04 | Response JSON has id_mapping with source_ids and new_id | integration | `cargo test --test integration test_compact_http_basic` | ❌ Wave 0 |
| (validation) | empty agent_id → 400 | integration | `cargo test --test integration test_compact_http_validation` | ❌ Wave 0 |
| (validation) | threshold out of range → 400 | integration | `cargo test --test integration test_compact_http_validation` | ❌ Wave 0 |
| (isolation) | compact Agent A, Agent B count unchanged | integration | `cargo test --test integration test_compact_http_agent_isolation` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --test integration compact`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/integration.rs` — add `build_test_compact_state()` helper and four HTTP-layer test functions
- [ ] No new test files needed — all additions go into the existing `tests/integration.rs`
- [ ] No new framework install needed — existing Cargo.toml dev-dependencies cover all requirements

---

## Sources

### Primary (HIGH confidence)

- Direct source code inspection: `src/server.rs` — AppState struct, build_router(), all handler patterns (lines 25-92)
- Direct source code inspection: `src/compaction.rs` — CompactRequest/CompactResponse types, CompactionService::compact() signature (lines 1-448)
- Direct source code inspection: `src/error.rs` — ApiError variants, IntoResponse impl (lines 78-116)
- Direct source code inspection: `src/main.rs` — CompactionService construction, AppState wiring (lines 110-127)
- Direct source code inspection: `tests/integration.rs` — build_test_state(), json_request(), response_json(), build_test_compaction() (lines 453-858)
- Direct source code inspection: `Cargo.toml` — all dependencies and versions confirmed present

### Secondary (MEDIUM confidence)

- Axum 0.8 routing priority (literal over parameterized): consistent with axum documentation pattern and validated by reviewing existing route registration in build_router()

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies already present, verified in Cargo.toml
- Architecture: HIGH — all patterns are direct copies from existing production handlers in server.rs
- Pitfalls: HIGH — derived from direct code inspection of existing type signatures, axum 0.8 behavior, and compaction.rs logic
- Test patterns: HIGH — integration.rs test infrastructure fully inspected and understood

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable stack; no moving parts)
