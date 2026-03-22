# Architecture Research

**Domain:** Rust single-binary agent memory server — v1.5 gRPC (tonic) integration
**Researched:** 2026-03-22
**Confidence:** HIGH (direct codebase inspection + verified against tonic docs, axum GitHub discussions, official examples)

---

## Context: What Already Exists (v1.4)

The v1.4 binary is ~10,763 lines of Rust across 13 source files. The current server architecture is:

```
AppState {
    service:      Arc<MemoryService>,        // holds Arc<dyn StorageBackend>
    compaction:   Arc<CompactionService>,    // holds Arc<dyn StorageBackend> + audit_db
    key_service:  Arc<KeyService>,           // holds Arc<Connection> (SQLite-only, auth stays local)
    backend_name: String,                    // display string from config.storage_provider
}
```

`server::serve()` binds a single `TcpListener` on `config.port` (default 8080) and runs the axum `Router`.
Auth is an axum `middleware::from_fn_with_state` applied via `route_layer` — it extracts `Authorization: Bearer mnk_...` from HTTP headers, calls `KeyService` for validation, and injects `AuthContext` into request extensions.

**The key question for v1.5:** How does tonic attach to this process without requiring a full architectural rewrite?

---

## v1.5 System Overview

The recommended approach is **dual-port, separate listeners, shared AppState**. Both servers share the same `Arc<MemoryService>` and `Arc<KeyService>` but run on independent `TcpListener` binds started concurrently with `tokio::join!`.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Entry Point (main.rs)                             │
│                                                                              │
│  Build shared state (unchanged from v1.4):                                  │
│    Arc<dyn StorageBackend>, Arc<MemoryService>, Arc<KeyService>,             │
│    Arc<CompactionService>                                                    │
│                                                                              │
│  Build AppState (unchanged struct)                                           │
│  Build GrpcState { service: Arc<MemoryService>, key_service: Arc<KeyService>}│
│                                                                              │
│  tokio::join!(                                                               │
│      server::serve(&config, app_state),          // REST on config.port      │
│      grpc::serve(&config, grpc_state),           // gRPC on config.grpc_port │
│  )                                                                           │
└─────────────────────────────────────────────────────────────────────────────┘
         │                                    │
         ▼                                    ▼
┌─────────────────────┐          ┌────────────────────────────┐
│  REST Server        │          │  gRPC Server (tonic)        │
│  axum on :8080      │          │  tonic on :50051            │
│                     │          │                             │
│  auth_middleware    │          │  AuthInterceptor            │
│  (axum layer)       │          │  (tonic interceptor)        │
│                     │          │                             │
│  MemoryService      │          │  MnemonicGrpcService        │
│  CompactionService  │          │  (delegates to MemoryService│
│  KeyService         │          │   and KeyService)           │
└─────────────────────┘          └────────────────────────────┘
         │                                    │
         └──────────────┬─────────────────────┘
                        ▼
         ┌──────────────────────────────┐
         │   Shared Service Layer       │
         │                             │
         │   Arc<MemoryService>        │ ← same instance
         │   Arc<KeyService>           │ ← same instance
         │   Arc<dyn StorageBackend>   │ ← same backend
         └──────────────────────────────┘
```

---

## Why Dual-Port (Not Single-Port Multiplexing)

Single-port multiplexing via `Content-Type: application/grpc` header routing is technically possible (it is the approach used by the axum `rest-grpc-multiplex` example), but it adds meaningful complexity:

- The multiplexer must unify response body types (`axum::body::Body` vs tonic's `BoxBody`) into a single type. This requires either custom `HybridBody` enums or `BoxBody` everywhere.
- The axum PR #2825 that "fixes" the multiplex example uses `tower::steer` and `tonic::transport::Server::into_router()` — `into_router()` is marked as experimental and its stability is unclear across tonic minor versions.
- The `axum-tonic` crate (the wrapper library) is at v0.1.0, last released January 2023 — not appropriate as a production dependency.
- The `tonic-web` layer (needed to support browser gRPC clients) introduces body type mismatches documented in tonic issue #1964, requiring additional workarounds.
- Multiplexing produces one listener binding, which makes port-based firewall rules for gRPC impossible.

**Dual-port is simpler, more maintainable, and directly supported by tonic's primary `serve()` pattern.** The cost is one extra config field (`grpc_port`) and a `tokio::join!` in `main.rs`.

**Confidence:** MEDIUM (verified via official axum multiplex examples, tonic GitHub issues, and the fpblock.com multi-part guide; multiplexing is feasible but the ecosystem tooling is immature relative to dual-port)

---

## Component Responsibilities (v1.5)

| Component | Status | Responsibility | Notes |
|-----------|--------|---------------|-------|
| `src/server.rs` | Modified | REST server — unchanged behavior | Add `build_router` export, no logic changes |
| `src/grpc/mod.rs` | New | gRPC server entry point — `serve()`, service wiring | Mirrors `server::serve()` structure |
| `src/grpc/service.rs` | New | `MnemonicGrpcService` struct implementing generated trait | Delegates to `Arc<MemoryService>` |
| `src/grpc/interceptor.rs` | New | `AuthInterceptor` — extracts Bearer from gRPC metadata, validates via `KeyService` | Mirrors axum `auth_middleware` logic |
| `proto/mnemonic.proto` | New | Service definition for store, search, list, delete, health | See proto design section |
| `build.rs` | New | Invokes `tonic_build` to generate Rust from `.proto` | Standard tonic-build pattern |
| `src/config.rs` | Modified | Add `grpc_port: u16`, `grpc_tls_cert/key: Option<String>` fields | Default `grpc_port = 50051` |
| `src/main.rs` | Modified | `tokio::join!(server::serve(), grpc::serve())` instead of just `server::serve()` | Arc-clone state for each server |
| `src/cli.rs` | Modified | Fix recall to route through `StorageBackend` trait (v1.4 tech debt) | Unrelated to gRPC wiring |

---

## New File Structure

```
mnemonic/
├── build.rs                      # NEW: tonic-build code generation
├── proto/
│   └── mnemonic.proto            # NEW: service definition
└── src/
    ├── grpc/
    │   ├── mod.rs                # NEW: serve() function, module exports
    │   ├── service.rs            # NEW: MnemonicGrpcService impl
    │   └── interceptor.rs        # NEW: AuthInterceptor impl
    ├── config.rs                 # MODIFIED: grpc_port, optional TLS fields
    ├── main.rs                   # MODIFIED: tokio::join! dual serve
    └── server.rs                 # UNCHANGED (or trivial export cleanup)
```

**Proto file placement rationale:** `proto/` at project root is the conventional location used by all tonic examples and the `tonic-build` documentation. `build.rs` references it as `"proto/mnemonic.proto"` relative to `Cargo.toml`.

---

## Proto Design

The `.proto` defines unary RPCs only (per v1.5 scope — no streaming). Operations covered: store, search, list, delete, health. Compaction and key management are REST-only.

```protobuf
syntax = "proto3";
package mnemonic;

service Mnemonic {
  rpc StoreMemory(StoreRequest) returns (Memory);
  rpc SearchMemories(SearchRequest) returns (SearchResponse);
  rpc ListMemories(ListRequest) returns (ListResponse);
  rpc DeleteMemory(DeleteRequest) returns (Memory);
  rpc HealthCheck(HealthRequest) returns (HealthResponse);
}

message StoreRequest {
  string content = 1;
  string agent_id = 2;
  string session_id = 3;
  repeated string tags = 4;
}

message Memory {
  string id = 1;
  string content = 2;
  string agent_id = 3;
  string session_id = 4;
  repeated string tags = 5;
  string embedding_model = 6;
  string created_at = 7;
  optional float distance = 8;
}

message SearchRequest {
  string query = 1;
  string agent_id = 2;
  optional string session_id = 3;
  optional int32 limit = 4;
}

message SearchResponse {
  repeated Memory memories = 1;
}

message ListRequest {
  optional string agent_id = 1;
  optional string session_id = 2;
  optional int32 limit = 3;
  optional int32 offset = 4;
}

message ListResponse {
  repeated Memory memories = 1;
  int32 total = 2;
}

message DeleteRequest {
  string id = 1;
}

message HealthRequest {}

message HealthResponse {
  string status = 1;
  string backend = 2;
}
```

**Field mapping rationale:** Fields map 1:1 to existing REST request/response types (`CreateMemoryRequest`, `SearchParams`, `ListParams`, `Memory`). The `distance` field on `Memory` is `optional` because it is only populated by `SearchResponse`.

---

## Architectural Patterns

### Pattern 1: tonic Interceptor with Shared Arc State

Tonic interceptors implement `FnMut(Request<()>) -> Result<Request<()>, Status>` — they receive the request before the service method is called. The interceptor can hold `Arc<KeyService>` because the interceptor struct is `Clone` and `Arc` is `Clone`.

**What:** An `AuthInterceptor` struct wraps `Arc<KeyService>`, implements `tonic::service::Interceptor`, extracts `authorization` metadata from the gRPC request, and validates the token using the same `KeyService::verify()` logic that the axum middleware uses.

**When to use:** For any per-request cross-cutting concern (auth, rate limiting) that only needs to read request metadata and either allow or reject. For response-level concerns (logging, metrics), use Tower middleware layered on the tonic server builder instead.

**Concrete structure:**

```rust
// src/grpc/interceptor.rs

#[derive(Clone)]
pub struct AuthInterceptor {
    pub key_service: Arc<KeyService>,
}

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(&mut self, mut req: Request<()>) -> Result<Request<()>, Status> {
        // Extract "authorization" from gRPC metadata (maps to HTTP/2 header)
        match req.metadata().get("authorization") {
            Some(token_value) => {
                let token_str = token_value.to_str()
                    .map_err(|_| Status::unauthenticated("invalid authorization header"))?;
                // Strip "Bearer " prefix — same as axum middleware
                let raw = token_str.strip_prefix("Bearer ")
                    .ok_or_else(|| Status::unauthenticated("authorization must be Bearer token"))?;
                // Blocking verify — interceptors are sync; use block_in_place or
                // pre-load auth context in a tower layer if async validation is needed
                let auth_ctx = futures::executor::block_on(
                    self.key_service.verify(raw)
                ).map_err(|_| Status::unauthenticated("invalid or revoked token"))?;
                // Inject AuthContext into request extensions for use in service methods
                req.extensions_mut().insert(auth_ctx);
                Ok(req)
            }
            None => {
                // Open mode: check if any keys exist (same live-check as axum middleware)
                let count = futures::executor::block_on(
                    self.key_service.count_active_keys()
                ).unwrap_or(0);
                if count == 0 {
                    Ok(req) // open mode — no keys, pass through
                } else {
                    Err(Status::unauthenticated("authorization required"))
                }
            }
        }
    }
}
```

**Important:** The `Interceptor::call` signature is synchronous (`fn`, not `async fn`). For async-capable interceptors, the `tonic-middleware` crate provides a `RequestInterceptor` async trait (last release 2024, actively maintained). Given that `KeyService::verify()` is a fast SQLite lookup, `block_in_place` (tokio) or `block_on` is acceptable and avoids the extra dependency. Evaluate based on profiling.

**Confidence:** HIGH — verified against tonic official `authentication/server.rs` example and `tonic::service::Interceptor` docs.

### Pattern 2: Tonic Service Delegating to Existing Service Layer

The generated trait (`mnemonic_server::Mnemonic`) is implemented by a struct that holds `Arc<MemoryService>` and `Arc<KeyService>`. No new business logic lives here — this is a thin adapter that translates proto types to/from the existing service types.

**What:** `MnemonicGrpcService` holds the same `Arc<MemoryService>` that `AppState` holds. Both REST and gRPC call the same `MemoryService` methods — business logic is not duplicated.

**Concrete structure:**

```rust
// src/grpc/service.rs

#[derive(Clone)]
pub struct MnemonicGrpcService {
    pub memory_service: Arc<MemoryService>,
    pub backend_name: String,
}

#[tonic::async_trait]
impl mnemonic_server::Mnemonic for MnemonicGrpcService {
    async fn store_memory(
        &self,
        request: Request<StoreRequest>,
    ) -> Result<Response<Memory>, Status> {
        let req = request.into_inner();
        let create_req = CreateMemoryRequest {
            content: req.content,
            agent_id: Some(req.agent_id),
            session_id: Some(req.session_id),
            tags: Some(req.tags),
        };
        let memory = self.memory_service.create_memory(create_req)
            .await
            .map_err(grpc_error)?;
        Ok(Response::new(memory_to_proto(memory)))
    }

    // ... search_memories, list_memories, delete_memory, health_check
}

// Error translation: ApiError → tonic::Status
fn grpc_error(e: ApiError) -> Status {
    match e {
        ApiError::NotFound => Status::not_found("memory not found"),
        ApiError::BadRequest(msg) => Status::invalid_argument(msg),
        ApiError::Forbidden(msg) => Status::permission_denied(msg),
        ApiError::Internal(_) => Status::internal("internal server error"),
        _ => Status::internal("internal server error"),
    }
}
```

**Trade-off:** Proto type conversion (proto `Memory` ↔ service `Memory`) requires manual mapping functions. These are mechanical but must be kept in sync when service types change. Consider a `From` impl on the proto-generated type to centralize the mapping.

### Pattern 3: Dual Server Startup with tokio::join!

**What:** `main.rs` constructs shared state once, clones `Arc` handles for the gRPC server, then starts both servers concurrently. If either server fails, the `?` propagates via `tokio::try_join!`.

**Concrete structure:**

```rust
// In main.rs, replace:
server::serve(&config, state).await?;

// With:
let grpc_state = grpc::GrpcState {
    memory_service: service.clone(),
    key_service: key_service.clone(),
    backend_name: config.storage_provider.clone(),
};
tokio::try_join!(
    server::serve(&config, state),
    grpc::serve(&config, grpc_state),
)?;
```

**Why `try_join!` not `join!`:** If the REST server fails to bind (port conflict), you want the entire process to exit immediately, not continue running a half-initialized gRPC server. `try_join!` propagates the first error.

---

## build.rs Configuration

```rust
// build.rs (project root, adjacent to Cargo.toml)
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false)      // no client needed in the server binary
        .compile_protos(
            &["proto/mnemonic.proto"],
            &["proto"],           // include path for imports
        )?;
    Ok(())
}
```

**Generated code location:** `target/debug/build/mnemonic-{hash}/out/mnemonic.rs`. Access it in source via `include!` macro (tonic standard pattern):

```rust
// src/grpc/mod.rs
pub mod proto {
    tonic::include_proto!("mnemonic"); // matches package name in .proto
}
```

**Cargo.toml additions:**

```toml
[dependencies]
tonic = "0.12"
prost = "0.13"

[build-dependencies]
tonic-build = "0.12"
```

**Version rationale:** tonic 0.12 is the current stable release as of early 2026, aligned with the existing `prost-types = "0.13"` already in `Cargo.toml` (used by `qdrant-client`). This avoids a version conflict.

**Confidence:** HIGH — verified against tonic-build docs.rs documentation and the tonic official examples.

---

## Auth Flow Comparison (REST vs gRPC)

| Step | REST (axum) | gRPC (tonic) |
|------|-------------|--------------|
| Token carrier | `Authorization: Bearer mnk_...` HTTP header | `authorization: Bearer mnk_...` gRPC metadata (lowercased) |
| Extraction point | `auth_middleware` via `route_layer` | `AuthInterceptor` via `with_interceptor` |
| Token validation | `KeyService::verify()` async call | Same `KeyService::verify()` call |
| Auth context injection | `req.extensions().insert(AuthContext)` | `req.extensions_mut().insert(AuthContext)` |
| Scope enforcement | `enforce_scope()` in each handler | Same logic in each gRPC method impl |
| Open mode detection | `KeyService::count_active_keys()` per request | Same check in interceptor |
| No-key pass-through | Returns `None` auth, handler allows | Interceptor calls `Ok(req)` without auth context |

**Key insight:** The auth logic is identical in structure. The only difference is where the token lives (HTTP header vs gRPC metadata header — both are HTTP/2 headers under the hood, just accessed via different APIs). No auth business logic is duplicated — both call the same `KeyService`.

---

## Data Flow (gRPC Request)

```
gRPC Client
    │  (HTTP/2, Content-Type: application/grpc)
    ▼
tokio::TcpListener (config.grpc_port, default :50051)
    │
    ▼
tonic::transport::Server
    │
    ▼
AuthInterceptor::call()
    │  ← extracts "authorization" metadata
    │  ← calls KeyService::count_active_keys() OR KeyService::verify()
    │  ← injects AuthContext into request extensions (if auth active)
    │  ← returns Status::unauthenticated if token invalid
    ▼
MnemonicGrpcService::{store_memory|search_memories|list_memories|delete_memory|health_check}()
    │  ← extracts AuthContext from extensions
    │  ← enforces agent_id scope (same enforce_scope logic as REST)
    │  ← translates proto types to service types
    ▼
Arc<MemoryService>::{create_memory|search_memories|list_memories|delete_memory}()
    │  (same service layer as REST — no duplication)
    ▼
Arc<dyn StorageBackend>::{store|search|list|delete}()
    │  (same storage backend as REST)
    ▼
Response<proto::Memory | proto::SearchResponse | ...>
```

---

## Config Changes

Add to `Config` struct in `src/config.rs`:

```rust
/// Port for the gRPC server. Defaults to 50051. 0 disables gRPC.
/// Set via MNEMONIC_GRPC_PORT env var or grpc_port in TOML config.
pub grpc_port: u16,
/// Path to TLS certificate file for gRPC (PEM). Optional — disables TLS if absent.
pub grpc_tls_cert: Option<String>,
/// Path to TLS private key file for gRPC (PEM). Optional — disables TLS if absent.
pub grpc_tls_key: Option<String>,
```

Default: `grpc_port: 50051`. `grpc_tls_cert` and `grpc_tls_key` both `None` (plaintext by default, per v1.5 scope which lists "optional TLS").

`validate_config` update: if `grpc_tls_cert` is set but `grpc_tls_key` is absent (or vice versa), bail with a clear error.

---

## Suggested Build Order

The phases below represent implementation ordering — each phase produces a working, testable increment.

| Phase | What Gets Built | Dependencies | Testable Outcome |
|-------|-----------------|-------------|------------------|
| 1 | `proto/mnemonic.proto` + `build.rs` + `tonic`/`prost`/`tonic-build` deps in `Cargo.toml` | None | `cargo build` succeeds; generated code compiles |
| 2 | `src/grpc/mod.rs` skeleton + `tonic::include_proto!` + `GrpcState` struct | Phase 1 | Module compiles; no server running yet |
| 3 | `MnemonicGrpcService` with `health_check` only + bare `grpc::serve()` + `tokio::try_join!` in `main.rs` | Phase 2 | `mnemonic serve` starts gRPC on :50051; `grpcurl` health check passes |
| 4 | `AuthInterceptor` + wired into `grpc::serve()` | Phase 3 | Open mode: requests pass through. Auth mode: invalid tokens rejected with `UNAUTHENTICATED` |
| 5 | `store_memory`, `search_memories`, `list_memories`, `delete_memory` in `MnemonicGrpcService` | Phase 4 | Full hot-path gRPC API functional; integration tests passing |
| 6 | `Config` grpc_port + grpc_tls fields + `validate_config` update + `config show` display | Phases 1-5 | Config documented; port configurable via env var |
| 7 | Fix recall CLI to route through `StorageBackend` trait (v1.4 tech debt) | None (independent) | `mnemonic recall` works with Qdrant/Postgres backends |
| 8 | Optional TLS for gRPC via `tonic::transport::ServerTlsConfig` | Phase 6 | `MNEMONIC_GRPC_TLS_CERT` + `MNEMONIC_GRPC_TLS_KEY` enable TLS |

**Phase ordering rationale:**
- Phase 1 first: `build.rs` code generation is a build prerequisite for all subsequent phases. Nothing gRPC-related compiles without it.
- Phase 3 before Phase 4: Validate the dual-listener pattern works before adding auth complexity. A bare health endpoint is the simplest possible gRPC server.
- Phase 4 before Phase 5: Auth must gate all service methods from the start. Adding auth after full method implementation risks forgetting scope enforcement on individual methods.
- Phase 7 independent: The recall CLI fix is v1.4 tech debt that doesn't touch the gRPC code path at all. It can be done in any order but is naturally grouped here as a cleanup phase.
- Phase 8 last: TLS requires cert/key files to test properly. Deferring to final phase keeps earlier phases fast to iterate on.

---

## Anti-Patterns

### Anti-Pattern 1: Duplicate Business Logic in gRPC Layer

**What people do:** Re-implement agent_id scope enforcement, embedding calls, or compaction logic directly in `MnemonicGrpcService` methods rather than delegating to existing services.

**Why it's wrong:** Creates two code paths for the same operation. When the business rule changes (e.g., scope enforcement logic), both paths must be updated. This is how bugs diverge between protocols.

**Do this instead:** `MnemonicGrpcService` is a thin adapter. Every method body should be: extract proto fields, translate to service type, call `self.memory_service.method()`, translate result to proto type, return. No business logic in the gRPC layer.

### Anti-Pattern 2: Blocking I/O in tonic Interceptors

**What people do:** Call async functions from the synchronous `Interceptor::call()` using `block_on()` without using `tokio::task::block_in_place()`.

**Why it's wrong:** `block_on()` inside an async context panics if called from within a tokio runtime (which tonic runs under). `futures::executor::block_on()` has the same problem.

**Do this instead:** Use `tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(async_fn()))` for blocking within an async runtime, or switch to `tonic-middleware`'s async `RequestInterceptor` trait which accepts async interceptors natively. For the `KeyService` SQLite lookup specifically, profiling should confirm whether the added complexity is warranted — `tokio-rusqlite` offloads to a thread pool so the call is non-blocking at the tokio level, but the interceptor invocation pattern must match.

### Anti-Pattern 3: Sharing a Single-Port Listener via Multiplex

**What people do:** Use `tonic::transport::Server::into_router()` to merge the tonic router into the axum router for single-port operation.

**Why it's wrong:** `into_router()` is marked experimental in tonic. The body type unification between axum and tonic requires custom wrapper types. The `tonic-web` layer (for browser gRPC) introduces additional body type incompatibilities (tonic issue #1964). Testing becomes harder because a single listener serves both protocols.

**Do this instead:** Use dual listeners with `tokio::try_join!`. The operational overhead of one extra port is negligible. Firewall rules are cleaner. Each server's middleware chain is independent and type-safe.

### Anti-Pattern 4: Putting Auth Keys in Proto Fields

**What people do:** Add an `api_key` field to each proto message so clients pass auth in the request body.

**Why it's wrong:** gRPC has a well-defined metadata mechanism for auth — it maps directly to HTTP/2 headers and is the universally expected location. Tools like `grpcurl`, client libraries, and service meshes all expect `authorization` metadata, not message-level fields. Message-level auth also bypasses the interceptor, making it invisible to middleware.

**Do this instead:** Use `authorization: Bearer mnk_...` in gRPC call metadata. The interceptor extracts it the same way axum middleware extracts HTTP headers.

---

## Integration Points

### New Components vs Modified Components

| Component | New or Modified | Touch Surface |
|-----------|----------------|---------------|
| `proto/mnemonic.proto` | New | Build input only — no existing code modified |
| `build.rs` | New | Invoked by Cargo; generates code into `OUT_DIR` |
| `src/grpc/mod.rs` | New | Imported from `main.rs` only |
| `src/grpc/service.rs` | New | Calls `Arc<MemoryService>` methods |
| `src/grpc/interceptor.rs` | New | Calls `Arc<KeyService>` methods |
| `src/config.rs` | Modified | Add 3 fields: `grpc_port`, `grpc_tls_cert`, `grpc_tls_key` |
| `src/main.rs` | Modified | Replace single `server::serve()` with `tokio::try_join!` |
| `Cargo.toml` | Modified | Add `tonic`, `prost` to `[dependencies]`; add `tonic-build` to `[build-dependencies]` |
| `src/server.rs` | Unchanged | REST server — no modifications needed |
| `src/service.rs` | Unchanged | `MemoryService` — same interface, called by both protocols |
| `src/auth.rs` | Unchanged | `KeyService` — same interface, called by both protocols |
| `src/storage/` | Unchanged | `StorageBackend` trait and implementations — untouched |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|--------------|-------|
| `main.rs` ↔ `grpc::serve()` | Direct function call, passes `GrpcState` by value | `GrpcState` holds `Arc` clones, not moves |
| `grpc::interceptor` ↔ `auth::KeyService` | `Arc<KeyService>` — same instance as REST | Interceptor holds clone of the Arc, not a new KeyService |
| `grpc::service` ↔ `service::MemoryService` | `Arc<MemoryService>` — same instance as REST | Single MemoryService instance serves both protocols |
| `grpc::service` ↔ `auth::AuthContext` | Via `tonic::Request::extensions()` | Interceptor inserts; service methods extract |
| `grpc::mod` ↔ generated proto types | `tonic::include_proto!("mnemonic")` | Generated at build time from `proto/mnemonic.proto` |

---

## Scaling Considerations

| Scale | Architecture Notes |
|-------|-------------------|
| Single agent, SQLite | Both REST and gRPC connect to same `SqliteBackend`. SQLite WAL mode handles concurrent reads from two servers without issue. |
| Multi-agent, Qdrant/Postgres | Both servers share `Arc<dyn StorageBackend>` which is already a pooled connection (Qdrant gRPC channel, sqlx pool). No additional connection overhead from having two servers. |
| High-throughput agent swarms | gRPC's binary framing and multiplexing over HTTP/2 is the point — this is why v1.5 adds gRPC. The dual-port design means gRPC traffic does not share a listener backlog with REST. |

---

## Sources

- [tonic official authentication example (server.rs)](https://github.com/hyperium/tonic/blob/master/examples/src/authentication/server.rs) — bearer token interceptor pattern
- [tonic::service::Interceptor trait docs](https://docs.rs/tonic/latest/tonic/service/trait.Interceptor.html) — interceptor signature, limitations vs Tower middleware
- [tonic-build docs.rs](https://docs.rs/tonic-build/latest/tonic_build/) — build.rs configuration, out_dir, server_only
- [axum rest-grpc-multiplex example fix PR #2825](https://github.com/tokio-rs/axum/pull/2825) — confirms into_router() approach and its limitations
- [axum discussion #2999: integrating axum with tonic](https://github.com/tokio-rs/axum/discussions/2999) — maintainer guidance pointing to PR #2825
- [tonic issue #1964: tonic-web multiplexing body type mismatch](https://github.com/hyperium/tonic/issues/1964) — documents single-port multiplexing limitations
- [tonic-middleware crate](https://github.com/teimuraz/tonic-middleware) — async interceptor alternative
- [fpblock.com: Combining Axum, Hyper, Tonic and Tower Part 4](https://academy.fpblock.com/blog/axum-hyper-tonic-tower-part4/) — HybridService body type unification pattern
- [http-grpc-cohosting (sunsided)](https://github.com/sunsided/http-grpc-cohosting) — cohosting reference implementation

---
*Architecture research for: Mnemonic v1.5 gRPC integration*
*Researched: 2026-03-22*
