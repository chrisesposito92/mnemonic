---
phase: 26-proto-foundation
plan: 01
subsystem: infra
tags: [grpc, protobuf, tonic, prost, tonic-build, cargo-features, build-script]

# Dependency graph
requires:
  - phase: 25-storage-pluggable
    provides: "feature-gated backend pattern (backend-qdrant, backend-postgres) used as template"
provides:
  - "proto/mnemonic.proto — locked gRPC contract for MnemonicService with 4 RPCs"
  - "build.rs — conditional tonic-build codegen gated on CARGO_FEATURE_INTERFACE_GRPC"
  - "interface-grpc feature flag in Cargo.toml with tonic 0.13 / prost 0.13 dependencies"
affects: [27-grpc-server, 28-grpc-handlers, 29-grpc-integration]

# Tech tracking
tech-stack:
  added:
    - "tonic 0.13.1 — gRPC runtime (optional, interface-grpc feature)"
    - "prost 0.13 — Protobuf serialization (optional, interface-grpc feature)"
    - "tonic-health 0.13.1 — health service (optional, interface-grpc feature)"
    - "tonic-reflection 0.13.1 — gRPC reflection (optional, interface-grpc feature)"
    - "tonic-build 0.13.1 — proto codegen (always in build-dependencies, runtime-gated via env var)"
  patterns:
    - "CARGO_FEATURE_ env var check in build.rs for conditional build-time codegen"
    - "Full relative path in compile_protos() to prevent always-dirty incremental build (#2239)"
    - "dep: prefix syntax for optional runtime dependencies, non-optional build dependency"

key-files:
  created:
    - "proto/mnemonic.proto"
    - "build.rs"
  modified:
    - "Cargo.toml"

key-decisions:
  - "tonic 0.13.1 / prost 0.13 chosen — compatible with qdrant-client's prost ^0.13.3 anchor; tonic 0.14 would cause prost version conflict"
  - "tonic-build is non-optional in [build-dependencies] because build scripts always compile — CARGO_FEATURE_ env var provides the runtime gate"
  - "Two tonic versions in tree (0.12.3 from qdrant-client, 0.13.1 ours) are acceptable — both share prost 0.13.5 with zero type incompatibilities"
  - "Proto3 defaults for optional fields (session_id, tags) instead of optional wrappers — avoids prost::Option<String> complexity in generated Rust"

patterns-established:
  - "Pattern: Check CARGO_FEATURE_INTERFACE_GRPC (not cfg!) in build.rs to gate feature-conditional codegen"
  - "Pattern: Emit cargo:rerun-if-changed before compile_protos to prevent always-dirty bug"
  - "Pattern: Use dep: prefix for runtime optional gRPC deps; tonic-build always-present as build dep"

requirements-completed: [PROTO-01, PROTO-02, PROTO-04]

# Metrics
duration: 6min
completed: 2026-03-22
---

# Phase 26 Plan 01: Proto Foundation Summary

**MnemonicService proto contract locked with tonic 0.13 / prost 0.13, feature-gated build pipeline verified — default binary unchanged, feature build generates Rust types in 10s, incremental build clean at 0.15s**

## Performance

- **Duration:** 6 minutes
- **Started:** 2026-03-22T13:16:39Z
- **Completed:** 2026-03-22T13:22:00Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Created proto/mnemonic.proto with MnemonicService (4 RPCs), shared Memory type, and all request/response message pairs using proto3 defaults
- Wired conditional tonic-build codegen in build.rs that exits early when interface-grpc feature is absent, preventing any gRPC overhead in the default build
- Added interface-grpc feature flag to Cargo.toml with tonic 0.13 / prost 0.13 pinned to match qdrant-client's prost ^0.13.3 anchor
- Verified full pipeline: default build succeeds, feature build generates types, incremental build clean (0.15s second run)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add interface-grpc feature flag and gRPC dependencies to Cargo.toml** - `51f1d35` (feat)
2. **Task 2: Create proto/mnemonic.proto with MnemonicService and all message types** - `434a529` (feat)
3. **Task 3: Create build.rs with conditional tonic-build codegen and verify full pipeline** - `8af2752` (feat)

## Files Created/Modified

- `proto/mnemonic.proto` — gRPC service definition: MnemonicService with StoreMemory, SearchMemories, ListMemories, DeleteMemory RPCs; Memory shared type; all request/response pairs
- `build.rs` — Conditional proto codegen: CARGO_FEATURE_INTERFACE_GRPC env var check, explicit rerun-if-changed directives, tonic_build::compile_protos("proto/mnemonic.proto")
- `Cargo.toml` — interface-grpc feature flag, optional tonic/prost/tonic-health/tonic-reflection runtime deps, always-present tonic-build build dep

## Decisions Made

- **tonic 0.13.1 over 0.12.x or 0.14.x:** tonic 0.14 requires prost 0.14 which conflicts with qdrant-client's prost ^0.13.3 constraint. tonic 0.13.1 is the newest version compatible with our existing prost 0.13 anchor.
- **tonic-build as non-optional build dep:** The plan specified `optional = true` for tonic-build in [build-dependencies], but build scripts always compile regardless of features. Making it optional causes unresolved symbol errors when the feature is off. The CARGO_FEATURE_INTERFACE_GRPC check in main() provides the runtime gate.
- **Two tonic versions are acceptable:** qdrant-client uses tonic 0.12.3 internally; our gRPC server uses tonic 0.13.1. Both use prost 0.13.5 — zero prost version conflicts, no type incompatibilities.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] tonic-build must be non-optional in [build-dependencies]**

- **Found during:** Task 3 (build.rs creation and pipeline verification)
- **Issue:** Plan specified `tonic-build = { version = "0.13", optional = true }` in [build-dependencies] and `dep:tonic-build` in the interface-grpc feature. But build scripts always compile as standalone binaries — when `optional = true` and the feature is off, tonic-build is not linked, causing `error[E0433]: failed to resolve: use of unresolved module or unlinked crate tonic_build` during `cargo build` (default, no features).
- **Fix:** Changed tonic-build to `tonic-build = "0.13"` (non-optional) in [build-dependencies]. Removed `dep:tonic-build` from the interface-grpc feature list. The CARGO_FEATURE_INTERFACE_GRPC env var check in build.rs main() provides the runtime gate — tonic_build::compile_protos is only called when the feature is active.
- **Files modified:** Cargo.toml, build.rs (no change to build.rs logic, only Cargo.toml)
- **Verification:** `cargo build` (no features) exits 0; `cargo build --features interface-grpc` exits 0
- **Committed in:** 8af2752 (Task 3 commit)

**2. [Rule 1 - Expected Behavior] Two tonic versions appear in cargo tree -d**

- **Found during:** Task 1 verification (cargo tree --features interface-grpc,backend-qdrant -d)
- **Issue:** Plan expected zero tonic/prost duplicates in cargo tree -d. The actual output shows tonic v0.12.3 (from qdrant-client's internal dependency) AND tonic v0.13.1 (our explicit dep) — two tonic versions. The research incorrectly predicted zero tonic duplicates.
- **Fix:** No fix needed — this is expected behavior. qdrant-client v1.x uses tonic 0.12 internally for its own gRPC communication with Qdrant server. Our gRPC server uses tonic 0.13. Both tonic versions share prost 0.13.5 with zero type incompatibilities. The builds succeed, generated types are correct, and no compilation errors occur.
- **Impact:** The literal `cargo tree -d | grep -E "tonic|prost"` check produces non-zero output, but the critical invariant (zero prost version conflicts) holds: only prost 0.13.5 appears in the tree.
- **Committed in:** N/A — documented, no code change needed

---

**Total deviations:** 2 (1 auto-fixed bug, 1 expected behavior clarification)
**Impact on plan:** Auto-fix corrects a logical error in the plan's build-dependency specification. The tonic version observation clarifies research expectations vs. reality — both are benign. Full plan goals achieved.

## Issues Encountered

- protoc not installed locally — installed via `brew install protobuf` (libprotoc 34.1) before running the Task 3 pipeline verification. CI will need the `arduino/setup-protoc@v3` step from Plan 02 (already completed in parallel).

## Known Stubs

None — this plan delivers build infrastructure (proto file, Cargo.toml, build.rs). No UI or data-rendering code was added.

## Next Phase Readiness

- proto/mnemonic.proto is the locked gRPC contract — Phase 27 will use `tonic::include_proto!("mnemonic.v1")` to include generated types
- build.rs correctly gates codegen on the interface-grpc feature — no changes needed for Phase 27
- interface-grpc feature flag is defined — Phase 27 adds the gRPC server behind `#[cfg(feature = "interface-grpc")]`
- CI protoc step is handled in Plan 02 (parallel execution completed)

## Self-Check: PASSED

- proto/mnemonic.proto: FOUND
- build.rs: FOUND
- 26-01-SUMMARY.md: FOUND
- Commit 51f1d35 (Task 1): FOUND
- Commit 434a529 (Task 2): FOUND
- Commit 8af2752 (Task 3): FOUND

---
*Phase: 26-proto-foundation*
*Completed: 2026-03-22*
