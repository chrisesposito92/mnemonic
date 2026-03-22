# Project Research Summary

**Project:** Mnemonic v1.5 — gRPC Interface
**Domain:** Adding a tonic gRPC server to an existing axum REST server (Rust, single-binary)
**Researched:** 2026-03-22
**Confidence:** HIGH (stack, features, architecture); MEDIUM on one critical version conflict flagged below

---

## CRITICAL DECISION POINT: tonic/prost Version Conflict

**This must be resolved before any Cargo.toml changes are made.**

The stack researcher and the pitfalls researcher reached different conclusions on which tonic/prost version to use:

| Researcher | Recommended tonic | Recommended prost | Rationale |
|---|---|---|---|
| STACK.md | `0.14.5` (latest stable, 2026-02-19) | `0.14` | Aligns with axum 0.8, hyper 1, tower 0.5 — exactly what mnemonic already uses |
| PITFALLS.md | `0.12` | `0.13` | qdrant-client 1.x pins `tonic ^0.12.3` and `prost ^0.13.3`; must not produce duplicate crate entries |
| ARCHITECTURE.md | `0.12` | `0.13` | Agrees with pitfalls position |

**The conflict explained:** If qdrant-client still resolves to `tonic ^0.12.x`, then adding `tonic = "0.14"` to `[dependencies]` produces two separate copies of tonic in the build (one for your gRPC server, one transitive from qdrant-client). Cargo permits this as long as no tonic types cross the mnemonic/qdrant-client API boundary — which they currently do not. However, whether this is truly safe requires empirical verification, not theoretical reasoning.

**Required first action in Phase 1:** Run `cargo add tonic --version "0.14" && cargo tree -d | grep -E "tonic|prost"` against the actual project. Zero duplicate tonic or prost entries is the success criterion for tonic 0.14. If duplicates appear at a version boundary that causes link errors, downgrade to `tonic = "0.12"` / `prost = "0.13"` — this is the safe fallback confirmed to satisfy qdrant-client's constraints.

**If tonic 0.14 is safe:** Use it — it is the current stable release and aligns with all existing dependencies.
**If tonic 0.14 causes duplicate crate conflicts:** Use `tonic = "0.12"` / `prost = "0.13"` and note that `prost-types` stays at `"0.13"` (it is already in Cargo.toml as an optional dep for the qdrant backend). The `tonic-prost` and `tonic-prost-build` crates at `0.14` do not exist at version `0.12` — in that case use `tonic-build = "0.12"` (the older codegen crate name).

**Flag for roadmap:** Phase 1 must include `cargo tree -d` as a hard gate before any gRPC handler code is written. The roadmap should not assume either version — leave it as a discovery item for Phase 1 execution.

---

## Executive Summary

Mnemonic v1.5 adds a gRPC interface to an already-complete REST memory server. The project is a focused, well-scoped extension: four hot-path unary RPCs (Store, Search, List, Delete) plus a standard health service, all sharing the existing `Arc<MemoryService>` and `Arc<KeyService>` that already power the REST API. The recommended architecture is dual-port — `tokio::try_join!` across two independent `TcpListener` binds — rather than same-port multiplexing. This avoids body-type incompatibilities between axum and tonic that are documented as unresolved upstream issues (tonic #1964, axum #2825). Both servers share state via `Arc` clones; no business logic is duplicated at the gRPC layer.

The stack additions are minimal: tonic, prost, tonic-prost, tonic-prost-build (build dep only), tonic-health, and optionally tonic-reflection. The only new system dependency is `protoc` (Protocol Buffers compiler), which must be added to the CI workflow in the same commit that introduces `build.rs`. The gRPC module is architecturally a thin adapter: `MnemonicGrpcService` translates proto types to existing service types and delegates entirely to `MemoryService`. Auth requires a Tower `Layer` (not a sync tonic interceptor) because the existing `KeyService` validation path is async. There is also a v1.4 tech debt item that blocks `ListMemories`: the CLI `recall` command bypasses the `StorageBackend` trait and must be fixed before `ListMemories` works across all backends.

The most dangerous pitfalls are the ones that are easy to miss and hard to retrofit: failing to call `enforce_scope()` in each gRPC handler (creates cross-agent data access vulnerability), using a sync tonic interceptor for auth (cannot call async `KeyService` methods — the compiler accepts it but the async calls cannot run), and mismatching tonic/prost versions with qdrant-client (failing build or silent duplicate crates). Proto field optionality must also be finalized before any clients exist — changing `string agent_id` to `optional string agent_id` after clients have been deployed is a wire-breaking change.

---

## Key Findings

### Recommended Stack

The v1.4 stack (axum 0.8, tokio 1, sqlx 0.8, qdrant-client 1) is unchanged. The new additions for v1.5 are tonic, prost, tonic-prost, tonic-prost-build (build dep only), tonic-health, and optionally tonic-reflection. The critical version constraint is qdrant-client's transitive dependency on `tonic ^0.12.3` and `prost ^0.13.3`. See the CRITICAL DECISION POINT above and STACK.md for full version analysis.

`protoc` (system binary) is required by prost-build and is not bundled. It must be installed in CI via `arduino/setup-protoc@v3` or `apt-get install protobuf-compiler`, in the same commit that introduces `build.rs`. Missing this step is a guaranteed CI failure that manifests as a cryptic missing-file error, not a clear protoc-not-found message.

**Core technologies:**
- `tonic` (version TBD — resolve in Phase 1): gRPC server runtime; uses the same hyper/tower/tokio stack already present
- `tonic-prost` / `prost`: protobuf message encoding; version must match tonic
- `tonic-prost-build` (or `tonic-build` at 0.12): build-time codegen from `.proto` files; lives in `[build-dependencies]` only
- `tonic-health`: standard `grpc.health.v1` health service; avoids implementing a custom health RPC
- `tonic-reflection` (P2): enables `grpcurl` service discovery without distributing the `.proto` file

### Expected Features

The v1.5 gRPC interface mirrors the REST hot-path exactly. All five RPCs are table stakes; compaction, key management, and streaming are explicitly out of scope per PROJECT.md. The `ListMemories` RPC is blocked by a v1.4 tech debt item: the CLI `recall` command bypasses the `StorageBackend` trait and accesses SQLite directly. This must be fixed before `ListMemories` can work with Qdrant and Postgres backends. See FEATURES.md for full message shape reference.

**Must have (table stakes):**
- `StoreMemory` unary RPC — core write path; mirrors POST /memories
- `SearchMemories` unary RPC — semantic search; the primary hot-path operation and motivation for gRPC
- `ListMemories` unary RPC — session replay and enumeration (requires StorageBackend routing fix as prerequisite)
- `DeleteMemory` unary RPC — cleanup path; completes CRUD over gRPC
- Standard `grpc.health.v1` health service via `tonic-health` — required by load balancers and orchestrators
- Auth via gRPC `authorization` metadata key — security parity with REST; Bearer token validated by same `KeyService`
- Separate gRPC port (default 50051) configurable via `MNEMONIC_GRPC_PORT` env var
- Shared embedding engine across REST and gRPC servers — do not instantiate a second model (doubles cold start time and memory)
- Canonical gRPC status codes (NOT_FOUND, UNAUTHENTICATED, PERMISSION_DENIED, INVALID_ARGUMENT) — not HTTP codes collapsed to INTERNAL

**Should have (differentiators):**
- `score` float field in `SearchMemoriesResponse` per result — enables client-side quality thresholding
- `agent_id` and `session_id` in proto message bodies (not metadata) — statically typed, consistent with Qdrant/Weaviate conventions
- `google.protobuf.Timestamp` for `created_at` — proto3 best practice; avoids int64 epoch ambiguity
- `tonic-reflection` for `grpcurl` discoverability — reduces developer onboarding friction
- Startup log line confirming gRPC port (matches existing REST startup log pattern)
- `grpc_port` field in `GET /health` JSON response for programmatic discovery

**Defer (v2+):**
- Server-streaming `SearchMemoriesStream` — defer until agents demonstrably need >50 results per call
- Bidirectional streaming / memory subscriptions — requires event infrastructure that does not exist
- gRPC for compaction and key management — admin operations; REST is sufficient; expands proto surface unnecessarily
- Same-port HTTP+gRPC multiplexing — body type incompatibilities are documented as unresolved upstream
- gRPC-Web — targets browsers; REST already covers that use case

### Architecture Approach

The recommended pattern is dual-port with shared service layer: `main.rs` constructs `Arc<MemoryService>`, `Arc<KeyService>`, and `Arc<CompactionService>` once, passes Arc clones to both `server::serve()` (REST) and `grpc::serve()` (gRPC), and runs both concurrently via `tokio::try_join!`. The gRPC module is three new files plus modifications to `main.rs`, `config.rs`, and `Cargo.toml`. Auth is implemented as a Tower `Layer` (async) rather than a sync tonic interceptor. `MnemonicGrpcService` is a pure adapter: proto types in, service types out, delegate to `Arc<MemoryService>`, proto types back. See ARCHITECTURE.md for concrete code patterns and anti-patterns.

**Major components:**
1. `proto/mnemonic.proto` — service definition; unary RPCs only; `optional` on all presence-tracked scalar fields; locked before any handler code
2. `build.rs` — invokes tonic codegen; emits explicit `cargo:rerun-if-changed=proto/mnemonic.proto` to prevent always-dirty builds
3. `src/grpc/mod.rs` — `grpc::serve()` function; binds separate `TcpListener`; wires Tower auth layer and tonic service
4. `src/grpc/service.rs` — `MnemonicGrpcService` struct; thin adapter delegating to `Arc<MemoryService>`; no business logic
5. `src/grpc/interceptor.rs` (Tower Layer) — `GrpcAuthLayer`; async `KeyService` calls; injects `AuthContext` into request extensions; mirrors axum auth middleware exactly
6. `src/grpc/error.rs` — `api_error_to_grpc_status()` conversion; maps `ApiError` variants to canonical gRPC status codes
7. `src/config.rs` — adds `grpc_port: u16` (default 50051), `grpc_tls_cert: Option<String>`, `grpc_tls_key: Option<String>`

### Critical Pitfalls

1. **tonic/prost version conflict with qdrant-client** — qdrant-client 1.x pins `tonic ^0.12.3` and `prost ^0.13.3`. Adding `tonic = "0.14"` may produce two separate tonic crate entries. Run `cargo tree -d` immediately after adding tonic and before writing any code. Zero duplicate entries is the success criterion; downgrade to 0.12 if needed.

2. **Sync tonic interceptor cannot do async auth** — `tonic::service::Interceptor` is sync-only and cannot call `KeyService::count_active_keys().await` or `KeyService::verify().await`. Using `block_on()` inside a tokio runtime panics. Use a Tower `Layer` on `Server::builder()` instead. Never ship a gRPC endpoint without auth when REST auth is active.

3. **protoc not installed in CI** — `prost-build` requires a system `protoc` binary; the failure manifests as a missing generated `.rs` file during `include!`, not a clear "protoc not found" error. Update `release.yml` in the same commit that adds `build.rs`. Install via `arduino/setup-protoc@v3` or `apt-get install protobuf-compiler`.

4. **Always-dirty incremental builds from build.rs** — tonic-build emits an incorrect `rerun-if-changed` path (known issue #2239). Prevention: add `println!("cargo:rerun-if-changed=proto/mnemonic.proto")` with the full relative path explicitly in `build.rs`. Verification: `cargo build && cargo build` — second run must complete under 2 seconds with no build script output.

5. **Missing `enforce_scope()` in gRPC handlers** — the REST server calls `enforce_scope(auth_ctx, agent_id)` in every handler; this is not type-enforced. gRPC handlers written separately are easy to ship without it, creating a cross-agent access vulnerability. Prevention: add an integration test for every gRPC handler that sends a scoped key with a mismatched `agent_id` and asserts `Code::PermissionDenied`.

6. **Proto field optionality is a wire-breaking change** — proto3 scalars default to zero value when absent, not `None`. `agent_id` missing from the wire becomes `""`, not `None`. Use `optional string agent_id` for any field where absence is semantically distinct from empty string. Changing this after clients exist requires a new service version (`MnemonicServiceV2`). Finalize the `.proto` file before any handler code.

---

## Implications for Roadmap

The natural phase structure follows the build dependency chain: proto file and codegen scaffolding must come first (generated types gate all subsequent compilation), the dual-server skeleton validates the concurrency architecture before auth complexity is layered on, auth and handlers complete the functional implementation, and the StorageBackend routing fix for recall is an independent cleanup that can be sequenced anywhere after Phase 1.

### Phase 1: Foundation — Proto File, Build Scaffolding, and Version Resolution
**Rationale:** Nothing gRPC-related compiles until `build.rs` generates Rust types from the `.proto` file. The tonic/prost version conflict with qdrant-client must be resolved here by empirical inspection of `cargo tree -d` — this is a hard gate before a single line of handler code is written. CI must be updated in this same phase or the first CI run will fail. Proto field optionality must be finalized before any clients exist.
**Delivers:** `cargo build` succeeds; generated gRPC types compile; incremental builds are fast (verified by double-build timing); CI passes with protoc installed; tonic/prost version decision documented.
**Addresses:** Proto message design (all five operations), field optionality decisions (locked before any clients), tonic/prost version selection (empirical resolution).
**Avoids:** Version conflict with qdrant-client (Pitfall 4), always-dirty builds (Pitfall 3), CI protoc failure (Pitfall 2), wire-breaking proto changes after clients exist (Pitfall 7).
**Hard gates:** `cargo tree -d` shows zero duplicate tonic/prost; `cargo build && cargo build` completes under 2s on second run; CI green with protoc.

### Phase 2: Dual-Server Skeleton and Auth Layer
**Rationale:** Validate the dual-listener `tokio::try_join!` pattern with the simplest possible gRPC server (health endpoint only) before adding handler complexity. Auth must be implemented before any protected handler — never retrofitted after. The Tower auth layer is more complex than a sync interceptor; implement and test it in isolation before adding four RPC methods on top of it.
**Delivers:** `mnemonic serve` starts both REST (existing port) and gRPC (`:50051`); `grpcurl` health check passes; Tower auth layer validates tokens and rejects invalid ones; open-mode (no keys) passthrough works.
**Uses:** tonic transport, Tower service builder, tonic-health, `Arc<KeyService>`.
**Implements:** `src/grpc/mod.rs` (serve function), `src/grpc/interceptor.rs` (GrpcAuthLayer), `src/config.rs` (grpc_port field).
**Avoids:** Single-port body-type mismatch (Pitfall 1), sync interceptor for async auth (Pitfall 5).
**Hard gates:** Integration test — bad token returns `Code::Unauthenticated`; no-keys open mode passes through; both servers start on confirmed separate ports; process exits immediately if either port fails to bind.

### Phase 3: Core RPC Handlers
**Rationale:** All four hot-path operations are implemented together because they share the same error mapping and scope enforcement pattern. Implementing them together ensures `api_error_to_grpc_status()` and `enforce_scope()` are consistently applied across all handlers from the start, rather than added handler-by-handler with potential omissions.
**Delivers:** Full gRPC hot-path functional: Store, Search, List, Delete all work; `google.protobuf.Timestamp` in responses; `score` field in SearchResponse; integration tests passing including scope enforcement for every handler.
**Implements:** `src/grpc/service.rs`, `src/grpc/error.rs`; shared `enforce_scope()` call pattern.
**Avoids:** Missing scope enforcement (Pitfall 8), incorrect gRPC status codes (Pitfall 6), auth bypass.
**Hard gates:** Integration test per handler — valid token + correct agent passes; scoped key + wrong agent returns `Code::PermissionDenied`; NOT_FOUND on delete of nonexistent ID; `Code::Unauthenticated` not `Code::Internal` for invalid token.

### Phase 4: StorageBackend Routing Fix (v1.4 Tech Debt)
**Rationale:** `ListMemories` is blocked by the CLI `recall` command bypassing the `StorageBackend` trait. This is independent of all gRPC wiring and can be done in parallel with Phase 3 or after it, but must complete before `ListMemories` is considered functional with Qdrant and Postgres backends.
**Delivers:** `mnemonic recall` routes through the `StorageBackend` trait; `ListMemories` gRPC RPC works correctly with SQLite, Qdrant, and Postgres backends.
**Addresses:** v1.4 tech debt that was a prerequisite for ListMemories from the original feature specification.

### Phase 5: Config, Polish, and Optional Discoverability
**Rationale:** Operational completeness. These items are low-risk and mechanical — they do not block the functional hot-path but significantly improve the operator and developer experience. Grouped together because they follow existing patterns with no new architectural decisions.
**Delivers:** `grpc_port` in `mnemonic config show`; `grpc_port` in `GET /health` JSON; startup log confirming gRPC port; optional `tonic-reflection` for grpcurl discoverability; TLS config fields (`grpc_tls_cert`, `grpc_tls_key`) with paired validation in `validate_config()`.
**Avoids:** gRPC port not discoverable (UX pitfall), no startup confirmation log (UX pitfall), TLS cert/key mismatch at startup (config validation gap).

### Phase Ordering Rationale

- Phase 1 must come first: the proto file is a build prerequisite for all generated types; the version conflict is a Cargo constraint that fails the build if unresolved.
- Phase 2 before Phase 3: auth must gate handlers from the moment they exist. Shipping unprotected gRPC handlers even temporarily is a security hole when REST auth is active.
- Phase 3 and Phase 4 can be sequenced in either order or in parallel; Phase 4 touches `src/cli.rs` only and does not intersect with the gRPC module.
- Phase 5 is last: operational polish does not block functional correctness; the config design can reflect what was actually built in Phases 1-4.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1 (version conflict):** Run `cargo add tonic --version "0.14" && cargo tree -d | grep -E "tonic|prost"` against the actual project before settling on the version. This is empirical, not theoretical — the right answer depends on what qdrant-client 1.x actually resolves to in this project's dependency tree at the time of implementation.
- **Phase 2 (Tower auth layer):** The Tower `Layer` pattern for async gRPC auth has less documentation than the sync interceptor path. The architecture researcher's code example uses `block_on` in a sync interceptor; the pitfalls researcher rejects this approach and requires a Tower `Layer`. Validate whether `tokio::task::block_in_place` is safe here (given `tokio-rusqlite` already offloads to a thread pool) before choosing the implementation approach. This decision affects Phase 2 scope.

Phases with standard, well-documented patterns (research-phase can be skipped):
- **Phase 3 (RPC handlers):** The adapter pattern (proto types in, service call, proto types out) is straightforward and demonstrated in official tonic examples. The error mapping and scope enforcement patterns are specified in detail in ARCHITECTURE.md and PITFALLS.md.
- **Phase 4 (StorageBackend routing):** Pure refactor of existing CLI code to route through an existing trait; no new dependencies or APIs.
- **Phase 5 (config/polish):** Mechanical additions following existing patterns in `src/config.rs` and `src/server.rs`.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | MEDIUM | tonic 0.14 verified by stack researcher via crates.io API (2026-02-19 release confirmed). qdrant-client constraint verified by pitfalls researcher against actual crate metadata. The two conflict and must be resolved empirically at the start of Phase 1. All other dependency choices are HIGH confidence. |
| Features | HIGH | Five RPCs are specified in PROJECT.md; message shapes grounded in Qdrant/Weaviate proto conventions and proto3 best practices. Feature set is small and well-bounded. |
| Architecture | HIGH | Dual-port pattern verified against tonic official examples and axum GitHub issues (#2825, #1964). Component boundaries directly derived from v1.4 codebase inspection (10,763 lines across 13 files). |
| Pitfalls | HIGH | Most pitfalls verified against specific GitHub issues with issue numbers (tonic #1964, #2239; axum #2825). Auth sync/async limitation is documented tonic behavior. Version conflict verified against qdrant-client actual Cargo.toml constraints. |

**Overall confidence:** HIGH — with the caveat that the tonic/prost version decision is an empirical open item that must be resolved at the start of Phase 1 execution.

### Gaps to Address

- **tonic version (critical, empirical):** Must run `cargo tree -d` against the actual project to determine whether tonic 0.14 coexists cleanly with qdrant-client 1.x or produces duplicate crate entries requiring a downgrade to 0.12. Cannot be resolved by research alone.
- **Auth implementation approach (Tower Layer vs block_in_place in sync interceptor):** The pitfalls researcher requires a Tower `Layer`; the architecture researcher shows a `block_on` pattern. Validate during Phase 2 whether `tokio::task::block_in_place` is safe given `tokio-rusqlite`'s thread-pool design. If block_in_place is safe, the sync interceptor approach reduces boilerplate significantly. If not, the Tower `Layer` is non-negotiable.
- **StorageBackend `list()` trait method signature:** The research confirms the recall CLI bypasses the trait but does not detail the current `list_memories()` trait method signature or whether `ListRequest`'s `offset` pagination field is already supported. Inspect `src/storage/mod.rs` during Phase 4 planning.

---

## Sources

### Primary (HIGH confidence)
- crates.io API (direct JSON) — tonic 0.14.5 (2026-02-19), tonic-reflection 0.14.5, prost 0.14.3 versions verified
- [tonic v0.14.5 Cargo.toml](https://raw.githubusercontent.com/hyperium/tonic/v0.14.5/tonic/Cargo.toml) — axum 0.8, hyper 1, tower 0.5 compatibility confirmed
- [qdrant-client 1.16+ Cargo.toml](https://docs.rs/crate/qdrant-client/latest) — `tonic ^0.12.3`, `prost ^0.13.3` constraints verified
- [tonic official authentication example](https://github.com/hyperium/tonic/blob/master/examples/src/authentication/server.rs) — bearer token interceptor pattern confirmed
- [tonic::service::Interceptor docs](https://docs.rs/tonic/latest/tonic/service/trait.Interceptor.html) — sync-only limitation confirmed
- [gRPC official docs — metadata, auth, status codes, health checking](https://grpc.io/docs/guides/) — conventions verified
- [Proto3 best practices (protobuf.dev)](https://protobuf.dev/best-practices/dos-donts/) — field optionality, tag stability, Timestamp usage
- Direct codebase inspection — `src/auth.rs`, `src/server.rs`, `src/storage/mod.rs`, `.github/workflows/release.yml`, `Cargo.toml` (v1.4 state)

### Secondary (MEDIUM confidence)
- tonic GitHub issue [#1964](https://github.com/hyperium/tonic/issues/1964) — same-port body type mismatch, open as of September 2025
- axum pull request [#2825](https://github.com/tokio-rs/axum/pull/2825) — gRPC multiplex example fix; confirms dual-port is preferred
- tonic GitHub issue [#2239](https://github.com/hyperium/tonic/issues/2239) — always-dirty builds from incorrect `rerun-if-changed` path emission
- [fpblock.com: Combining Axum, Hyper, Tonic and Tower Part 4](https://academy.fpblock.com/blog/axum-hyper-tonic-tower-part4/) — dual-port `tokio::join!` pattern; HybridService body type complexity analysis
- [Qdrant gRPC API (DeepWiki)](https://deepwiki.com/qdrant/qdrant/9.2-grpc-api-services) — proto message shape conventions
- [Weaviate gRPC API docs](https://docs.weaviate.io/weaviate/api/grpc) — separate port convention (6333 REST / 6334 gRPC)
- [tonic-middleware crate](https://crates.io/crates/tonic-middleware) — async interceptor alternative evaluated and noted

### Tertiary (LOW confidence)
- [Mnemosyne gRPC memory service](https://rand.github.io/mnemosyne/) — reference for MemoryService RPC patterns; documentation sparse
- WebSearch results — dual-port `tokio::join!` community usage; no single canonical code snippet from official tonic docs

---
*Research completed: 2026-03-22*
*Ready for roadmap: yes*
