---
phase: 27-dual-server-skeleton-and-auth-layer
plan: 01
subsystem: grpc
tags: [tonic, grpc, health, config, dual-port, tokio]

# Dependency graph
requires:
  - phase: 26-proto-foundation
    provides: proto/mnemonic.proto, build.rs with tonic-build codegen, interface-grpc feature flag
provides:
  - grpc_port: u16 field in Config struct with default 50051 and MNEMONIC_GRPC_PORT env var support
  - src/grpc/mod.rs with MnemonicGrpcService struct, include_proto!, 4 unimplemented RPC stubs, serve_grpc() with tonic-health
  - dual-port startup in main.rs via tokio::try_join! (cfg-gated, falls back to REST-only without feature)
  - config show CLI displays grpc_port in both JSON and human-readable output
affects:
  - 27-02-PLAN (auth layer adds Tower layer to serve_grpc call and uses MnemonicGrpcService)
  - 28-grpc-handlers (handlers implement RPCs using MnemonicGrpcService fields)

# Tech tracking
tech-stack:
  added: [prost-types added to interface-grpc feature (required by generated Timestamp type in mnemonic.v1.rs)]
  patterns: [tonic::include_proto! for proto generated types, tonic-health health_reporter pattern, tokio::try_join! dual-server pattern]

key-files:
  created:
    - src/grpc/mod.rs
  modified:
    - src/config.rs
    - src/cli.rs
    - src/main.rs
    - Cargo.toml

key-decisions:
  - "prost-types added to interface-grpc feature — generated mnemonic.v1.rs uses prost_types::Timestamp for created_at field; omitting it causes E0433 compile error"
  - "health_reporter does not need mut — tonic-health 0.13.1 set_serving() takes &self not &mut self"
  - "Arc instances cloned for AppState, originals moved to MnemonicGrpcService — both share the same underlying data"

patterns-established:
  - "Pattern: cfg(feature) block in main.rs for dual-port startup — interface-grpc adds try_join!, otherwise falls through to REST-only"
  - "Pattern: MnemonicGrpcService holds same Arc<T> instances as AppState without inner data duplication"
  - "Pattern: serve_grpc() returns anyhow::Result<()> to match server::serve() return type for try_join! compatibility"

requirements-completed: [SERVER-01, SERVER-02, SERVER-03]

# Metrics
duration: 18min
completed: 2026-03-22
---

# Phase 27 Plan 01: Dual-Server Skeleton Summary

**tonic gRPC skeleton with health checking, grpc_port config field, and dual-port tokio::try_join! startup alongside existing REST server**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-22T17:00:00Z
- **Completed:** 2026-03-22T17:18:00Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added grpc_port: u16 to Config struct with default 50051, MNEMONIC_GRPC_PORT env var support via Figment auto-mapping, and config show CLI display in both JSON and human-readable modes
- Created src/grpc/mod.rs with MnemonicGrpcService struct holding shared Arc instances, 4 unimplemented RPC stubs (store_memory, search_memories, list_memories, delete_memory), and serve_grpc() with tonic-health SERVING status
- Wired dual-port startup in main.rs with tokio::try_join! (cfg-gated on interface-grpc feature), preserving REST-only fallback when feature is disabled

## Task Commits

Each task was committed atomically:

1. **Task 1: Add grpc_port to Config and update config show** - `1405411` (feat)
2. **Task 2: Create gRPC module skeleton with service struct and serve_grpc** - `fb2bc12` (feat)
3. **Task 3: Wire dual-port startup in main.rs with tokio::try_join!** - `58bad0f` (feat)

## Files Created/Modified

- `src/grpc/mod.rs` - gRPC service struct, include_proto!, 4 unimplemented handlers, serve_grpc() with tonic-health
- `src/config.rs` - grpc_port: u16 field with default 50051, test_grpc_port_env_override test, updated test_config_defaults
- `src/cli.rs` - grpc_port added to config show JSON and human-readable output
- `src/main.rs` - cfg-gated mod grpc declaration and dual-port startup via tokio::try_join!
- `Cargo.toml` - prost-types added to interface-grpc feature list

## Decisions Made

- prost-types added to interface-grpc feature: generated mnemonic.v1.rs uses prost_types::Timestamp for created_at field; this was not anticipated in the plan but required for compilation (Rule 1 auto-fix)
- health_reporter does not need mut: tonic-health 0.13.1 set_serving() takes &self; the plan template showed `mut` but the compiler confirmed it is unnecessary
- Arc cloning pattern: AppState gets clones of service/compaction/key_service Arcs, MnemonicGrpcService receives the originals; both hold references to the same underlying data

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Added dep:prost-types to interface-grpc feature in Cargo.toml**
- **Found during:** Task 2 (Create gRPC module skeleton with service struct and serve_grpc)
- **Issue:** Generated mnemonic.v1.rs references prost_types::Timestamp for the Memory.created_at field. The interface-grpc feature did not include prost-types, causing E0433 compile error "could not find `prost_types` in the list of imported crates"
- **Fix:** Added "dep:prost-types" to the interface-grpc feature list in Cargo.toml
- **Files modified:** Cargo.toml
- **Verification:** cargo check --features interface-grpc exits 0
- **Committed in:** fb2bc12 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Required for compilation with interface-grpc feature. prost-types was already a dependency (used by backend-qdrant) — only the feature gate needed updating. No scope creep.

## Issues Encountered

- tonic-health's health_reporter() returns a reporter that uses &self for set_serving() in version 0.13.1, contrary to the plan's template showing `mut health_reporter`. The unused_mut warning was removed by dropping the mut keyword.

## Known Stubs

The following stubs are intentional — Plan 02 (auth layer) and Phase 28 (handlers) will resolve them:

- `src/grpc/mod.rs:34-37` — store_memory returns Status::unimplemented (Phase 28)
- `src/grpc/mod.rs:40-43` — search_memories returns Status::unimplemented (Phase 28)
- `src/grpc/mod.rs:46-49` — list_memories returns Status::unimplemented (Phase 28)
- `src/grpc/mod.rs:52-55` — delete_memory returns Status::unimplemented (Phase 28)

These stubs are the intended state for this plan — the goal was to establish the skeleton, not implement the handlers.

## Next Phase Readiness

- Plan 02 (auth layer) can add `.layer(GrpcAuthLayer { key_service: ... })` to the Server::builder chain in serve_grpc()
- MnemonicGrpcService.key_service field is available for the auth layer
- Both `cargo build` and `cargo build --features interface-grpc` are clean (warnings only, no errors)
- 85 lib tests pass, 0 regressions

## Self-Check: PASSED

- src/grpc/mod.rs: FOUND
- src/config.rs: FOUND
- src/cli.rs: FOUND
- src/main.rs: FOUND
- Commit 1405411 (feat: grpc_port config): FOUND
- Commit fb2bc12 (feat: gRPC module skeleton): FOUND
- Commit 58bad0f (feat: dual-port startup): FOUND
- All 85 lib tests: PASSING

---
*Phase: 27-dual-server-skeleton-and-auth-layer*
*Completed: 2026-03-22*
