# Phase 28: Core RPC Handlers, Health, and Discoverability - Research

**Researched:** 2026-03-22
**Domain:** tonic gRPC handler implementation, tonic-reflection, tonic-health, prost-types timestamp conversion
**Confidence:** HIGH

## Summary

Phase 28 completes the gRPC interface by replacing the four stub `unimplemented!` handlers in `src/grpc/mod.rs` with real implementations that delegate to `MemoryService`, wire scope enforcement via `enforce_scope()`, map `ApiError` to `tonic::Status`, and register tonic-reflection for grpcurl discoverability. All seven requirements (GRPC-01 through GRPC-05, HEALTH-01, HEALTH-02) are well-defined and the pattern is a direct mirror of the already-proven REST handlers in `src/server.rs`.

The only non-trivial technical work is (1) the `build.rs` change to emit a file descriptor set for reflection, (2) the `prost_types::Timestamp` conversion from the ISO 8601 string stored in the DB, and (3) the `memory_to_proto()` helper that bridges the service-layer `Memory` struct to the proto `Memory` message. Every other piece — auth extraction, scope enforcement, service delegation, error mapping — has an exact REST analogue already in production.

tonic-reflection 0.13 uses `Builder::configure().register_encoded_file_descriptor_set(BYTES).build_v1()?` pattern. tonic-health 0.13 is already wired in Phase 27's `serve_grpc()` and only needs a verification test. `prost_types::Timestamp` implements `FromStr` for RFC 3339 strings, meaning the `"YYYY-MM-DDTHH:MM:SS.fffZ"` format stored in the DB parses directly with `.parse::<prost_types::Timestamp>()` — no chrono dependency needed.

**Primary recommendation:** Implement handlers in `src/grpc/mod.rs` (or a thin `src/grpc/handlers.rs` submodule), wire reflection via a 3-step build.rs + include macro + add_service change, and cover every handler with an integration test that asserts `Code::PermissionDenied` for mismatched agent_id.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-01:** Direct mapping from ApiError variants to tonic status codes: BadRequest -> InvalidArgument, NotFound -> NotFound, Forbidden -> PermissionDenied, Unauthorized -> Unauthenticated, Internal -> Internal

**D-02:** Create a helper function `api_error_to_status(e: ApiError) -> tonic::Status` in grpc/mod.rs (not a trait impl) — keeps gRPC-specific mapping inside the grpc module, avoids polluting the error module with tonic dependency

**D-03:** Error messages pass through — the tonic::Status message contains the same user-facing string as the ApiError (e.g., "content must not be empty")

**D-04:** Extract AuthContext from `request.extensions()` — the GrpcAuthLayer (Phase 27) injects AuthContext into http::Request extensions, and tonic::Request exposes these via `.extensions()`

**D-05:** Each handler extracts `Option<&AuthContext>` — None means open mode (no auth), Some means auth is active. Mirrors the REST `Option<Extension<AuthContext>>` pattern exactly.

**D-06:** Reuse the existing `enforce_scope()` logic by moving it from server.rs to a shared location (auth.rs) or duplicating a small gRPC-local version in grpc/mod.rs — the function is 13 lines, duplication is acceptable if moving creates churn

**D-07:** Every handler calls enforce_scope before delegating to MemoryService — not type-enforced, covered by per-handler integration tests

**D-08:** Delete handler follows REST pattern: scoped key requires DB lookup of memory's agent_id before deletion (same ownership verification as REST delete_memory_handler)

**D-09:** Parse created_at ISO 8601 string to prost_types::Timestamp using chrono or manual parsing — created_at is stored as "YYYY-MM-DDTHH:MM:SS.fffZ" format in the DB

**D-10:** If parsing fails, fall back to Timestamp { seconds: 0, nanos: 0 } rather than erroring — a missing timestamp should not prevent returning the memory

**D-11:** Create a `fn memory_to_proto(m: service::Memory) -> proto::Memory` helper in grpc/mod.rs that converts the service-layer Memory struct to the proto-generated Memory message

**D-12:** proto3 defaults apply: empty session_id becomes "" (not optional), empty tags becomes empty repeated field — matches the proto design comment

**D-13:** Add FILE_DESCRIPTOR_SET const via tonic-build's `file_descriptor_set_path` in build.rs, include_bytes! in grpc/mod.rs

**D-14:** Wire tonic-reflection in serve_grpc() via `add_service(tonic_reflection::server::Builder::configure().register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET).build_v1()?)` — makes `grpcurl list` work

**D-15:** tonic-health is already wired in serve_grpc() (Phase 27) — Phase 28 only needs to verify it responds correctly. No new health code needed.

**D-16:** StoreMemory: reject empty content with InvalidArgument (mirrors REST)

**D-17:** SearchMemories: reject empty query with InvalidArgument (mirrors REST)

**D-18:** ListMemories: no required fields — empty agent_id returns all memories (matches REST GET /memories behavior)

**D-19:** DeleteMemory: reject empty id with InvalidArgument; non-existent id returns NotFound

### Claude's Discretion

- Exact function signatures and argument ordering in handler implementations
- Whether to use a separate `handlers.rs` submodule under grpc/ or keep handlers in grpc/mod.rs
- Test structure — integration tests vs unit tests per handler
- Whether to add tracing spans per handler (recommended but not required)

### Deferred Ideas (OUT OF SCOPE)

- gRPC streaming for SearchMemories (STREAM-01) — future requirement, unary sufficient for v1.5
- Compaction/keys over gRPC (GRPC-EXT-01) — explicitly out of scope per PROJECT.md
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| GRPC-01 | StoreMemory RPC accepts content, agent_id, session_id, tags and returns stored memory with ID | `memory_to_proto()` helper + `create_memory()` delegation pattern documented |
| GRPC-02 | SearchMemories RPC accepts query, agent_id, optional session_id/tags, limit and returns ranked results with distances | `SearchResultItem.distance` (f64) maps to `SearchResult.distance` (float/f32); cast required |
| GRPC-03 | ListMemories RPC accepts agent_id, optional session_id/tags, limit/offset and returns memory list | `list_memories()` delegation with `ListParams` construction pattern documented |
| GRPC-04 | DeleteMemory RPC accepts memory ID and returns success/not-found status | Ownership lookup pattern from REST `delete_memory_handler` documented |
| GRPC-05 | Consistent gRPC status code mapping (INVALID_ARGUMENT, NOT_FOUND, UNAUTHENTICATED/PERMISSION_DENIED, INTERNAL) | `api_error_to_status()` mapping table documented with exact `tonic::Code` variants |
| HEALTH-01 | grpc.health.v1 standard health service via tonic-health reporting SERVING status | Already wired in Phase 27 `serve_grpc()`; verification test is the only deliverable |
| HEALTH-02 | tonic-reflection enabled for grpcurl/grpc_cli service discovery | 3-step wiring: build.rs `file_descriptor_set_path` + `include_file_descriptor_set!` macro + `add_service()` in serve_grpc() |
</phase_requirements>

---

## Standard Stack

### Core (All already in Cargo.toml under interface-grpc feature)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tonic | 0.13 | gRPC framework, tonic::Request/Response/Status | Pinned to match prost 0.13 anchor; already in tree |
| prost | 0.13 | Protobuf message encoding/decoding | Pinned to match qdrant-client prost anchor |
| prost-types | 0.13 | `prost_types::Timestamp` for created_at field | Already in interface-grpc feature; implements `FromStr` for RFC 3339 |
| tonic-health | 0.13 | grpc.health.v1 health service | Already wired in serve_grpc(); no new install |
| tonic-reflection | 0.13 | grpcurl `list` discoverability via server reflection | Already in Cargo.toml; needs wiring + build.rs change |
| tonic-build | 0.13 | Proto codegen + file descriptor set generation | Already in build-dependencies; needs `file_descriptor_set_path` addition |

**No new dependencies required.** All crates are already declared in Cargo.toml under the `interface-grpc` feature.

## Architecture Patterns

### Recommended Project Structure
```
src/grpc/
├── mod.rs          # MnemonicGrpcService trait impl (handlers), serve_grpc(), helpers
│                   # (or split handlers into handlers.rs — Claude's discretion)
└── auth.rs         # GrpcAuthLayer / GrpcAuthService (Phase 27, no changes needed)
proto/
└── mnemonic.proto  # Unchanged — no schema changes in Phase 28
build.rs            # Add file_descriptor_set_path for reflection
```

### Pattern 1: api_error_to_status() mapping function

**What:** A free function in grpc/mod.rs that converts ApiError variants to tonic::Status with the exact gRPC code and user-facing message passed through.

**When to use:** Called at the end of every handler to convert `Result<_, ApiError>` to `Result<_, tonic::Status>`.

```rust
// Source: CONTEXT.md D-01, D-02, D-03 + src/error.rs ApiError variants
fn api_error_to_status(e: ApiError) -> tonic::Status {
    match e {
        ApiError::BadRequest(msg) => tonic::Status::invalid_argument(msg),
        ApiError::NotFound         => tonic::Status::not_found("not found"),
        ApiError::Forbidden(msg)   => tonic::Status::permission_denied(msg),
        ApiError::Unauthorized(msg)=> tonic::Status::unauthenticated(msg),
        ApiError::Internal(e)      => {
            tracing::error!(error = %e, "gRPC internal error");
            tonic::Status::internal("internal server error")
        }
    }
}
```

### Pattern 2: AuthContext extraction from tonic::Request extensions

**What:** tonic::Request exposes `request.extensions().get::<T>()` — same as http::Request extensions. GrpcAuthLayer already injected `AuthContext` during the Tower middleware pass.

**When to use:** First line of every handler implementation.

```rust
// Source: CONTEXT.md D-04, D-05 + src/grpc/auth.rs (line 120: req.extensions_mut().insert(auth_ctx))
async fn store_memory(
    &self,
    request: tonic::Request<proto::StoreMemoryRequest>,
) -> Result<tonic::Response<proto::StoreMemoryResponse>, tonic::Status> {
    let auth_ctx = request.extensions().get::<AuthContext>().cloned();
    // auth_ctx: Option<AuthContext> — None = open mode, Some = auth active
    let body = request.into_inner();
    // ...
}
```

### Pattern 3: enforce_scope() for gRPC handlers

**What:** Either duplicate the 13-line function from `src/server.rs:78-95` into grpc/mod.rs, or move it to auth.rs. Per D-06, duplication is acceptable.

**Function signature (from server.rs:78-95):**
```rust
// Source: src/server.rs lines 78-95
fn enforce_scope(
    auth: Option<&AuthContext>,
    requested: Option<&str>,
) -> Result<Option<String>, ApiError> {
    match auth {
        None => Ok(None),
        Some(ctx) => match &ctx.allowed_agent_id {
            None => Ok(requested.map(str::to_string)),
            Some(allowed) => match requested {
                None => Ok(Some(allowed.clone())),
                Some(req_id) if req_id == allowed.as_str() => Ok(Some(allowed.clone())),
                Some(req_id) => Err(ApiError::Forbidden(format!(
                    "key scoped to {} cannot access {}", allowed, req_id
                ))),
            },
        },
    }
}
```

**gRPC adapter:** wrap with `.map_err(api_error_to_status)?`

### Pattern 4: memory_to_proto() conversion helper

**What:** Converts service-layer `service::Memory` (has `created_at: String`) to proto-generated `proto::Memory` (has `created_at: Option<prost_types::Timestamp>`).

**Key insight:** `prost_types::Timestamp` implements `FromStr` for RFC 3339 strings. The DB stores `"YYYY-MM-DDTHH:MM:SS.fffZ"` which is valid RFC 3339. No chrono dependency needed.

```rust
// Source: CONTEXT.md D-11, D-12 + prost-types 0.13 docs (FromStr for RFC 3339)
// + proto/mnemonic.proto Memory message fields
fn memory_to_proto(m: crate::service::Memory) -> proto::Memory {
    let created_at = m.created_at
        .parse::<prost_types::Timestamp>()
        .ok();  // D-10: fall back to None on parse failure; proto optional field sends default
    proto::Memory {
        id: m.id,
        content: m.content,
        agent_id: m.agent_id,
        session_id: m.session_id,   // proto3 default: "" if empty — matches D-12
        tags: m.tags,               // proto3 default: empty repeated field — matches D-12
        created_at,
        embedding_model: m.embedding_model,
    }
}
```

Note: The proto `Memory.created_at` field is `google.protobuf.Timestamp` which prost generates as `Option<prost_types::Timestamp>`. Passing `None` is safe and sends the protobuf default (zero value).

### Pattern 5: Reflection wiring — three-step change

**Step 1 — build.rs:** Add `file_descriptor_set_path` to the tonic_build configure call.

```rust
// Source: tonic-build 0.13 docs + hyperium/tonic examples/build.rs
fn main() {
    if std::env::var("CARGO_FEATURE_INTERFACE_GRPC").is_err() {
        return;
    }
    println!("cargo:rerun-if-changed=proto/mnemonic.proto");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("mnemonic_descriptor.bin"))
        .compile_protos(&["proto/mnemonic.proto"], &["proto"])
        .expect("Failed to compile proto/mnemonic.proto");
}
```

Note: The current build.rs uses `tonic_build::compile_protos()` (shorthand). To use `file_descriptor_set_path`, switch to `tonic_build::configure().file_descriptor_set_path(...).compile_protos(...)`.

**Step 2 — grpc/mod.rs:** Expose the descriptor bytes as a constant.

```rust
// Source: hyperium/tonic examples/src/reflection/server.rs (v0.13.0 tag)
// tonic::include_file_descriptor_set!("mnemonic_descriptor") expands to
// include_bytes!(concat!(env!("OUT_DIR"), "/mnemonic_descriptor.bin"))
pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("mnemonic_descriptor");
```

**Step 3 — serve_grpc():** Register the reflection service.

```rust
// Source: hyperium/tonic examples/src/reflection/server.rs (v0.13.0 tag)
// CONTEXT.md D-14
let reflection_service = tonic_reflection::server::Builder::configure()
    .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
    .build_v1()
    .expect("failed to build reflection service");

tonic::transport::Server::builder()
    .layer(auth::GrpcAuthLayer { key_service: Arc::clone(&svc.key_service) })
    .add_service(health_service)
    .add_service(reflection_service)
    .add_service(MnemonicServiceServer::new(svc))
    .serve(addr)
    .await?;
```

**Reflection bypass in GrpcAuthLayer:** The existing auth.rs only bypasses `/grpc.health.v1.Health/`. Reflection requests go to `/grpc.reflection.v1.ServerReflection/`. If auth is active and a client calls `grpcurl list`, it will receive `Unauthenticated` unless the path is also bypassed or a valid token is provided. The success criterion (D-5 in the phase) does not specify unauthenticated reflection, so no bypass is required unless a test uses `grpcurl` without a token.

### Pattern 6: DeleteMemory ownership check (scoped keys)

**What:** Mirror the REST `delete_memory_handler` ownership verification for scoped API keys.

```rust
// Source: src/server.rs lines 148-172
async fn delete_memory(
    &self,
    request: tonic::Request<proto::DeleteMemoryRequest>,
) -> Result<tonic::Response<proto::DeleteMemoryResponse>, tonic::Status> {
    let auth_ctx = request.extensions().get::<AuthContext>().cloned();
    let body = request.into_inner();

    if body.id.is_empty() {
        return Err(tonic::Status::invalid_argument("id must not be empty"));
    }

    // Scope enforcement for scoped keys: verify memory ownership (D-08)
    if let Some(ref ctx) = auth_ctx {
        if let Some(ref allowed_id) = ctx.allowed_agent_id {
            match self.memory_service.get_memory_agent_id(&body.id).await
                .map_err(|e| api_error_to_status(ApiError::from(e)))?
            {
                None => return Err(tonic::Status::not_found("not found")),
                Some(ref mem_agent_id) if mem_agent_id != allowed_id => {
                    return Err(tonic::Status::permission_denied(format!(
                        "key scoped to {} cannot access {}", allowed_id, mem_agent_id
                    )));
                }
                Some(_) => {}
            }
        }
    }

    let memory = self.memory_service.delete_memory(body.id).await
        .map_err(api_error_to_status)?;
    Ok(tonic::Response::new(proto::DeleteMemoryResponse {
        memory: Some(memory_to_proto(memory)),
    }))
}
```

### Pattern 7: SearchMemories f64 -> f32 distance cast

**What:** `service::SearchResultItem.distance` is `f64` (matches Qdrant/SQLite backends). The proto `SearchResult.distance` field is `float` (f32). Explicit cast is required.

```rust
// Source: proto/mnemonic.proto line 52 (float distance = 2)
// + src/service.rs line 84 (pub distance: f64)
proto::SearchResult {
    memory: Some(memory_to_proto(item.memory)),
    distance: item.distance as f32,
}
```

### Anti-Patterns to Avoid

- **Skipping enforce_scope on any handler:** Not type-enforced; each handler must call it explicitly before delegating. Per STATE.md critical research flag: add per-handler integration test asserting `Code::PermissionDenied`.
- **Using `tonic_build::compile_protos()` shorthand with reflection:** The shorthand does not accept `file_descriptor_set_path`. Switch to `tonic_build::configure().file_descriptor_set_path(...).compile_protos(...)`.
- **Calling `chrono::DateTime::parse_from_rfc3339()` for timestamp parsing:** Not needed. `prost_types::Timestamp` implements `FromStr` directly for RFC 3339; no chrono dependency required.
- **Wiring reflection with `build_v1alpha()`:** Use `build_v1()`. grpcurl 1.x uses reflection v1; v1alpha is for backward compatibility with older clients only.
- **Not bypassing reflection in GrpcAuthLayer:** If HEALTH-02 verification uses `grpcurl list` without a Bearer token while auth is active, it will fail. Bypass must either be added to auth.rs or test must provide a valid token.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| gRPC status code mapping | Custom enum or match tree spread across modules | `api_error_to_status()` helper in grpc/mod.rs | All ApiError -> Status mappings in one place; mirrors axum IntoResponse |
| Timestamp conversion | Manual string parsing / splitting on "T" and "Z" | `str.parse::<prost_types::Timestamp>()` | `prost_types::Timestamp` implements FromStr for RFC 3339; zero extra deps |
| Scope enforcement logic | New enforcement function in grpc/ | Duplicate or move `enforce_scope()` from server.rs | 13 lines, already proven by REST tests, has correct edge cases |
| File descriptor loading | Manual `include_bytes!` with hard-coded OUT_DIR path | `tonic::include_file_descriptor_set!("mnemonic_descriptor")` | Macro handles OUT_DIR env var resolution correctly for all build targets |
| Service discovery | Advertising services via hard-coded string list | `tonic-reflection` + `use_all_service_names` (default) | Reflection auto-discovers all registered services from the descriptor set |

**Key insight:** Every hard problem in this phase already has a solution in either the service layer (business logic), server.rs (REST patterns to mirror), or tonic's own ecosystem (reflection, health, timestamp).

## Common Pitfalls

### Pitfall 1: build.rs compile_protos shorthand doesn't support file_descriptor_set_path
**What goes wrong:** `tonic_build::compile_protos("proto/mnemonic.proto")` succeeds but generates no descriptor `.bin` file. `include_file_descriptor_set!` panics at compile time with "file not found" in OUT_DIR.
**Why it happens:** The shorthand is a convenience function that calls `configure().compile_protos()` without `file_descriptor_set_path`. Only the configure() builder supports it.
**How to avoid:** Switch to `tonic_build::configure().file_descriptor_set_path(out_dir.join("mnemonic_descriptor.bin")).compile_protos(&["proto/mnemonic.proto"], &["proto"])`.
**Warning signs:** Compiler error mentioning OUT_DIR and "mnemonic_descriptor.bin" not found.

### Pitfall 2: tonic::Request extensions vs tonic::Request metadata
**What goes wrong:** Handler reads `request.metadata().get("authorization")` instead of `request.extensions().get::<AuthContext>()`. Gets None even with valid auth.
**Why it happens:** The Tower layer operates on `http::Request`, injecting into http extensions. tonic wraps this but the injected type is accessed via `.extensions()`, not `.metadata()`.
**How to avoid:** Always use `request.extensions().get::<AuthContext>()` as established by the GrpcAuthLayer design (src/grpc/auth.rs comment, line 9: "Must use clone+swap pattern").
**Warning signs:** All handlers return Unauthenticated even in open mode; auth tests fail.

### Pitfall 3: Reflection auth bypass not covering /grpc.reflection.v1.ServerReflection/
**What goes wrong:** grpcurl list fails with `Failed to list services: rpc error: code = Unauthenticated` when API keys are configured and no token is provided to grpcurl.
**Why it happens:** GrpcAuthLayer only bypasses `/grpc.health.v1.Health/` paths. Reflection service paths start with `/grpc.reflection.v1.ServerReflection/`.
**How to avoid:** Either (a) add a reflection path bypass to GrpcAuthLayer, or (b) ensure all reflection verification tests and documentation instruct users to provide a Bearer token with grpcurl. Success criterion 5 ("grpcurl -plaintext localhost:50051 list") does not specify unauthenticated access — if auth is inactive (open mode) this passes as-is.
**Warning signs:** `grpcurl list` works in open mode but fails when any API key exists.

### Pitfall 4: enforce_scope not called on ListMemories handler
**What goes wrong:** A scoped key for agent-A can list memories belonging to agent-B because ListMemories has no required field (easy to forget the scope enforcement step).
**Why it happens:** ListMemories is the only handler with no required field — the empty agent_id case is handled differently — and it's easy to skip the enforce_scope call.
**How to avoid:** Call `enforce_scope(auth_ctx.as_ref(), Some(body.agent_id.as_str()).filter(|s| !s.is_empty()))` before constructing ListParams. Add a per-handler integration test asserting Code::PermissionDenied.
**Warning signs:** Auth test for ListMemories with mismatched agent_id returns Code::Ok with wrong results instead of Code::PermissionDenied.

### Pitfall 5: f64 distance not cast to f32
**What goes wrong:** Rust type error: `mismatched types expected f32, found f64` when constructing `proto::SearchResult { distance: item.distance, ... }`.
**Why it happens:** service::SearchResultItem.distance is f64 (matches vector DB output precision). Proto float is f32. Rust does not implicit-cast.
**How to avoid:** Use `item.distance as f32` explicitly.
**Warning signs:** Compile error in search_memories handler on the distance field.

### Pitfall 6: created_at parse failure silently produces wrong timestamp
**What goes wrong:** `Timestamp { seconds: 0, nanos: 0 }` is the epoch (1970-01-01T00:00:00Z). If DB stored format does not match RFC 3339 exactly (e.g. missing timezone suffix), `.parse()` returns Err and the fallback produces epoch time.
**Why it happens:** D-10 specifies fallback to `{seconds: 0, nanos: 0}` but `prost_types::Timestamp`'s Option field maps `None` to the protobuf default (absent field), not `{seconds: 0, nanos: 0}`. Use `.ok()` to get `Option<Timestamp>` and rely on proto3 default for None.
**How to avoid:** Use `.parse::<prost_types::Timestamp>().ok()` which produces `None` on failure. The proto field is `Option<Timestamp>` — None is cleaner than a zero epoch.
**Warning signs:** Test memories show created_at as "1970-01-01T00:00:00Z".

## Code Examples

Verified patterns from official sources and existing codebase:

### Full StoreMemory handler skeleton
```rust
// Source: CONTEXT.md D-04/D-05/D-06/D-07 + src/server.rs create_memory_handler pattern
async fn store_memory(
    &self,
    request: tonic::Request<proto::StoreMemoryRequest>,
) -> Result<tonic::Response<proto::StoreMemoryResponse>, tonic::Status> {
    let auth_ctx = request.extensions().get::<crate::auth::AuthContext>().cloned();
    let body = request.into_inner();

    // D-16: reject empty content
    if body.content.trim().is_empty() {
        return Err(tonic::Status::invalid_argument("content must not be empty"));
    }

    // D-07: enforce scope before delegating
    let effective_agent_id = enforce_scope(auth_ctx.as_ref(), Some(body.agent_id.as_str()).filter(|s| !s.is_empty()))
        .map_err(api_error_to_status)?;

    let memory = self.memory_service.create_memory(crate::service::CreateMemoryRequest {
        content: body.content,
        agent_id: effective_agent_id.or_else(|| {
            if body.agent_id.is_empty() { None } else { Some(body.agent_id) }
        }),
        session_id: if body.session_id.is_empty() { None } else { Some(body.session_id) },
        tags: if body.tags.is_empty() { None } else { Some(body.tags) },
    }).await.map_err(api_error_to_status)?;

    Ok(tonic::Response::new(proto::StoreMemoryResponse {
        memory: Some(memory_to_proto(memory)),
    }))
}
```

### tonic-reflection registration in serve_grpc()
```rust
// Source: hyperium/tonic v0.13.0 examples/src/reflection/server.rs
// Must be placed AFTER health_service creation, BEFORE Server::builder()
let reflection_service = tonic_reflection::server::Builder::configure()
    .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
    .build_v1()
    .expect("failed to build gRPC reflection service");
```

### prost_types::Timestamp from stored created_at string
```rust
// Source: docs.rs/prost-types/0.13.0 — Timestamp implements FromStr for RFC 3339
// DB stores "YYYY-MM-DDTHH:MM:SS.fffZ" — valid RFC 3339
let created_at: Option<prost_types::Timestamp> = m.created_at.parse().ok();
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| tonic-reflection `build()` (v1alpha only) | `build_v1()` for v1, `build_v1alpha()` for backward compat | tonic-reflection 0.12 | Use `build_v1()` — grpcurl 1.x uses reflection v1 |
| `tonic_build::compile_protos()` without descriptor | `tonic_build::configure().file_descriptor_set_path().compile_protos()` | tonic-build 0.9+ | Required to enable reflection; shorthand doesn't support it |
| `tonic::include_file_descriptor_set!` unavailable | `tonic::include_file_descriptor_set!("stem")` macro available | tonic 0.9+ | Handles OUT_DIR env var resolution; use instead of raw include_bytes! |

**Deprecated/outdated:**
- `build_v1alpha()` for new implementations: Use `build_v1()`. v1alpha is legacy compatibility only.
- Sync interceptors for auth: Not applicable here (Phase 27 already uses Tower Layer correctly).

## Open Questions

1. **Reflection bypass for unauthenticated grpcurl**
   - What we know: GrpcAuthLayer bypasses `/grpc.health.v1.Health/` but not `/grpc.reflection.v1.ServerReflection/`
   - What's unclear: Success criterion 5 says `grpcurl list` returns results, but doesn't specify whether auth must be active during the test
   - Recommendation: Implement without bypass (consistent with auth-first design). Document in test that `grpcurl list` requires a token when auth is active. Add bypass as a separate decision if needed.

2. **Handler placement: grpc/mod.rs vs grpc/handlers.rs**
   - What we know: CONTEXT.md leaves this to Claude's discretion; four handlers + two helpers (~150-200 lines) is manageable in mod.rs
   - What's unclear: Whether future phases (GRPC-EXT-01 is out of scope but STREAM-01 exists) will expand grpc/ enough to warrant a submodule now
   - Recommendation: Keep in grpc/mod.rs for Phase 28. The file will be ~250 lines after Phase 28 — still readable. Defer submodule split to when streaming is added.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework (`cargo test`) |
| Config file | None — standard `#[cfg(test)]` modules + `tests/` directory |
| Quick run command | `cargo test --features interface-grpc grpc` |
| Full suite command | `cargo test --features interface-grpc` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| GRPC-01 | StoreMemory returns stored memory with assigned ID | integration | `cargo test --features interface-grpc test_grpc_store_memory` | No — Wave 0 |
| GRPC-02 | SearchMemories returns ranked results matching REST search | integration | `cargo test --features interface-grpc test_grpc_search_memories` | No — Wave 0 |
| GRPC-03 | ListMemories returns memory list with correct filter | integration | `cargo test --features interface-grpc test_grpc_list_memories` | No — Wave 0 |
| GRPC-04 | DeleteMemory non-existent ID returns NotFound | integration | `cargo test --features interface-grpc test_grpc_delete_not_found` | No — Wave 0 |
| GRPC-05 (scope) | Scoped key with wrong agent_id returns PermissionDenied | integration | `cargo test --features interface-grpc test_grpc_scope_enforcement` | No — Wave 0 |
| GRPC-05 (validation) | Empty content / empty id returns InvalidArgument | unit | `cargo test --features interface-grpc test_grpc_invalid_argument` | No — Wave 0 |
| HEALTH-01 | health_reporter returns SERVING | unit/smoke | `cargo test --features interface-grpc test_grpc_health_serving` | No — Wave 0 |
| HEALTH-02 | Reflection builds without error | unit/smoke | `cargo test --features interface-grpc test_grpc_reflection_builds` | No — Wave 0 |

**Critical tests (per STATE.md critical research flag):**
Every handler needs a dedicated scope enforcement test. These are not type-enforced and must be verified per-handler. Suggested test names:
- `test_grpc_store_memory_scope_enforcement`
- `test_grpc_search_memories_scope_enforcement`
- `test_grpc_list_memories_scope_enforcement`
- `test_grpc_delete_memory_scope_enforcement`

### Sampling Rate
- **Per task commit:** `cargo test --features interface-grpc 2>&1 | tail -5`
- **Per wave merge:** `cargo test --features interface-grpc`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/grpc_integration.rs` — covers GRPC-01 through GRPC-05, HEALTH-01, HEALTH-02 with in-process MnemonicGrpcService (no real network needed — call trait methods directly with tonic::Request::new())
- [ ] Per-handler scope enforcement tests (4 tests) — covers STATE.md critical research flag
- [ ] `build.rs` update — add `file_descriptor_set_path` before any reflection code compiles

## Sources

### Primary (HIGH confidence)
- `src/grpc/mod.rs` — Phase 27 skeleton; MnemonicGrpcService struct fields, serve_grpc() wiring, stub impls
- `src/grpc/auth.rs` — AuthContext injection into extensions; health bypass pattern
- `src/server.rs:78-95` — enforce_scope() function to duplicate/move; exact signature
- `src/service.rs` — MemoryService methods, Memory struct, SearchResultItem.distance type
- `src/error.rs` — ApiError variants (BadRequest, NotFound, Forbidden, Unauthorized, Internal)
- `proto/mnemonic.proto` — All 4 RPC definitions, Memory message field numbers and types
- `Cargo.toml` — All tonic/prost deps already at 0.13 under interface-grpc feature
- [docs.rs/prost-types/0.13.0 Timestamp](https://docs.rs/prost-types/latest/prost_types/struct.Timestamp.html) — FromStr for RFC 3339 confirmed
- [tonic-health 0.13 health_reporter](https://docs.rs/tonic-health/0.13.0/tonic_health/server/fn.health_reporter.html) — Returns `(HealthReporter, HealthServer<impl Health>)`; already wired in serve_grpc()

### Secondary (MEDIUM confidence)
- [tonic-reflection server module 0.13.0](https://docs.rs/tonic-reflection/0.13.0/tonic_reflection/server/index.html) — Builder struct confirmed; build_v1() method confirmed
- [hyperium/tonic v0.13.0 examples/src/reflection/server.rs](https://github.com/hyperium/tonic/blob/v0.13.0/examples/src/reflection/server.rs) — `include_file_descriptor_set!` macro + `build_v1()` + `register_encoded_file_descriptor_set()` usage verified
- [tonic-build 0.13.0 Builder.file_descriptor_set_path](https://docs.rs/tonic-build/0.13.0/tonic_build/struct.Builder.html) — `file_descriptor_set_path(path: impl AsRef<Path>) -> Self` confirmed
- [hyperium/tonic examples/build.rs v0.13.0](https://github.com/hyperium/tonic/blob/v0.13.0/examples/build.rs) — `out_dir.join("helloworld_descriptor.bin")` pattern confirmed

### Tertiary (LOW confidence)
- None — all claims verified against official sources.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crates already in Cargo.toml at pinned version; no new dependencies
- Architecture: HIGH — patterns are direct mirrors of REST handlers in server.rs; tonic-reflection API verified from official examples
- Pitfalls: HIGH — sourced from official issue tracker (build shorthand), existing code comments (auth bypass), and type system analysis (f64/f32 cast)
- Timestamp conversion: HIGH — prost_types::Timestamp FromStr implementation confirmed in official docs

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (tonic 0.13 is stable; prost 0.13 pinned by qdrant-client anchor; low churn risk)
