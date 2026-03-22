# Phase 27: Dual-Server Skeleton and Auth Layer - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Start REST+gRPC dual-port via tokio::try_join!, implement an async Tower auth layer for gRPC that reuses existing KeyService, add grpc_port configuration, and share AppState across both servers. This phase delivers the server skeleton and auth — RPC handler implementations are Phase 28.

</domain>

<decisions>
## Implementation Decisions

### gRPC module structure
- **D-01:** New `src/grpc/` directory with `mod.rs` as entry point, gated behind `#[cfg(feature = "interface-grpc")]` in main.rs
- **D-02:** `src/grpc/mod.rs` contains the tonic service struct, `include_proto!` for generated types, and the `serve_grpc()` function
- **D-03:** Phase 28 will add `src/grpc/handlers.rs` for RPC implementations — Phase 27 only needs the skeleton with unimplemented handlers that return `Status::Unimplemented`

### Dual-port startup
- **D-04:** `tokio::try_join!` across two independent `TcpListener` binds — NOT same-port multiplexing (documented body-type mismatch bugs: tonic #1964, axum #2825)
- **D-05:** REST server starts unconditionally on existing `config.port` (default 8080). gRPC server starts alongside it on `config.grpc_port` (default 50051)
- **D-06:** If either server fails to bind, both shut down (try_join! semantics — first error propagates)
- **D-07:** Startup log prints both addresses: `"REST listening on 0.0.0.0:{port}, gRPC listening on 0.0.0.0:{grpc_port}"`

### grpc_port configuration
- **D-08:** New field `grpc_port: u16` in Config struct, default 50051 (gRPC convention)
- **D-09:** Configurable via `MNEMONIC_GRPC_PORT` env var or `grpc_port` in TOML (same precedence as existing config fields)
- **D-10:** `config show` CLI includes grpc_port in output (not a secret — no redaction needed)

### Shared state between REST and gRPC
- **D-11:** gRPC service struct holds `Arc<MemoryService>`, `Arc<KeyService>`, `Arc<CompactionService>`, and `backend_name: String` — same Arc instances as REST AppState
- **D-12:** Both servers constructed from the same Arc instances in main.rs — no cloning of inner data, only Arc reference counting
- **D-13:** The tonic service struct is separate from axum's AppState — they share the same underlying Arc'd services but are different wrapper types (axum needs State<AppState>, tonic needs the service impl)

### Tower auth layer for gRPC
- **D-14:** Implement as a Tower Layer/Service that wraps the gRPC service — NOT a tonic Interceptor (interceptors are sync, KeyService.validate() is async)
- **D-15:** Extract bearer token from gRPC `authorization` metadata key (same "Bearer <token>" format as REST Authorization header)
- **D-16:** Reuse `KeyService.validate()` for token validation and `KeyService.count_active_keys()` for open-mode check — same code path as REST auth middleware
- **D-17:** On auth success, inject `AuthContext` into tonic `Request::extensions()` so handlers can extract it (mirrors REST pattern of injecting into axum request extensions)
- **D-18:** Auth error mapping: missing token when auth active → `Status::Unauthenticated`, invalid/revoked token → `Status::Unauthenticated`, malformed header → `Status::InvalidArgument`
- **D-19:** Open mode (zero active keys) bypasses auth entirely — request passes through with no AuthContext in extensions (consistent with REST behavior)

### Feature gating
- **D-20:** All gRPC code is behind `#[cfg(feature = "interface-grpc")]` — default binary has zero gRPC code or dependencies
- **D-21:** When built without `interface-grpc`, `mnemonic serve` starts only the REST server (current behavior preserved exactly)
- **D-22:** The `grpc_port` config field exists unconditionally in Config struct (simpler than conditional compilation on config) but is only used when the feature is enabled

### Claude's Discretion
- Exact Tower Layer boilerplate structure (Pin<Box<dyn Future>> vs async-trait approach)
- Whether to use a separate auth module file within src/grpc/ or inline the layer in mod.rs
- Test strategy for auth layer (integration vs unit)
- Exact error message wording for gRPC auth failures

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. The existing REST auth middleware in `src/auth.rs` is the clear template for gRPC auth behavior. The dual-port pattern follows the STATE.md critical research flag (tokio::try_join!, not same-port multiplexing).

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements and success criteria
- `.planning/REQUIREMENTS.md` — SERVER-01, SERVER-02, SERVER-03, AUTH-01, AUTH-02, AUTH-03 define all dual-server and auth requirements
- `.planning/ROADMAP.md` Phase 27 section — success criteria including dual-port startup, grpcurl health check, and auth layer validation

### Critical research flags (MUST READ)
- `.planning/STATE.md` "Accumulated Context > Critical Research Flags" — Phase 27 flags: async Tower Layer (not sync interceptor), tokio::try_join! (not same-port multiplexing)
- `.planning/STATE.md` "Accumulated Context > v1.5 open decisions" — Tower auth layer is the resolved approach

### Prior phase context
- `.planning/milestones/v1.5-phases/26-proto-foundation/26-CONTEXT.md` — Proto service definition decisions, version choices, feature gate pattern
- `.planning/milestones/v1.5-phases/26-proto-foundation/26-01-SUMMARY.md` — Build pipeline outcomes, tonic-build deviation (non-optional in build-deps)

### Existing code to extend
- `src/main.rs` lines 240-248 — Current serve path: builds AppState, calls `server::serve()`. Phase 27 adds gRPC server alongside
- `src/server.rs` lines 33-40 — AppState struct (gRPC service struct mirrors these fields)
- `src/server.rs` lines 349-359 — `serve()` function (must become dual-server with try_join!)
- `src/config.rs` lines 8-30 — Config struct (add grpc_port field)
- `src/config.rs` lines 32-49 — Config defaults (add grpc_port: 50051)

### Auth pattern to replicate for gRPC
- `src/auth.rs` lines 248-313 — REST auth_middleware (open-mode check, header extraction, token validation via KeyService)
- `src/auth.rs` lines 30-35 — AuthContext struct (injected into extensions)
- `src/auth.rs` lines 196-236 — KeyService.validate() (reuse directly for gRPC auth)
- `src/server.rs` lines 78-95 — enforce_scope() (Phase 28 will reuse for gRPC handlers)

### Proto definition (locked in Phase 26)
- `proto/mnemonic.proto` — MnemonicService with 4 RPCs, all message types

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **KeyService** (`src/auth.rs`): `validate()` and `count_active_keys()` are the core auth logic — gRPC Tower Layer calls these directly, no duplication
- **AuthContext** (`src/auth.rs:30-35`): Same struct injected into both REST and gRPC request extensions
- **enforce_scope()** (`src/server.rs:78-95`): Scope enforcement logic ready for Phase 28 gRPC handlers
- **AppState pattern** (`src/server.rs:34-40`): Template for gRPC service struct fields

### Established Patterns
- **Feature gating**: `#[cfg(feature = "backend-qdrant")]` pattern in `src/storage/mod.rs` and Cargo.toml — replicate for `interface-grpc`
- **Config extension**: Config struct extended with new fields (storage_provider, qdrant_url, postgres_url in v1.4) — same pattern for grpc_port
- **Figment config loading**: Env vars prefixed with `MNEMONIC_` auto-map to Config fields — `MNEMONIC_GRPC_PORT` works automatically

### Integration Points
- **main.rs serve path** (line 241-247): Replace `server::serve(&config, state).await` with dual-server try_join!
- **Cargo.toml features**: `interface-grpc` feature already defined with tonic/prost deps — add tonic to runtime deps (it's already there as optional)
- **main.rs module declarations**: Add `#[cfg(feature = "interface-grpc")] mod grpc;`

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 27-dual-server-skeleton-and-auth-layer*
*Context gathered: 2026-03-22*
