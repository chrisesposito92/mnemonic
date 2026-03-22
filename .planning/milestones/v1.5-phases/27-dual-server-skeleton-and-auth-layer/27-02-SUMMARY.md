---
phase: 27-dual-server-skeleton-and-auth-layer
plan: 02
subsystem: grpc
tags: [tonic, grpc, tower, auth, jwt, middleware, open-mode]

# Dependency graph
requires:
  - phase: 27-01
    provides: src/grpc/mod.rs with MnemonicGrpcService, serve_grpc(), key_service field
  - phase: 26-proto-foundation
    provides: interface-grpc feature flag, tonic 0.13 dependency
affects:
  - 28-grpc-handlers (handlers extract AuthContext from request extensions)

# Tech tracking
tech-stack:
  added:
    - http = "1" as direct dependency (was transitive via axum; needed explicitly for http::Request in lib code)
    - tower = "0.5" as optional dependency gated by interface-grpc feature (was dev-dep only; needed in lib code for Layer/Service traits)
  patterns:
    - Tower Layer+Service pattern for async gRPC auth (clone+swap in Service::call)
    - GrpcAuthLayer applied via Server::builder().layer() before add_service()
    - Health check bypass via URI path inspection in Tower Layer

key-files:
  created:
    - src/grpc/auth.rs
  modified:
    - src/grpc/mod.rs
    - src/lib.rs
    - Cargo.toml

key-decisions:
  - "http crate added as direct [dependencies] entry — transitive via axum but not usable in lib code without explicit declaration"
  - "tower added as optional dependency in [dependencies] gated by interface-grpc — was dev-dep only; tower::Layer and tower::Service traits caused ambiguity without direct dep"
  - "grpc mod added to lib.rs under cfg(feature = interface-grpc) — required for test discovery via cargo test --lib; main.rs mod grpc is not visible to test runner"
  - "BoxCloneService used in tests to make inner service type concrete — avoids type inference failures with impl Service return types"

patterns-established:
  - "Pattern: Tower Layer receives http::Request<tonic::body::Body> not tonic::Request<T> — use req.headers() not req.metadata()"
  - "Pattern: clone+swap with std::mem::replace before Box::pin async move block"
  - "Pattern: Health check bypass via req.uri().path().starts_with() in Tower Layer call"
  - "Pattern: Status::into_http() to convert gRPC error into HTTP response within Tower Layer"

requirements-completed: [AUTH-01, AUTH-02, AUTH-03]

# Metrics
duration: 5min
completed: 2026-03-22
---

# Phase 27 Plan 02: gRPC Auth Layer Summary

**Async Tower auth middleware for gRPC using GrpcAuthLayer/GrpcAuthService — reuses KeyService for open-mode bypass, bearer token validation, and AuthContext injection into request extensions**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-22T14:03:37Z
- **Completed:** 2026-03-22T14:09:19Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Created src/grpc/auth.rs with GrpcAuthLayer (Tower Layer) and GrpcAuthService (Tower Service) implementing async gRPC authentication
- Implemented all required auth behaviors: open-mode bypass (AUTH-03), health check bypass, valid token injection, invalid/revoked token rejection, malformed header detection
- Wired GrpcAuthLayer into serve_grpc() via Server::builder().layer() in src/grpc/mod.rs
- 6 unit tests covering all auth behaviors — all pass
- Both cargo build and cargo build --features interface-grpc succeed with zero errors
- 91 total lib tests pass (85 pre-existing + 6 new auth tests)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement GrpcAuthLayer and GrpcAuthService in src/grpc/auth.rs** - `78e759e` (feat)
2. **Task 2: Wire GrpcAuthLayer into serve_grpc Server::builder** - `75120c8` (feat)

## Files Created/Modified

- `src/grpc/auth.rs` - GrpcAuthLayer, GrpcAuthService, 6 unit tests
- `src/grpc/mod.rs` - pub mod auth declaration, .layer(auth::GrpcAuthLayer) wired into serve_grpc()
- `src/lib.rs` - added cfg(feature = "interface-grpc") pub mod grpc for test discovery
- `Cargo.toml` - http = "1" as direct dep; tower = "0.5" as optional dep in interface-grpc feature

## Decisions Made

- http crate added as direct [dependencies] entry: transitive via axum but Rust does not allow using transitive crate names directly in lib code without explicit declaration
- tower added as optional [dependencies] entry gated by interface-grpc: was dev-dep only; tower::Layer and tower::Service caused "ambiguous associated type" errors in production builds because dev-deps are not available in library code
- grpc mod added to lib.rs under cfg(feature): cargo test --lib only discovers tests in lib.rs module tree; main.rs `mod grpc` is not visible to the test runner
- BoxCloneService used in tests for concrete inner type: impl Service return from make_auth_service caused type inference failures with .ready() and .call(); boxing the inner service made the full GrpcAuthService<BoxCloneService<...>> type concrete and inferrable

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical dependency] Added http = "1" as direct [dependencies] entry**
- **Found during:** Task 1 (RED phase — compile attempt)
- **Issue:** http crate is a transitive dependency via axum but cannot be referenced as `http::Request` in library source files without an explicit declaration. Error: "unresolved import `http`"
- **Fix:** Added `http = "1"` to `[dependencies]` in Cargo.toml
- **Files modified:** Cargo.toml
- **Verification:** cargo build --features interface-grpc exits 0

**2. [Rule 2 - Missing critical dependency] Added tower = "0.5" as optional dependency gated by interface-grpc**
- **Found during:** Task 2 (build verification)
- **Issue:** tower was only in [dev-dependencies], so tower::Layer and tower::Service traits caused "ambiguous associated type" errors in library code during `cargo build --features interface-grpc` (dev-deps not available in non-test builds)
- **Fix:** Added `tower = { version = "0.5", optional = true }` to `[dependencies]` and `"dep:tower"` to interface-grpc feature list
- **Files modified:** Cargo.toml
- **Verification:** cargo build --features interface-grpc exits 0, cargo build exits 0

**3. [Rule 2 - Missing critical functionality] Added pub mod grpc to lib.rs under cfg(feature)**
- **Found during:** Task 1 (RED phase — test discovery)
- **Issue:** `cargo test --features interface-grpc grpc::auth::tests --lib` returned 0 tests because the grpc module is declared only in main.rs, which is not visible to the test runner (lib.rs is the test root)
- **Fix:** Added `#[cfg(feature = "interface-grpc")] pub mod grpc;` to src/lib.rs
- **Files modified:** src/lib.rs
- **Verification:** cargo test --features interface-grpc grpc::auth::tests --lib finds and runs 6 tests

**4. [Rule 1 - Bug] Replaced impl Service return type in tests with BoxCloneService**
- **Found during:** Task 1 (RED phase — compile attempt)
- **Issue:** `make_auth_service` returning `impl tower::Service<...>` caused "cannot infer type" errors on `.ready()` and `.oneshot()` calls
- **Fix:** Changed make_auth_service to return concrete `GrpcAuthService<BoxCloneService<...>>` type alias; used `tower::ServiceExt::oneshot()` instead of `.ready().await + .call()`
- **Files modified:** src/grpc/auth.rs (test module)
- **Verification:** All 6 tests compile and pass

---

**Total deviations:** 4 auto-fixed (Rules 1-2)
**Impact on plan:** All fixes are correctness requirements — http and tower deps required for compilation, lib.rs mod required for test discovery, concrete types required for test inference. No scope creep.

## Known Stubs

The following stubs from Plan 01 remain intentional (unchanged in Plan 02):

- `src/grpc/mod.rs:34-37` — store_memory returns Status::unimplemented (Phase 28)
- `src/grpc/mod.rs:40-43` — search_memories returns Status::unimplemented (Phase 28)
- `src/grpc/mod.rs:46-49` — list_memories returns Status::unimplemented (Phase 28)
- `src/grpc/mod.rs:52-55` — delete_memory returns Status::unimplemented (Phase 28)

## Next Phase Readiness

- Phase 28 handlers can extract AuthContext from request extensions: `request.extensions().get::<AuthContext>()`
- GrpcAuthLayer is live on all gRPC services — auth enforcement is in place
- Both REST and gRPC auth share identical KeyService logic — no behavioral divergence
- 91 tests pass, zero regressions

## Self-Check: PASSED

- src/grpc/auth.rs: FOUND
- src/grpc/mod.rs (contains GrpcAuthLayer): FOUND
- Commit 78e759e (feat: GrpcAuthLayer): FOUND
- Commit 75120c8 (feat: wire auth layer): FOUND
- All 91 lib tests: PASSING
- cargo build: PASSES
- cargo build --features interface-grpc: PASSES

---
*Phase: 27-dual-server-skeleton-and-auth-layer*
*Completed: 2026-03-22*
