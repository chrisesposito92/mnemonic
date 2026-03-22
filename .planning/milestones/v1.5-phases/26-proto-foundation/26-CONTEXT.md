# Phase 26: Proto Foundation - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Resolve tonic/prost version conflict, define mnemonic.proto with all five RPC message shapes, wire build.rs codegen with incremental build correctness, and update CI with protoc installation. All gRPC dependencies are behind the `interface-grpc` feature flag — the default binary carries no new dependencies. This phase delivers a verified build pipeline and locked contract, not runtime code.

</domain>

<decisions>
## Implementation Decisions

### Proto package and service naming
- **D-01:** Package `mnemonic.v1`, service `MnemonicService` — follows proto3 convention of `{project}.{version}.{ServiceName}`
- **D-02:** Four RPCs as defined in PROTO-01: `StoreMemory`, `SearchMemories`, `ListMemories`, `DeleteMemory` — all unary, no streaming

### Message field mapping
- **D-03:** Proto messages mirror REST JSON structure with proto-idiomatic naming — same logical fields, proto3 types (`string` for IDs, `repeated string` for tags, `float` for distances, `google.protobuf.Timestamp` for timestamps)
- **D-04:** Request/response messages are RPC-specific (e.g., `StoreMemoryRequest`, `StoreMemoryResponse`) — not shared generic wrappers
- **D-05:** `Memory` message type shared across responses (same shape as REST JSON Memory object: id, content, agent_id, session_id, tags, created_at, embedding_model)

### Proto file placement
- **D-06:** Single file `proto/mnemonic.proto` at project root — standard convention, clear `cargo:rerun-if-changed` path
- **D-07:** build.rs uses `tonic_build::compile_protos("proto/mnemonic.proto")` with explicit `println!("cargo:rerun-if-changed=proto/mnemonic.proto")` to prevent always-dirty builds (per STATE.md critical flag)

### Feature flag design
- **D-08:** All gRPC deps (tonic, prost, tonic-build, tonic-health, tonic-reflection) behind `interface-grpc` feature flag — consistent with existing `backend-qdrant` / `backend-postgres` pattern
- **D-09:** build.rs conditionally compiles proto only when `interface-grpc` feature is active — default builds skip proto codegen entirely
- **D-10:** tonic-build is a build-dependency, not a runtime dependency — goes in `[build-dependencies]` section of Cargo.toml

### Version conflict resolution
- **D-11:** Start with tonic 0.12 / prost 0.13 (conservative approach matching existing prost-types 0.13 in Cargo.toml) — upgrade to 0.14 only if zero conflicts confirmed empirically via `cargo tree -d`
- **D-12:** Zero duplicate tonic/prost entries in `cargo tree -d` is a hard gate — version is documented in a comment in Cargo.toml per success criteria #4

### CI protoc installation
- **D-13:** Use `arduino/setup-protoc@v3` in release.yml — must be added in same commit as build.rs to prevent cryptic missing-file errors (per STATE.md critical flag)
- **D-14:** Protoc step added before `cargo build` for all three matrix targets (linux-x86_64, macos-x86_64, macos-aarch64)

### Claude's Discretion
- Exact proto field ordering and numbering within messages
- Whether to use `optional` wrapper types or plain proto3 defaults for optional fields (session_id, tags)
- Specific tonic-build configuration options (e.g., type_attribute, field_attribute)
- build.rs structure and conditional compilation approach
- Test strategy for verifying generated types compile correctly

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements and success criteria
- `.planning/REQUIREMENTS.md` — PROTO-01 through PROTO-04 define all proto/codegen/CI/feature-gate requirements
- `.planning/ROADMAP.md` Phase 26 section — 5 success criteria including zero-duplicate cargo tree check and incremental build verification

### Critical research flags (MUST READ)
- `.planning/STATE.md` "Accumulated Context > Critical Research Flags" section — Phase 26 hard gates: version conflict resolution, build.rs always-dirty bug prevention, CI protoc installation timing

### Existing feature gate pattern to follow
- `Cargo.toml` lines 12-17 — `backend-qdrant` and `backend-postgres` feature flag definitions (template for `interface-grpc`)
- `src/storage/mod.rs` lines 1-12 — `#[cfg(feature = "...")]` conditional module declarations (pattern for gRPC module gating)

### Existing prost dependency (version anchor)
- `Cargo.toml` line 42 — `prost-types = { version = "0.13", optional = true }` already in tree via qdrant-client; new prost version must be compatible

### CI workflow to modify
- `.github/workflows/release.yml` — Current release workflow without protoc; protoc step must be added before cargo build

### Server architecture (context for later phases)
- `src/server.rs` lines 34-40 — AppState struct and build_router() that Phase 27 will extend with gRPC server
- `src/config.rs` lines 9-30 — Config struct that Phase 27 will extend with grpc_port

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Feature gate pattern** (`Cargo.toml` + `src/storage/mod.rs`): Exact template for `interface-grpc` feature flag — optional deps in Cargo.toml, `#[cfg(feature = "...")]` in source
- **prost-types 0.13** already in dependency tree (via qdrant-client) — anchors prost version choice

### Established Patterns
- **Optional feature deps**: `dep:qdrant-client` and `dep:sqlx` pattern in `[features]` — `interface-grpc` should follow same `dep:tonic`, `dep:prost`, etc. pattern
- **Module organization**: `src/storage/mod.rs` conditionally includes backends — future `src/grpc/` module will follow same pattern
- **No build.rs exists yet**: This phase creates it from scratch — no migration concerns

### Integration Points
- **Cargo.toml `[features]`**: New `interface-grpc` feature flag with dep list
- **Cargo.toml `[dependencies]`**: New optional tonic, prost entries
- **Cargo.toml `[build-dependencies]`**: New tonic-build (conditional on feature)
- **`src/lib.rs`**: Will eventually add `#[cfg(feature = "interface-grpc")] pub mod grpc;` (Phase 27, not this phase)
- **`.github/workflows/release.yml`**: Protoc installation step before cargo build

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. The existing feature gate pattern (backend-qdrant/backend-postgres) is the clear template for interface-grpc. Conservative version choice (tonic 0.12 / prost 0.13) anchored by existing prost-types dependency.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 26-proto-foundation*
*Context gathered: 2026-03-22*
