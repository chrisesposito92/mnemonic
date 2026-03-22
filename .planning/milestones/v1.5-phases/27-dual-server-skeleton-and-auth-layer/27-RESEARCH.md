# Phase 27: Dual-Server Skeleton and Auth Layer - Research

**Researched:** 2026-03-22
**Domain:** Rust / tonic 0.13 / Tower async middleware / tokio dual-server
**Confidence:** HIGH

## Summary

Phase 27 wires REST and gRPC onto separate TCP ports using `tokio::try_join!`, implements a hand-rolled async Tower Layer for gRPC auth (because no compatible async-interceptor crate exists for tonic 0.13), and adds `grpc_port` to Config. The task is almost entirely integration/extension — no new crates required, no greenfield service design. Every sub-task has an exact template in the existing codebase.

The Tower Layer receives `http::Request<tonic::body::Body>` (verified from tonic 0.13.1 source), not `tonic::Request<T>`. This is the primary API hazard. Auth logic is extracted directly from the already-working `src/auth.rs` middleware — `KeyService.count_active_keys()`, `KeyService.validate()`, and `AuthContext` are reused without modification.

`tonic-async-interceptor 0.13.x` does not exist in the registry (latest is 0.14.x, which requires tonic 0.14). `tonic-middleware 0.4.1` requires tonic 0.14. Both are out of scope — the hand-rolled Tower Layer is the correct and only path.

**Primary recommendation:** Hand-roll a Tower `Layer + Service` pair in `src/grpc/auth.rs`. Use `Pin<Box<dyn Future>>` return type. Extract bearer from `req.headers().get("authorization")` (not `req.metadata()` — that is only on `tonic::Request`, not `http::Request`).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-01:** New `src/grpc/` directory with `mod.rs` as entry point, gated behind `#[cfg(feature = "interface-grpc")]` in main.rs
**D-02:** `src/grpc/mod.rs` contains the tonic service struct, `include_proto!` for generated types, and the `serve_grpc()` function
**D-03:** Phase 28 will add `src/grpc/handlers.rs` for RPC implementations — Phase 27 only needs the skeleton with unimplemented handlers that return `Status::Unimplemented`
**D-04:** `tokio::try_join!` across two independent `TcpListener` binds — NOT same-port multiplexing
**D-05:** REST server starts unconditionally on existing `config.port` (default 8080). gRPC server starts alongside it on `config.grpc_port` (default 50051)
**D-06:** If either server fails to bind, both shut down (try_join! semantics — first error propagates)
**D-07:** Startup log prints both addresses: `"REST listening on 0.0.0.0:{port}, gRPC listening on 0.0.0.0:{grpc_port}"`
**D-08:** New field `grpc_port: u16` in Config struct, default 50051 (gRPC convention)
**D-09:** Configurable via `MNEMONIC_GRPC_PORT` env var or `grpc_port` in TOML (same precedence as existing config fields)
**D-10:** `config show` CLI includes grpc_port in output (not a secret — no redaction needed)
**D-11:** gRPC service struct holds `Arc<MemoryService>`, `Arc<KeyService>`, `Arc<CompactionService>`, and `backend_name: String`
**D-12:** Both servers constructed from the same Arc instances in main.rs — no cloning of inner data
**D-13:** The tonic service struct is separate from axum's AppState — they share the same underlying Arc'd services but are different wrapper types
**D-14:** Implement as a Tower Layer/Service that wraps the gRPC service — NOT a tonic Interceptor (interceptors are sync, KeyService.validate() is async)
**D-15:** Extract bearer token from gRPC `authorization` metadata key (same "Bearer <token>" format as REST Authorization header)
**D-16:** Reuse `KeyService.validate()` for token validation and `KeyService.count_active_keys()` for open-mode check
**D-17:** On auth success, inject `AuthContext` into tonic `Request::extensions()` so handlers can extract it
**D-18:** Auth error mapping: missing token when auth active → `Status::Unauthenticated`, invalid/revoked token → `Status::Unauthenticated`, malformed header → `Status::InvalidArgument`
**D-19:** Open mode (zero active keys) bypasses auth entirely — request passes through with no AuthContext in extensions
**D-20:** All gRPC code is behind `#[cfg(feature = "interface-grpc")]` — default binary has zero gRPC code or dependencies
**D-21:** When built without `interface-grpc`, `mnemonic serve` starts only the REST server (current behavior preserved exactly)
**D-22:** The `grpc_port` config field exists unconditionally in Config struct but is only used when the feature is enabled

### Claude's Discretion
- Exact Tower Layer boilerplate structure (Pin<Box<dyn Future>> vs async-trait approach)
- Whether to use a separate auth module file within src/grpc/ or inline the layer in mod.rs
- Test strategy for auth layer (integration vs unit)
- Exact error message wording for gRPC auth failures

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SERVER-01 | Dual-port startup — REST on existing port, gRPC on configurable grpc_port via tokio::try_join! | tokio::try_join! across two independent TcpListener binds is the correct pattern; same-port multiplexing has known bugs (tonic #1964, axum #2825) |
| SERVER-02 | grpc_port configuration field in Config struct (env var MNEMONIC_GRPC_PORT + TOML grpc_port) | Figment auto-maps MNEMONIC_GRPC_PORT to Config.grpc_port via Env::prefixed("MNEMONIC_") — no extra code needed |
| SERVER-03 | Shared AppState — gRPC server shares Arc<MemoryService>, Arc<KeyService>, Arc<EmbeddingEngine> with REST server | Arc reference counting in main.rs; gRPC service struct holds same Arc instances as AppState without inner data cloning |
| AUTH-01 | Bearer token auth via gRPC `authorization` metadata key using async Tower Layer (not sync interceptor) | Hand-rolled Tower Layer is required; tonic-async-interceptor 0.13.x does not exist; tonic-middleware 0.4.1 requires tonic 0.14 |
| AUTH-02 | Agent scope enforcement — gRPC handlers enforce agent_id matches API key scope (same logic as REST) | enforce_scope() in src/server.rs is available for Phase 28 handler reuse; Phase 27 only needs the Layer to inject AuthContext |
| AUTH-03 | Open mode bypass — no auth required when zero API keys exist (consistent with REST behavior) | KeyService.count_active_keys() == 0 bypasses auth; same logic as REST auth_middleware (src/auth.rs:253-257) |
</phase_requirements>

## Standard Stack

### Core (all already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tonic | 0.13.1 | gRPC server (Server::builder, add_service, serve) | Already pinned in Phase 26 |
| tower | 0.5.3 | Layer/Service traits for async middleware | Transitive dep; already in dev-deps explicitly |
| tokio | 1 (full) | try_join!, TcpListener::bind | Runtime already used everywhere |
| tonic-health | 0.13.1 | grpc.health.v1 SERVING status | Already in interface-grpc feature |
| tonic-reflection | 0.13.1 | grpcurl service discovery | Already in interface-grpc feature |

### No New Dependencies Required
All required crates are already declared in Cargo.toml behind the `interface-grpc` feature. Phase 27 adds zero new `[dependencies]` lines.

For the Tower Layer implementation, only standard library types are needed (`std::pin::Pin`, `std::future::Future`, `std::task::{Context, Poll}`). These are not crates — they are in `std`.

**Version verification (verified from local cargo tree 2026-03-22):**
- tonic 0.13.1 — confirmed
- tower 0.5.3 — confirmed
- tower-layer 0.3.3 — confirmed
- tower-service 0.3.3 — confirmed
- tonic-health 0.13.1 — confirmed
- tonic-reflection 0.13.1 — confirmed
- http 1.4.0 — confirmed

### Alternatives Considered and Rejected
| Instead of | Could Use | Why Rejected |
|------------|-----------|--------------|
| Hand-rolled Tower Layer | tonic-async-interceptor | No 0.13.x version exists; latest 0.14.1 requires tonic 0.14 |
| Hand-rolled Tower Layer | tonic-middleware 0.3.x | Registry latest is 0.4.1 which requires tonic 0.14; 0.3.x API unclear |
| Separate TcpListener binds | Same-port HTTP+gRPC multiplexing | Documented body-type mismatch bugs: tonic #1964, axum #2825 |
| `tokio::try_join!` | `tokio::spawn` for each server | try_join! propagates first error and cancels both — correct fail-fast semantics for D-06 |

## Architecture Patterns

### Project Structure (additions only)
```
src/
├── grpc/                   # NEW — gated behind #[cfg(feature = "interface-grpc")]
│   ├── mod.rs              # Service struct, include_proto!, serve_grpc()
│   └── auth.rs             # Tower Layer + Service for async gRPC auth (Claude's discretion: separate file)
├── main.rs                 # MODIFY: add mod grpc (cfg-gated), replace serve() with try_join!
├── server.rs               # UNCHANGED (serve() function stays, REST continues on config.port)
├── config.rs               # MODIFY: add grpc_port: u16 field (default 50051)
└── auth.rs                 # UNCHANGED (KeyService, AuthContext reused by grpc/auth.rs)
```

### Pattern 1: Generated Module Access (tonic include_proto!)
**What:** The `tonic::include_proto!` macro includes the generated `.rs` file from `OUT_DIR`. For package `mnemonic.v1`, the file is `mnemonic.v1.rs` and the macro argument is `"mnemonic.v1"`.
**Generated names:**
- Module: `mnemonic_service_server` (snake_case of proto service name)
- Trait: `MnemonicService` (implement this trait on the service struct)
- Server wrapper: `MnemonicServiceServer<T>` (wraps the impl, passed to add_service)

```rust
// Source: verified from OUT_DIR/mnemonic.v1.rs
pub mod proto {
    tonic::include_proto!("mnemonic.v1");
}
use proto::mnemonic_service_server::{MnemonicService, MnemonicServiceServer};
```

### Pattern 2: Tower Layer for Async Auth (hand-rolled)
**What:** A Tower `Layer` wraps the gRPC service. The `Service` impl performs async auth using `KeyService`. Receives `http::Request<tonic::body::Body>` (not `tonic::Request<T>`).
**When to use:** Whenever interceptors are sync but the auth logic is async. This is the only correct approach for tonic 0.13 when async operations are needed.

```rust
// Source: verified from tonic-0.13.1/src/transport/server/mod.rs line 73, 547
// BoxService type = BoxCloneService<Request<Body>, Response<Body>, BoxError>
// serve() bound: L::Service: Service<Request<Body>, Response = Response<ResBody>>
// Body = tonic::body::Body (from tonic::body module)

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tonic::body::Body;
use tower::{Layer, Service};
use http::Request;

#[derive(Clone)]
pub struct GrpcAuthLayer {
    pub key_service: Arc<crate::auth::KeyService>,
}

impl<S> Layer<S> for GrpcAuthLayer {
    type Service = GrpcAuthService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        GrpcAuthService {
            inner,
            key_service: Arc::clone(&self.key_service),
        }
    }
}

#[derive(Clone)]
pub struct GrpcAuthService<S> {
    inner: S,
    key_service: Arc<crate::auth::KeyService>,
}

impl<S, ResBody> Service<Request<Body>> for GrpcAuthService<S>
where
    S: Service<Request<Body>, Response = http::Response<ResBody>>
        + Clone + Send + 'static,
    S::Future: Send + 'static,
    ResBody: Default + Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // CRITICAL: clone + swap to avoid moving self.inner into async block
        // while also being able to call poll_ready again on the original
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let key_service = Arc::clone(&self.key_service);

        Box::pin(async move {
            // Open mode check
            match key_service.count_active_keys().await {
                Ok(0) => return inner.call(req).await,
                Err(_) => {
                    return Ok(tonic::Status::unauthenticated("auth service unavailable")
                        .into_http());
                }
                Ok(_) => {}
            }

            // Extract authorization header from HTTP headers (NOT tonic metadata)
            let auth_header = req.headers().get("authorization");
            let bearer = match auth_header.and_then(|v| v.to_str().ok()) {
                None => {
                    return Ok(tonic::Status::unauthenticated("missing authorization header")
                        .into_http());
                }
                Some(raw) => match raw.strip_prefix("Bearer ") {
                    Some(token) if !token.is_empty() => token.to_string(),
                    _ => {
                        return Ok(
                            tonic::Status::invalid_argument(
                                "authorization header must use format: Bearer <token>"
                            ).into_http()
                        );
                    }
                },
            };

            // Validate token
            match key_service.validate(&bearer).await {
                Ok(auth_ctx) => {
                    let mut req = req;
                    req.extensions_mut().insert(auth_ctx);
                    inner.call(req).await
                }
                Err(_) => Ok(tonic::Status::unauthenticated("invalid or revoked API key")
                    .into_http()),
            }
        })
    }
}
```

**CRITICAL NOTE:** `tonic::Status::into_http::<B: Default>()` produces `http::Response<B>`. Since `ResBody: Default`, calling `.into_http()` without a type annotation produces `http::Response<ResBody>` which satisfies the service's `Response = http::Response<ResBody>` bound.

### Pattern 3: Dual-Server tokio::try_join!
**What:** Both servers bind independent TCP listeners and run concurrently. First error terminates both.
**Why try_join! not spawn:** Panics in spawned tasks are lost; try_join! propagates the first Err to main, which exits with a clear error message.

```rust
// Source: tokio docs, CONTEXT.md D-04, D-06
// In main.rs, replace: server::serve(&config, state).await?;
// With the following (inside #[cfg(feature = "interface-grpc")] block):

let rest_fut = server::serve(&config, state.clone());
let grpc_fut = grpc::serve_grpc(&config, /* grpc service struct */);
tokio::try_join!(rest_fut, grpc_fut)?;

// Feature-flag fallback (when feature is off — D-21):
#[cfg(not(feature = "interface-grpc"))]
server::serve(&config, state).await?;
```

### Pattern 4: serve_grpc() Function
**What:** Mirrors the existing `server::serve()` function but for tonic.

```rust
// Source: tonic-0.13.1 Router::serve() signature — verified
pub async fn serve_grpc(config: &Config, svc: MnemonicGrpcService) -> anyhow::Result<()> {
    let addr: SocketAddr = format!("0.0.0.0:{}", config.grpc_port).parse()?;
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<MnemonicServiceServer<MnemonicGrpcService>>().await;

    tonic::transport::Server::builder()
        .layer(GrpcAuthLayer { key_service: Arc::clone(&svc.key_service) })
        .add_service(health_service)
        .add_service(MnemonicServiceServer::new(svc))
        .serve(addr)
        .await?;
    Ok(())
}
```

### Pattern 5: Config Extension (grpc_port field)
**What:** Adding `grpc_port: u16` to the Config struct. Figment auto-maps `MNEMONIC_GRPC_PORT` env var via `Env::prefixed("MNEMONIC_")`.

```rust
// Source: src/config.rs — replicate existing pattern for port field
pub struct Config {
    // ... existing fields ...
    pub grpc_port: u16,  // ADD: default 50051, gated unconditionally (D-22)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            grpc_port: 50051,
        }
    }
}
```

### Pattern 6: tonic-reflection File Descriptor Set
**What:** For `grpcurl` service discovery, tonic-reflection needs the proto's file descriptor set. tonic-build can emit a `.bin` file alongside the `.rs` file.
**How:** Update `build.rs` to use `compile_protos_with_config` with `file_descriptor_set_path` set, then `include_bytes!` to embed it.

```rust
// In build.rs (modify tonic_build call):
tonic_build::configure()
    .file_descriptor_set_path(
        std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap())
            .join("mnemonic.v1.bin")
    )
    .compile_protos(&["proto/mnemonic.proto"], &["proto"])
    .expect("Failed to compile proto/mnemonic.proto");

// In src/grpc/mod.rs:
const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("mnemonic.v1");

// Register with reflection builder:
tonic_reflection::server::Builder::configure()
    .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
    .build_v1()?
```

**NOTE:** Phase 27 does not require tonic-reflection (that is HEALTH-02, a Phase 28 requirement). The file descriptor set work goes in Phase 28. Phase 27 only needs `tonic-health` for the SERVING status requirement (success criterion 1: grpcurl health check returns SERVING).

### Anti-Patterns to Avoid
- **Sync interceptor for async auth:** `tonic::service::Interceptor` is sync. Calling `.await` inside it panics (`block_on` inside tokio runtime panics).
- **Same-port multiplexing:** Do not use `axum_tonic_mux` or similar. Documented body-type mismatch bugs.
- **`tonic::Request::metadata()`in Tower Layer:** The Tower Layer receives `http::Request<Body>`, not `tonic::Request<T>`. Use `req.headers().get("authorization")` not `req.metadata().get("authorization")`.
- **Moving inner into async block without clone+swap:** The Tower Service pattern requires `mem::replace` to move `self.inner` into the async block; otherwise the service would be consumed on first call.
- **`tonic-middleware` or `tonic-async-interceptor` for tonic 0.13:** Latest versions require tonic 0.14. Adding them would conflict.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| gRPC health checking | Custom health RPC | tonic-health::server::health_reporter() | Standard grpc.health.v1 protocol; grpcurl checks this specific service |
| gRPC service skeleton | Raw hyper/h2 | tonic::transport::Server::builder() | Handles HTTP/2 framing, multiplexing, trailers |
| Token hashing/comparison | Custom hash | KeyService.validate() (already exists) | Constant-time comparison, BLAKE3 hash already wired |
| Open mode detection | Custom key count | KeyService.count_active_keys() (already exists) | Same DB query used by REST auth |
| Auth context injection | Custom extension type | AuthContext (already exists in src/auth.rs) | Phase 28 handlers extract this same type from extensions |

**Key insight:** The entire auth layer is assembling existing pieces (KeyService, AuthContext, tonic Status codes) — no new auth logic is required.

## Common Pitfalls

### Pitfall 1: Using tonic::Request::metadata() in Tower Layer
**What goes wrong:** `req.metadata()` compiles only on `tonic::Request<T>`. The Tower Layer receives `http::Request<tonic::body::Body>`. Calling `.metadata()` produces "no method named `metadata` found".
**Why it happens:** tonic has two Request types. Interceptors get `tonic::Request`; Tower Layers get `http::Request`.
**How to avoid:** Use `req.headers().get("authorization")` in the Tower Layer.
**Warning signs:** Compiler error "no method named `metadata`" on the Request parameter.

### Pitfall 2: Forgetting clone+swap in Service::call
**What goes wrong:** The async block inside `call` cannot borrow `self.inner` because `call` takes `&mut self`. Moving `self.inner` into the async block consumes self on the first call, making the service unusable.
**Why it happens:** Tower services are designed to be called multiple times. The async block needs ownership of `inner` but service must retain it.
**How to avoid:** Use `let clone = self.inner.clone(); let mut inner = std::mem::replace(&mut self.inner, clone);` before the `Box::pin(async move { ... })`.
**Warning signs:** "cannot move out of `self.inner` which is behind a mutable reference" compiler error.

### Pitfall 3: Status::into_http() type inference failure
**What goes wrong:** `tonic::Status::into_http()` has type parameter `B: Default`. Without a type annotation the compiler cannot infer `B` and produces "cannot infer type for type parameter `B`".
**Why it happens:** The generic return type needs to match `ResBody` from the service bound.
**How to avoid:** The `GrpcAuthService<S>` bound `ResBody: Default` allows inference if `S::Response = http::Response<ResBody>`. Call `.into_http::<ResBody>()` explicitly if inference fails, or ensure the bound chains correctly.
**Warning signs:** "cannot infer type for type parameter" on the `.into_http()` call.

### Pitfall 4: grpc_port field breaks existing Config tests
**What goes wrong:** Adding `grpc_port: u16` to Config without adding it to `Default::default()` causes `Config { port: 0, ..Config::default() }` constructions in tests to fail to compile (missing field).
**Why it happens:** Rust struct update syntax `..Config::default()` fills in all unspecified fields from Default — so as long as Default is updated, existing code with struct update syntax continues to compile.
**How to avoid:** Always add the new field to BOTH the struct definition AND `Default::default()`. Add a test asserting `config.grpc_port == 50051` alongside the existing `config.port == 8080` test.
**Warning signs:** Compilation errors in `src/auth.rs` test helper `test_key_service()` which uses `Config { ..Config::default() }` — it would fail if Default doesn't include grpc_port.

### Pitfall 5: try_join! requires both futures to return the same error type
**What goes wrong:** `tokio::try_join!` requires all futures to return `Result<_, E>` where `E` is the same type. `server::serve()` returns `anyhow::Result<()>` and `grpc::serve_grpc()` returns `anyhow::Result<()>` — this is fine. But if one returns `tonic::transport::Error` and the other returns `axum::Error`, try_join! fails to compile.
**How to avoid:** Both `serve()` and `serve_grpc()` must return `anyhow::Result<()>`. Map tonic errors with `.map_err(anyhow::Error::from)` or use `?` inside an `anyhow::Result` context.
**Warning signs:** "the trait bound `anyhow::Error: From<tonic::transport::Error>` is not satisfied" at the try_join! call site.

### Pitfall 6: Feature-gating the grpc module but not the grpc_port config field
**What goes wrong:** Per D-22, `grpc_port` exists unconditionally in Config. If someone accidentally gates it behind `#[cfg(feature = "interface-grpc")]`, the REST-only binary fails to parse config because the field is missing from the struct.
**How to avoid:** Add `grpc_port: u16` to Config struct unconditionally. Only gate the `serve_grpc()` call and `mod grpc;` declaration.

### Pitfall 7: Health service bypasses auth layer
**What goes wrong:** The health service is added via `add_service(health_service)` — it sits behind the auth layer because `Server::builder().layer(auth_layer)` wraps ALL services. In open mode this is fine. In auth-active mode, grpcurl health checks require a bearer token.
**Why it happens:** `Server::builder().layer()` wraps every service registered with `add_service`.
**How to avoid:** The auth layer already implements open-mode pass-through. For auth-active mode, grpcurl health checks should supply a token. Document this in the startup log if needed. Alternatively, skip auth for the `/grpc.health.v1.Health/Check` path by inspecting the URI in the Tower Layer. Research decision: check if the CONTEXT.md specifies behavior. It does not, so this is Claude's discretion — recommend URI-based health bypass in the Tower Layer.
**Warning signs:** grpcurl health check fails with UNAUTHENTICATED when API keys are configured.

## Code Examples

Verified patterns from tonic 0.13.1 source (inspected locally):

### include_proto! Macro Usage
```rust
// Source: tonic 0.13.1 include_proto macro, verified OUT_DIR/mnemonic.v1.rs
// Package name in proto = "mnemonic.v1" → include_proto! argument = "mnemonic.v1"
pub mod proto {
    tonic::include_proto!("mnemonic.v1");
}
// Generated types accessible as:
// proto::Memory, proto::StoreMemoryRequest, etc.
// proto::mnemonic_service_server::MnemonicService (trait)
// proto::mnemonic_service_server::MnemonicServiceServer<T> (server wrapper)
```

### Unimplemented Service Stub (Phase 27 skeleton)
```rust
// Source: pattern from tonic generated trait shape (verified in OUT_DIR)
#[tonic::async_trait]
impl MnemonicService for MnemonicGrpcService {
    async fn store_memory(
        &self,
        _request: tonic::Request<proto::StoreMemoryRequest>,
    ) -> Result<tonic::Response<proto::StoreMemoryResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("StoreMemory not yet implemented"))
    }
    // ... same for search_memory, list_memories, delete_memory
}
```

### tonic-health Setup
```rust
// Source: tonic-health-0.13.1/src/server.rs line 21 — verified locally
let (mut health_reporter, health_svc) = tonic_health::server::health_reporter();
// Set the MnemonicService as SERVING
health_reporter
    .set_serving::<MnemonicServiceServer<MnemonicGrpcService>>()
    .await;
// Add to server:
Server::builder()
    .add_service(health_svc)
    .add_service(MnemonicServiceServer::new(grpc_svc))
    .serve(addr)
    .await?;
```

### Status Error to HTTP Response
```rust
// Source: tonic-0.13.1/src/status.rs line 578 — verified locally
// Status::into_http::<B: Default>() → http::Response<B>
// Used in Tower Layer to return gRPC error response from HTTP Service:
let err_response: http::Response<Body> =
    tonic::Status::unauthenticated("invalid or revoked API key").into_http();
return Ok(err_response);
```

### Dual-Port Startup Pattern
```rust
// Source: tokio docs, CONTEXT.md D-04
// In main.rs after building state and grpc_svc:
#[cfg(feature = "interface-grpc")]
{
    let rest_fut = server::serve(&config, state);
    let grpc_fut = grpc::serve_grpc(&config, grpc_svc);
    tokio::try_join!(rest_fut, grpc_fut)?;
}
#[cfg(not(feature = "interface-grpc"))]
server::serve(&config, state).await?;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| tonic::Interceptor (sync) | Tower Layer (async) | tonic 0.5+ | Enables async KeyService calls without block_on |
| Same-port HTTP+gRPC routing | Separate TcpListener binds | axum 0.6+ | Avoids body-type mismatch bugs |
| BoxBody (old tonic versions) | tonic::body::Body (tonic 0.13) | tonic 0.13 | Single public Body type; BoxBody is private |
| tonic-async-interceptor for 0.12 | Hand-rolled Tower Layer for 0.13 | tonic 0.13 release | No compatible async-interceptor crate for 0.13 |

**Deprecated/outdated:**
- `tonic::body::BoxBody`: Private in tonic 0.13. Public type is `tonic::body::Body`. Use `Body` in service bounds.
- `RequireAuthorizationLayer::bearer()` from tower-http: Works for static tokens only; cannot call async KeyService.

## Open Questions

1. **Health check auth bypass**
   - What we know: Tower Layer wraps all services including health; in auth-active mode, grpcurl health checks will require a bearer token
   - What's unclear: CONTEXT.md does not specify expected behavior; success criterion 1 says "grpcurl health check returns SERVING" without specifying auth headers
   - Recommendation: Add URI check in Tower Layer — if `req.uri().path()` starts with `/grpc.health.v1.Health/`, bypass auth and pass through. This matches the REST pattern where `/health` is a public route. Document this decision in the plan.

2. **tonic-reflection for Phase 27 vs Phase 28**
   - What we know: tonic-reflection is already in Cargo.toml; success criterion requires grpcurl health check (not reflection); HEALTH-02 (reflection) is a Phase 28 requirement
   - What's unclear: Whether `serve_grpc()` should register the reflection service now or defer to Phase 28
   - Recommendation: Defer reflection registration to Phase 28 (it is HEALTH-02). Phase 27 only needs health service for success criterion 1. The file descriptor set infrastructure (build.rs `file_descriptor_set_path`) should also be deferred to Phase 28 to keep this phase minimal.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | none (no external config; uses `#[tokio::test]` and `#[test]`) |
| Quick run command | `cargo test --features interface-grpc 2>&1` |
| Full suite command | `cargo test --features interface-grpc,backend-qdrant,backend-postgres 2>&1` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SERVER-01 | Both servers bind and accept connections | integration | `cargo test --features interface-grpc dual_port` | Wave 0 |
| SERVER-02 | MNEMONIC_GRPC_PORT env var sets grpc_port | unit | `cargo test test_grpc_port_default` (in config.rs tests) | Wave 0 |
| SERVER-03 | gRPC service struct holds same Arc instances as AppState | unit | verify construction compiles + Arc::ptr_eq | Wave 0 |
| AUTH-01 | Invalid bearer token returns Status::Unauthenticated | unit | `cargo test --features interface-grpc grpc_auth_invalid_token` | Wave 0 |
| AUTH-02 | AuthContext injected into extensions on valid token | unit | `cargo test --features interface-grpc grpc_auth_valid_token_injects_context` | Wave 0 |
| AUTH-03 | Open mode (zero keys) bypasses auth | unit | `cargo test --features interface-grpc grpc_auth_open_mode_bypasses` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --features interface-grpc 2>&1` (unit tests only, fast)
- **Per wave merge:** `cargo test --features interface-grpc 2>&1` (full feature suite)
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/grpc/auth.rs` unit tests for Tower Layer auth behavior (open mode, valid token, invalid token, malformed header)
- [ ] `src/config.rs` test for `grpc_port` default value (50051) and MNEMONIC_GRPC_PORT env override
- [ ] Dual-port integration test: bind both ports, verify health service responds (may be a standalone integration test or a #[cfg(feature)] block in tests/integration.rs)

*(Existing test infrastructure in tests/integration.rs and src/auth.rs covers KeyService — no changes needed there)*

## Sources

### Primary (HIGH confidence)
- tonic-0.13.1 source code (inspected locally from cargo registry) — body types, serve() trait bounds, Server::layer() API
- tonic-health-0.13.1 source code (inspected locally) — health_reporter() API, set_serving() pattern
- tonic-reflection-0.13.1 source code (inspected locally) — Builder::configure(), register_encoded_file_descriptor_set(), build_v1() API
- OUT_DIR/mnemonic.v1.rs (inspected locally) — verified generated module name `mnemonic_service_server`, trait `MnemonicService`, server `MnemonicServiceServer<T>`
- src/auth.rs (project source) — KeyService.validate(), count_active_keys(), AuthContext — confirmed reusable
- src/config.rs (project source) — Figment Env::prefixed pattern confirmed for auto-mapping MNEMONIC_ vars

### Secondary (MEDIUM confidence)
- [tonic transport server docs](https://docs.rs/tonic/0.13.0/tonic/transport/server/struct.Server.html) — Server::builder() API, layer() method signature
- [Tower async middleware pattern](https://mark-story.com/posts/view/request-signatures-with-tonic-tower) — clone+swap pattern for Service::call, UnsyncBoxBody usage (verified against tonic source)
- [tonic examples authentication server](https://github.com/hyperium/tonic/blob/master/examples/src/authentication/server.rs) — interceptor pattern (sync; confirms why Tower Layer is required instead)
- [tonic-health docs](https://docs.rs/tonic-health/0.13.0/tonic_health/server/index.html) — health_reporter() function description

### Tertiary (LOW confidence — flag for validation)
- tonic-middleware 0.4.1 / tonic-async-interceptor 0.14.1 version compatibility: confirmed incompatible via cargo search and GitHub Cargo.toml inspection, but 0.3.x behavior is not independently verified from source

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions verified from local cargo tree
- Architecture patterns: HIGH — verified from tonic 0.13.1 source code directly
- Tower Layer boilerplate: HIGH — verified from source + multiple examples
- Health service API: HIGH — verified from tonic-health-0.13.1 source
- Pitfalls: HIGH — pitfalls 1-3 verified from source code analysis; pitfalls 4-7 verified from existing project patterns

**Research date:** 2026-03-22
**Valid until:** 2026-06-22 (stable — tonic 0.13 is pinned; no expected breaking changes within 90 days given the prost version constraint)
