# Pitfalls Research

**Domain:** Adding gRPC (tonic) to existing axum REST server — Mnemonic v1.5
**Researched:** 2026-03-22
**Confidence:** HIGH (tonic/axum internals verified against official docs and GitHub issues), MEDIUM (binary size, CI cross-platform specifics)

---

## Critical Pitfalls

### Pitfall 1: Tonic and Axum Body Type Mismatch on Same Port

**What goes wrong:**
When multiplexing gRPC and REST on the same port, axum produces `Response<Body>` while tonic (through `GrpcWebLayer` or custom multiplexers) produces `Response<UnsyncBoxBody<Bytes, Status>>`. Routing between them fails to compile with a wall of trait errors. The pre-axum-0.7 pattern using `Router::into_router()` was deprecated, and the modern `Routes::new().into_axum_router()` combined with `GrpcWebLayer` has an open incompatibility (GitHub issue #1964, still unresolved as of September 2025).

**Why it happens:**
tonic and axum both upgraded to hyper 1.x but still use different body wrapper types internally. Developers copy-paste examples from older blog posts that predate the hyper 1.0 upgrade and hit type errors. The axum multiplexing PR #2825 resolved part of the issue by using `tower::steer` and `tonic::Routes::into_axum_router()`, but these methods still fall short when `GrpcWebLayer` is involved.

**How to avoid:**
Run tonic on a **separate port** entirely — the v1.5 spec already mandates this ("gRPC server on a separate port alongside REST"). Bind tonic's own `TcpListener` via `tonic::transport::Server::builder()` and run it concurrently with the existing axum server using `tokio::join!`. Separate ports sidestep all body-type unification problems completely and no custom multiplex service is needed.

**Warning signs:**
- Error messages referencing `UnsyncBoxBody`, `BoxBody`, or `impl Service<axum::http::Request<Body>>` is not satisfied
- Blog posts or examples that reference `multiplex_service.rs` — those target a pre-0.12 tonic API
- Dependency on the `axum-tonic` crate being added to `Cargo.toml`

**Phase to address:**
Phase 1 (server skeleton and dual-listener setup). Decide on separate ports before writing any handler code. The main() startup sequence should wire both listeners in `tokio::join!` from the beginning.

---

### Pitfall 2: protoc System Dependency Breaks CI

**What goes wrong:**
`tonic-build` (via `prost-build` v0.11+) requires a system-installed `protoc` binary. The build appears to succeed locally but then fails in CI with a cryptic `No such file or directory` error pointing into `OUT_DIR` when the proto-generated `.rs` file is included. In GitHub Actions, `which protoc` may succeed yet `cargo build` still fails because the PATH inherited by the build script environment is different.

The existing release workflow (`release.yml`) has zero protoc installation steps — it only installs the Rust toolchain and calls `cargo build --release`. Adding `tonic-build` to `[build-dependencies]` without updating the workflow is silently broken until the first CI run.

**Why it happens:**
`prost-build` changed in v0.11 to require `protoc` rather than bundling it. When `protoc` is absent, the build script does not fail immediately — it exits silently and leaves no generated `.rs` file. The failure surfaces later as `include!(concat!(env!("OUT_DIR"), "/mnemonic.rs"))` producing "No such file or directory."

**How to avoid:**
Update the release workflow in the same phase that adds `build.rs`. Add these steps before `cargo build`:
- **Ubuntu:** `sudo apt-get install -y protobuf-compiler`
- **macOS:** `brew install protobuf`
- Explicitly set `PROTOC=$(which protoc)` as an env var in the workflow step

Alternative: add the `protoc-bin-vendored` crate as a `[build-dependencies]` entry and call `prost_build::Config::new().protoc_executable(protoc_bin_vendored::protoc_bin_path().unwrap())` — this provides a pre-built protoc binary, removes the system dependency entirely, and is the lower-risk path for a project that already cross-compiles.

**Warning signs:**
- Build succeeds locally (where `brew install protobuf` was done at some point) but fails in CI
- CI error: `could not find `protoc`` or `No such file or directory` in `target/debug/build/.../out/mnemonic.rs`
- `cargo build` exits 101 during the build script phase, not the compile phase

**Phase to address:**
Phase 1 (build.rs and tonic-build setup). Update `release.yml` in the same commit that adds `build.rs`. Do not merge a `build.rs` without a corresponding workflow update.

---

### Pitfall 3: build.rs Causes Always-Dirty Incremental Builds

**What goes wrong:**
`tonic_build::compile_protos("proto/mnemonic.proto")` internally emits `cargo:rerun-if-changed=mnemonic.proto` (the literal filename argument) rather than the resolved path `proto/mnemonic.proto`. Cargo checks whether a file named `mnemonic.proto` exists in the workspace root — it doesn't — so it considers the build perpetually dirty. Every `cargo build` re-runs the entire build script and recompiles all generated code, adding 5-15 seconds to every build.

**Why it happens:**
Known tonic-build bug (issue #2239): the `rerun-if-changed` directive emits the literal path argument rather than the fully resolved path. This is especially triggered when proto files live in a subdirectory (`proto/`) but only the filename or a relative sub-path is passed to `compile_protos`.

**How to avoid:**
Always add an explicit `println!` directive using the full relative path, and pass the full relative path to `compile_protos`. The explicit directive overrides tonic-build's incorrect implicit one:

```rust
// build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/mnemonic.proto");
    tonic_build::compile_protos("proto/mnemonic.proto")?;
    Ok(())
}
```

Verify by running `cargo build && cargo build` — the second run must complete in under one second with no build script output.

**Warning signs:**
- `cargo build` takes >10 seconds on the second consecutive run with no source changes
- `cargo build -v` shows `Running build script` on every invocation
- Build output mentions "the file `mnemonic.proto` is missing" as a dirty trigger

**Phase to address:**
Phase 1 (build.rs setup). Verify incremental behavior before declaring the phase complete — this is a pass/fail criterion, not a nice-to-have.

---

### Pitfall 4: prost/tonic Version Conflict with qdrant-client

**What goes wrong:**
mnemonic already depends on `qdrant-client = "1"` (optional, `backend-qdrant` feature). `qdrant-client 1.16+` depends on `tonic ^0.12.3` and `prost ^0.13.3`. When you add your own `tonic` and `prost` to `[dependencies]` for the gRPC server, Cargo must unify these versions. If you pin to an incompatible version (e.g., `tonic = "0.11"`) the dependency resolver fails. Conversely, if you accidentally pick `tonic = "0.13"` (which does not exist as of early 2026), you get an error. The resolved versions must satisfy both constraints simultaneously.

**Why it happens:**
Both qdrant-client and your new gRPC server need tonic and prost. Writing `tonic = "0.12"` satisfies `^0.12.3` cleanly, but writing `tonic = "0.11"` does not. This is an easy mistake when copying from tutorials that predate qdrant-client's 1.x release.

**How to avoid:**
Match what qdrant-client requires. As of early 2026:
- `tonic = "0.12"` in `[dependencies]`
- `prost = "0.13"` in `[dependencies]`
- `tonic-build = "0.12"` in `[build-dependencies]`

Run `cargo tree -d` immediately after adding these dependencies to check for duplicate versions. Zero duplicates of tonic or prost is the success criterion.

**Warning signs:**
- `error: failed to select a version for 'tonic'` during `cargo build`
- `cargo tree -d` shows two copies of `tonic` or `prost` at different versions
- Compilation errors referencing `prost::Message` trait not being satisfied at a crate boundary

**Phase to address:**
Phase 1 (Cargo.toml changes). Run `cargo tree -d` immediately after adding tonic/prost and before writing any code.

---

### Pitfall 5: Tonic Interceptor Cannot Do Async Auth — Tower Layer Required

**What goes wrong:**
The natural first instinct for porting the existing `auth_middleware` to gRPC is to use `tonic::service::interceptor()`. This looks like the gRPC equivalent of an axum middleware. However, tonic interceptors are **sync-only** and see only `MetadataMap` — they cannot perform async operations, cannot call `KeyService::count_active_keys().await`, and cannot inject extensions that handlers later extract via `req.extensions()`.

The existing auth pattern requires:
1. Async SQLite query (`count_active_keys().await`) to determine open-mode vs. auth-active
2. Async SQLite query (`validate().await`) to hash and compare the bearer token
3. Injecting `AuthContext` into request extensions for handler-level scope enforcement

None of these work inside a sync tonic interceptor closure.

**Why it happens:**
tonic interceptors are intentionally restricted: they strip the body before calling the interceptor function so it only receives metadata. This is documented behavior. Developers familiar with axum's `middleware::from_fn_with_state()` assume interceptors are the equivalent — they are not.

**How to avoid:**
Use a Tower `Layer` applied to the tonic `Server::builder()`, not `with_interceptor()`:

```rust
tonic::transport::Server::builder()
    .layer(
        ServiceBuilder::new()
            .layer(GrpcAuthLayer::new(Arc::clone(&key_service)))
    )
    .add_service(MnemonicServiceServer::new(grpc_impl))
    .serve(grpc_addr)
    .await?;
```

`GrpcAuthLayer` wraps a `tower::Service` implementation that is async, has access to the full request, can call `KeyService` async methods, and injects `AuthContext` into `request.extensions_mut()`. The `tonic-middleware` crate provides a simpler async interceptor API as an alternative if full Tower layer boilerplate is undesirable.

**Warning signs:**
- Using `with_interceptor()` and adding `.await` inside the closure — the compiler rejects it with a confusing future/Send error
- Auth that only checks for header presence (not validates it via async DB call) — a sync interceptor can do that, but the full validation logic cannot
- The gRPC server accepts any bearer token without failing in tests

**Phase to address:**
Phase 2 (gRPC auth layer). Write the Tower auth layer before implementing any protected handler. Never ship a gRPC endpoint without auth for a server that has REST auth enabled.

---

### Pitfall 6: gRPC Status Codes Are Not HTTP Status Codes

**What goes wrong:**
The existing `ApiError` enum maps to HTTP status codes (401, 403, 400, 404, 500) and implements `IntoResponse` for axum. When porting auth and handler logic to gRPC, trying to return `ApiError` directly from a tonic handler causes a type mismatch. If forced by wrapping, the gRPC client receives `Code::Unknown` or `Code::Internal` for every error because HTTP codes do not map directly to gRPC codes.

Correct gRPC status code mapping for this project:
- HTTP 401 → `tonic::Status::unauthenticated("...")`
- HTTP 403 → `tonic::Status::permission_denied("...")`
- HTTP 400 → `tonic::Status::invalid_argument("...")`
- HTTP 404 → `tonic::Status::not_found("...")`
- HTTP 500 → `tonic::Status::internal("...")`

**Why it happens:**
`ApiError` is axum-specific (implements `axum::response::IntoResponse`). There is no automatic conversion to `tonic::Status`. Developers try to reuse the existing error type across both protocols without a conversion layer.

**How to avoid:**
Create a `fn api_error_to_grpc_status(e: ApiError) -> tonic::Status` conversion function in a shared module (e.g., `src/grpc/error.rs`). Every tonic handler result type must be `Result<tonic::Response<T>, tonic::Status>`. Do not add `impl From<ApiError> for tonic::Status` as a blanket — make the conversion explicit at each call site so every mapping gets reviewed.

Note: `tonic::Status::from_error()` is explicitly documented as having unstable downcast behavior — do not use it for mapping known error types.

**Warning signs:**
- Tonic handler functions with return type `Result<..., ApiError>` instead of `Result<..., tonic::Status>`
- gRPC clients receiving `Code::Unknown` for auth failures
- Error messages appearing as empty strings in gRPC responses

**Phase to address:**
Phase 2 (gRPC handler implementations). Define the `api_error_to_grpc_status` function before writing any handler body.

---

### Pitfall 7: Proto Field Optionality Mismatch — Scalar vs. Message Types

**What goes wrong:**
In proto3, scalar fields (`string`, `int32`, `bool`) are **not** `Option<T>` in prost-generated Rust code by default — they use their zero value (`""`, `0`, `false`) when absent from the wire. Message fields are `Option<T>`. This creates asymmetry:

- `string agent_id = 1;` missing from the wire becomes `""`  not `None`
- A nested message field missing from the wire becomes `None`

For mnemonic, `agent_id` being `""` looks like "no agent filter" to the client but the existing service layer receives it as an empty string. The current service behavior with `agent_id = ""` is inconsistent across search, list, and store operations. An agent that sends no `agent_id` in a gRPC request could inadvertently access the global default namespace or hit a validation error depending on how the service interprets `""`.

**Why it happens:**
proto3 removed field presence tracking for scalars to simplify the wire format. Developers coming from REST (where missing field = null) do not realize that `""` and "not present" are indistinguishable in proto3 scalars unless the `optional` keyword is explicitly used.

**How to avoid:**
Use the `optional` keyword on every scalar field where absence vs. empty string has semantic significance:

```protobuf
syntax = "proto3";

message StoreRequest {
  string content = 1;                  // Required — always present
  optional string agent_id = 2;        // Option<String> in Rust
  optional string session_id = 3;      // Option<String> in Rust
  repeated string tags = 4;            // Vec<String>, empty = not provided
}
```

`optional` on a proto3 scalar generates `pub agent_id: Option<String>` in prost-generated Rust. Review every field before finalizing the `.proto` file — changing field optionality after clients exist is a wire-breaking change.

**Warning signs:**
- prost-generated struct shows `pub agent_id: String` instead of `pub agent_id: Option<String>`
- Service layer receiving `agent_id: ""` when the gRPC client sent no agent_id
- Tests that pass `agent_id: ""` and observe unexpected all-namespace behavior

**Phase to address:**
Phase 1 (proto file design). Finalize all field optionality in the `.proto` file before implementing any handlers. This is a breaking change if modified after clients exist.

---

### Pitfall 8: Missing enforce_scope() in gRPC Handlers

**What goes wrong:**
The REST handlers all call `enforce_scope(auth_ctx, body.agent_id.as_deref())` at the top of each handler to enforce key-scoped namespace isolation. This logic is easy to forget when writing gRPC handlers. A gRPC `Store` call from an agent using a key scoped to `agent-A` that supplies `agent_id: "agent-B"` in the request should be rejected with `PERMISSION_DENIED`. Without the scope check, it silently stores the memory under the wrong agent.

**Why it happens:**
gRPC handlers are written separately from REST handlers and the enforcement pattern is not part of the service layer — it lives in `server.rs` as a function called by each axum handler. When writing new tonic handler implementations, this call site is easy to miss because it is not enforced by the type system.

**How to avoid:**
Move `enforce_scope()` into a shared module that is required to be called by both REST and gRPC handlers. Or, add it to the service layer so that `MemoryService::store()` takes an `Option<&AuthContext>` and enforces scope internally — this makes the enforcement impossible to forget because it is inside the business logic, not the transport layer.

For v1.5, at minimum: add a test for each gRPC handler that verifies a scoped key cannot access a different agent's data.

**Warning signs:**
- gRPC handlers that call `state.service.create_memory(body).await` directly without checking scope
- No test that tries a scoped key with a mismatched agent_id via gRPC and asserts `PERMISSION_DENIED`
- REST and gRPC allow different agent access patterns under the same API key

**Phase to address:**
Phase 2 (gRPC handler implementations). Add scope enforcement test before declaring any handler complete.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Skip gRPC auth entirely for v1.5 | Faster to ship | Unauthenticated gRPC port is a security hole if REST auth is active; auth retrofit is harder than auth first | Never |
| Copy axum handler logic into tonic handlers | Fast implementation | Duplicated validation at two call sites; bug fixed in REST not fixed in gRPC | Never — share the service layer via `Arc<MemoryService>` |
| Same-port multiplexing via content-type routing | One port is simpler | Body-type incompatibility requires a custom Tower service that fights axum/tonic version drift | Only if separate ports are a hard external constraint |
| Commit generated proto `.rs` files to git | Avoids build.rs dependency | Generated code in git causes merge conflicts; build state in source is an anti-pattern | Acceptable during initial scaffolding only; remove before v1.5 release |
| Use `tonic::service::interceptor` for auth | Less boilerplate | Cannot do async key validation; open-mode check impossible in sync context | Never for this project's auth model |
| Default tonic features (includes TLS via ring/aws-lc) | Works out of the box | Pulls in large TLS crates that increase binary size and cross-compilation friction | Use `default-features = false, features = ["transport", "codegen"]` if TLS is not required for v1.5 |
| Hardcode gRPC port as REST_PORT + 1 | Simplifies initial implementation | Port conflict if user runs two instances; not configurable | Never — add `grpc_port` to Config from the start |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| tonic + axum AppState | Passing `AppState` directly to the tonic service struct (wrong type) | Clone `Arc<MemoryService>`, `Arc<CompactionService>`, `Arc<KeyService>` into both the axum `AppState` and the tonic service struct independently |
| tonic + KeyService auth | Calling async `KeyService` methods inside a sync tonic interceptor | Implement auth as a Tower `Layer` applied to `Server::builder()`; interceptors are sync-only |
| tonic + ApiError | Returning `ApiError` from a tonic handler | Create `api_error_to_grpc_status(ApiError) -> tonic::Status` conversion; never use `ApiError` in a tonic context |
| tonic-build + release.yml | No protoc installation step in CI workflow | Add `brew install protobuf` (macOS) and `apt-get install protobuf-compiler` (Ubuntu) before `cargo build` in the workflow |
| tonic + qdrant-client | Version conflict on prost/tonic | Pin `tonic = "0.12"` and `prost = "0.13"` to match qdrant-client's constraints; verify with `cargo tree -d` |
| gRPC port config | Hardcoding gRPC port | Add `grpc_port: u16` to `Config` struct with a documented default (e.g., 50051); expose via env var `GRPC_PORT` and TOML config |
| tonic TLS features | Enabling default TLS features that pull in `ring` | Use `tonic = { version = "0.12", default-features = false, features = ["transport", "codegen"] }` for v1.5 if TLS is not in scope |
| gRPC metadata key casing | Using `"Authorization"` (capitalized) as metadata key | gRPC metadata keys are lowercase by convention and by HTTP/2 spec; use `"authorization"` — `MetadataKey::from_static("authorization")` |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Per-request SQLite `count_active_keys()` on gRPC hot path | Auth adds 1 SQLite query per gRPC request; slower than REST equivalent under high concurrency | Intentional per D-04 decision; acceptable for v1.5 scale; document as a known single-file bottleneck | >10k req/s to a single SQLite file |
| Embedding model mutex under concurrent gRPC `Store` calls | gRPC store latency spikes due to `Arc<Mutex<LocalEngineInner>>` contention | Same as REST — the existing `EmbeddingEngine` is already the bottleneck; document; no fix needed for v1.5 | >20 concurrent gRPC store calls |
| Blocking inside a tonic async handler | Handler hangs; tokio runtime thread starved | Never call `std::thread::sleep`, `block_on()`, or synchronous I/O inside tonic handlers; all existing DB/embedding paths are already async | Any blocking call |
| Default tonic max message size (4MB) exceeded | Silent truncation or gRPC error on large payloads | Set `.max_decoding_message_size()` on `Server::builder()` | Memory content batch exceeding 4MB; unlikely for v1.5 use cases |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| gRPC port open without auth while REST requires auth | Agent bypasses auth by using gRPC port | gRPC auth Tower layer must be implemented in the same phase as gRPC handlers — no "auth later" deferred phase |
| Logging raw bearer token from gRPC metadata | Token exposed in structured logs (same risk as REST — see comment in `create_key_handler`) | Extract and validate token; never `tracing::debug!` or `tracing::info!` the raw bearer value in the auth layer |
| gRPC port bound to `0.0.0.0` without TLS | Plaintext traffic visible on network | Document in v1.5 that TLS is optional; emit a startup warning if gRPC binds to a non-loopback address without TLS configured |
| Missing `enforce_scope()` in gRPC handlers | Cross-agent memory access via gRPC even with valid API key | Call `enforce_scope()` at the top of every gRPC handler that accepts `agent_id`; add a test for each handler to verify scope rejection |
| MetadataKey case sensitivity | `Authorization` vs `authorization` — HTTP/2 requires lowercase; tonic's MetadataKey enforces this at runtime with a panic on invalid keys | Always use lowercase metadata key strings; use `MetadataKey::from_static("authorization")` not a string literal with mixed case |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| gRPC port not discoverable | Agents cannot discover the gRPC port programmatically after startup | Add `"grpc_port": <n>` to the `GET /health` response body when gRPC is enabled |
| gRPC server starts but emits no startup log | No confirmation that gRPC is accepting connections | Log `gRPC server listening on 0.0.0.0:<port>` at startup, matching the existing REST `server listening` log |
| No reflection service | Agent developers must distribute the `.proto` file to use `grpcurl` | Add `tonic-reflection` with the compiled file descriptor set; enables `grpcurl ls` to enumerate services without distributing protos |
| gRPC errors with empty message strings | Agent receives `Code::InvalidArgument` with no details | Every `tonic::Status` must include a human-readable message string consistent with the REST error message pattern |
| Proto breaking change without service version | Existing agent gRPC clients silently receive wrong data after server upgrade | Treat `.proto` changes as a versioned API contract; field removals and type changes require a new service version |

---

## "Looks Done But Isn't" Checklist

- [ ] **Incremental builds work:** Run `cargo build && cargo build` — the second run must complete in under 2 seconds with no `Running build script` output. Failure means the `rerun-if-changed` path is wrong.
- [ ] **CI protoc installed:** The release workflow YAML has explicit protoc installation steps for both Ubuntu and macOS runners before `cargo build --release`.
- [ ] **Version conflict free:** `cargo tree -d` shows zero duplicate `tonic` or `prost` entries after adding tonic to Cargo.toml.
- [ ] **gRPC auth open-mode works:** Integration test calls a gRPC method with no API keys created and no `authorization` header; request succeeds.
- [ ] **gRPC auth enforces tokens:** Integration test sends a bad bearer token; receives `Code::Unauthenticated`, not `Code::Unknown` or `Code::Internal`.
- [ ] **Scope enforcement on gRPC:** Test that a scoped API key calling gRPC with wrong `agent_id` receives `Code::PermissionDenied`.
- [ ] **Proto field optionality correct:** Every `agent_id`, `session_id`, `threshold`, `limit` field in the `.proto` uses `optional` where absence is meaningful; inspect the prost-generated structs to verify `Option<T>`.
- [ ] **Status codes correct:** HTTP 401 → `Unauthenticated`, HTTP 403 → `PermissionDenied`, HTTP 404 → `NotFound`, HTTP 400 → `InvalidArgument` — not all collapsed to `Internal`.
- [ ] **gRPC port in config:** `grpc_port` field exists in `Config`, has a documented default, appears in `mnemonic config show` output.
- [ ] **Health endpoint updated:** `GET /health` JSON includes `grpc_port` when the server starts with gRPC enabled.
- [ ] **No raw token in logs:** Grep the auth layer implementation for `tracing` calls and confirm no bearer token value is logged at any level.
- [ ] **Separate port confirmed:** Both REST and gRPC listeners start on different ports; no content-type multiplexing code exists.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Same-port body type mismatch discovered mid-implementation | MEDIUM | Switch to separate-port architecture; remove multiplex service; axum and tonic bind independently via `tokio::join!` |
| CI protoc missing on first CI run | LOW | Add protoc install step to release.yml; re-run CI |
| Always-dirty incremental builds | LOW | Add `println!("cargo:rerun-if-changed=proto/mnemonic.proto")` with the full path to `build.rs`; verify timing |
| prost/tonic version conflict | LOW-MEDIUM | Run `cargo tree -d`; update pins to match qdrant-client constraints; may require updating other transitive dependencies |
| sync interceptor chosen for auth instead of Tower layer | MEDIUM | Replace `with_interceptor()` with `Server::builder().layer()`; rewrite auth as a Tower service; existing `KeyService` interface is unchanged |
| Proto field type wrong after first release | HIGH | Requires a new `.proto` service version (`MnemonicServiceV2`); all existing clients must be updated simultaneously; proto design must be reviewed and locked before first release |
| Missing scope enforcement discovered after release | HIGH | Add `enforce_scope()` calls to all affected gRPC handlers; release a patch; audit logs for cross-agent accesses |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Body type mismatch (same-port) | Phase 1: Dual-server skeleton | Two listeners bind on different ports; axum and tonic start concurrently in `tokio::join!` |
| CI protoc missing | Phase 1: build.rs + Cargo.toml changes | Release workflow runs successfully in CI with protoc install steps present |
| Always-dirty build | Phase 1: build.rs setup | `cargo build && cargo build` — second run shows no build script output, completes under 2s |
| prost/tonic version conflict with qdrant-client | Phase 1: Cargo.toml | `cargo tree -d` shows zero duplicate tonic or prost entries |
| Sync interceptor for async auth | Phase 2: gRPC auth Tower layer | Auth layer compiles; integration test validates open-mode passthrough and token validation via async `KeyService` |
| gRPC status code mapping | Phase 2: gRPC handlers | Test that invalid token returns `Code::Unauthenticated`, bad agent_id returns `Code::PermissionDenied`, not `Code::Internal` |
| Proto field optionality | Phase 1: proto design | Every `agent_id`, `session_id` field uses `optional`; prost-generated structs show `Option<String>` |
| Missing enforce_scope in gRPC handlers | Phase 2: gRPC handlers | Test: scoped key + wrong agent_id → `Code::PermissionDenied` for every gRPC method |
| Logging raw token | Phase 2: gRPC auth layer | Code review grep for `tracing` calls in auth layer; confirm no token value is logged |
| gRPC port not in health | Phase 3: integration | `GET /health` response JSON contains `grpc_port` when server starts with gRPC enabled |

---

## Sources

- tonic GitHub issue [#1964](https://github.com/hyperium/tonic/issues/1964) — `tonic-web` + axum body type mismatch, open as of September 2025
- axum pull request [#2825](https://github.com/tokio-rs/axum/pull/2825) — gRPC multiplex example fix using `tower::steer` and `tonic::Routes::into_axum_router()`
- tonic GitHub issue [#2239](https://github.com/hyperium/tonic/issues/2239) — always-dirty builds from incorrect `rerun-if-changed` path emission
- [tonic-build docs](https://docs.rs/tonic-build/latest/tonic_build/) — OUT_DIR behavior, protoc system dependency, build script configuration
- [prost-build docs](https://docs.rs/prost-build/latest/prost_build/) — protoc requirement since v0.11; `PROTOC_NO_VENDOR` env var
- [qdrant-client 1.16+ Cargo.toml](https://docs.rs/crate/qdrant-client/latest) — `tonic ^0.12.3`, `prost ^0.13.3` constraints
- [tonic Interceptor docs](https://docs.rs/tonic/latest/tonic/service/interceptor/index.html) — sync-only, metadata-only design
- [Announcing Tonic 0.5](https://tokio.rs/blog/2021-07-tonic-0-5) — Tower-based interceptor API; interceptors are sync, Tower layers are async
- [tonic-middleware crate](https://crates.io/crates/tonic-middleware) — async interceptor alternative
- prost issue [#520](https://github.com/tokio-rs/prost/issues/520) — proto3 scalar fields not `Option<T>` by default; `optional` keyword required
- [GitHub Actions protoc Discussion #160036](https://github.com/orgs/community/discussions/160036) — protoc not found in CI despite installation
- [tonic::Status docs](https://docs.rs/tonic/latest/tonic/struct.Status.html) — `from_error()` instability note; correct code constructors
- Direct codebase inspection: `src/auth.rs`, `src/server.rs`, `src/storage/mod.rs`, `.github/workflows/release.yml`, `Cargo.toml`

---
*Pitfalls research for: Adding gRPC (tonic) to existing axum/Rust server — Mnemonic v1.5*
*Researched: 2026-03-22*
