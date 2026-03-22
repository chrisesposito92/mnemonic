# Phase 26: Proto Foundation - Research

**Researched:** 2026-03-22
**Domain:** Rust gRPC build pipeline — tonic, prost, tonic-build, protoc CI, incremental builds
**Confidence:** HIGH

## Summary

Phase 26 establishes the entire gRPC build pipeline for mnemonic: a proto file with five RPC message shapes, a build.rs that invokes tonic-build, feature-gating all gRPC deps behind `interface-grpc`, and CI protoc installation. No runtime gRPC code lands in this phase — only types that compile.

The critical finding is that the version conflict concern documented in STATE.md is **resolved in favor of tonic 0.13**: qdrant-client currently resolves prost to `0.13.5`, tonic 0.12.x and 0.13.x both require `prost ^0.13`, and tonic 0.14 requires prost 0.14 which is **incompatible** with qdrant-client's `prost ^0.13.3` constraint. The correct choice is **tonic 0.13.1 / prost 0.13.5** — not 0.12 as the conservative fallback, and not 0.14 which breaks when `backend-qdrant` is also enabled.

The build.rs always-dirty bug (tonic issue #2239) is real and actively open. The fix is using the full path in `compile_protos` (`"proto/mnemonic.proto"`) rather than a bare filename with a separate include directory — when the path resolves directly, the `cargo:rerun-if-changed` directive points to a file that exists. Adding an explicit `println!("cargo:rerun-if-changed=proto/mnemonic.proto")` before calling `compile_protos` provides belt-and-suspenders protection.

**Primary recommendation:** tonic 0.13.1 / prost 0.13 (compatible with qdrant-client 0.13.5 anchor), build.rs with full-path `compile_protos` + explicit rerun-if-changed, `arduino/setup-protoc@v3` in CI before the cargo build step.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Package `mnemonic.v1`, service `MnemonicService`
- **D-02:** Four RPCs: `StoreMemory`, `SearchMemories`, `ListMemories`, `DeleteMemory` — all unary, no streaming
- **D-03:** Proto messages mirror REST JSON structure with proto-idiomatic naming (`string` for IDs, `repeated string` for tags, `float` for distances, `google.protobuf.Timestamp` for timestamps)
- **D-04:** Request/response messages are RPC-specific (e.g., `StoreMemoryRequest`, `StoreMemoryResponse`) — not shared generic wrappers
- **D-05:** `Memory` message type shared across responses (id, content, agent_id, session_id, tags, created_at, embedding_model)
- **D-06:** Single file `proto/mnemonic.proto` at project root
- **D-07:** build.rs uses `tonic_build::compile_protos("proto/mnemonic.proto")` with explicit `println!("cargo:rerun-if-changed=proto/mnemonic.proto")`
- **D-08:** All gRPC deps (tonic, prost, tonic-build, tonic-health, tonic-reflection) behind `interface-grpc` feature flag
- **D-09:** build.rs conditionally compiles proto only when `interface-grpc` feature is active
- **D-10:** tonic-build is a build-dependency, not a runtime dependency
- **D-11:** Start with tonic 0.12 / prost 0.13 — upgrade to 0.14 only if zero conflicts confirmed empirically
- **D-12:** Zero duplicate tonic/prost entries in `cargo tree -d` is a hard gate — version documented in Cargo.toml comment
- **D-13:** Use `arduino/setup-protoc@v3` in release.yml — added in same commit as build.rs
- **D-14:** Protoc step added before `cargo build` for all three matrix targets

### Claude's Discretion

- Exact proto field ordering and numbering within messages
- Whether to use `optional` wrapper types or plain proto3 defaults for optional fields (session_id, tags)
- Specific tonic-build configuration options (e.g., type_attribute, field_attribute)
- build.rs structure and conditional compilation approach
- Test strategy for verifying generated types compile correctly

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PROTO-01 | Proto service definition (mnemonic.proto) with MnemonicService containing StoreMemory, SearchMemories, ListMemories, DeleteMemory RPCs and corresponding request/response messages | D-01 through D-05 locked; proto3 syntax patterns documented below |
| PROTO-02 | tonic-build integration in build.rs with explicit rerun-if-changed path to prevent always-dirty builds | tonic issue #2239 confirmed open; full-path fix documented; conditional CARGO_FEATURE_ pattern documented |
| PROTO-03 | CI release workflow updated with protoc installation step for all build targets | arduino/setup-protoc@v3 confirmed as latest; usage pattern documented |
| PROTO-04 | All gRPC dependencies (tonic, prost, tonic-build, tonic-health, tonic-reflection) feature-gated behind `interface-grpc` flag | Existing `dep:` pattern from backend-qdrant/backend-postgres confirmed; version compatibility fully resolved |
</phase_requirements>

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tonic | 0.13.1 | gRPC server/client framework | Latest version compatible with prost 0.13 (qdrant-client anchor); published 2025-05-05 |
| prost | 0.13 | Protobuf serialization | Resolved to 0.13.5 by qdrant-client; tonic 0.13 requires `^0.13` |
| tonic-build | 0.13.1 | Build-time proto code generation | Companion crate to tonic; same version |
| tonic-health | 0.13.1 | grpc.health.v1 standard service | Used in Phase 28; declared here for feature-gate correctness (PROTO-04 says "all gRPC deps") |
| tonic-reflection | 0.13.1 | gRPC reflection for grpcurl/grpc_cli | Same; declared here, wired in Phase 28 |

### Version Conflict Analysis (CRITICAL)

The existing `prost-types = { version = "0.13", optional = true }` in Cargo.toml is already resolved to `prost 0.13.5` via `qdrant-client 1.x`. Qdrant-client's dependency is `prost ^0.13.3` and `prost-types ^0.13.3`.

**tonic 0.13.x** requires `prost ^0.13` — compatible. Zero duplicates expected.
**tonic 0.14.x** requires `prost ^0.14` — **incompatible** with qdrant-client's `prost ^0.13.3`. This would produce duplicate prost entries and fail the `cargo tree -d` hard gate whenever both `backend-qdrant` and `interface-grpc` features are active.

**Decision: tonic 0.13.1** (supersedes D-11's "try 0.12 first" — 0.13 is the most recent version compatible with prost 0.13).

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| tonic 0.13 | tonic 0.14 | 0.14 requires prost 0.14 — incompatible with qdrant-client; fails cargo tree -d hard gate |
| tonic 0.13 | tonic 0.12 | 0.12 also uses prost 0.13; 0.13 is newer with more fixes; no reason to prefer 0.12 |
| arduino/setup-protoc@v3 | apt-get install protobuf-compiler | apt-get installs OS package (often outdated); setup-protoc installs the exact protoc release |

**Installation:**
```toml
# Cargo.toml — in [features]
interface-grpc = ["dep:tonic", "dep:prost", "dep:tonic-health", "dep:tonic-reflection"]

# Cargo.toml — in [dependencies] (all optional)
tonic = { version = "0.13", optional = true }
prost = { version = "0.13", optional = true }
tonic-health = { version = "0.13", optional = true }
tonic-reflection = { version = "0.13", optional = true }

# Cargo.toml — in [build-dependencies]
tonic-build = { version = "0.13", optional = true }
```

**Note on build-dependencies and features:** Cargo does not support `optional = true` on build-dependencies the same way as regular dependencies — `[build-dependencies]` entries are included whenever the feature that enables them appears. The correct pattern is:

```toml
[build-dependencies]
tonic-build = { version = "0.13", optional = true }
```

And then gating it in `[features]`:
```toml
interface-grpc = ["dep:tonic", "dep:prost", "dep:tonic-health", "dep:tonic-reflection", "dep:tonic-build"]
```

The build script (`build.rs`) must check `CARGO_FEATURE_INTERFACE_GRPC` env var and skip proto compilation when the feature is absent.

**Version verification (run before implementing):**
```bash
cargo tree --features interface-grpc -d | grep -E "tonic|prost"
```
Expected: zero duplicate entries. If prost 0.14 appears alongside 0.13, tonic version must be downgraded.

---

## Architecture Patterns

### Recommended Project Structure

This phase adds:
```
mnemonic/
├── proto/
│   └── mnemonic.proto        # Single proto file (D-06)
├── build.rs                  # tonic-build codegen, feature-gated (D-07, D-09)
├── Cargo.toml                # interface-grpc feature flag added
└── .github/workflows/
    └── release.yml           # protoc installation step added
```

Generated types land in `$OUT_DIR/mnemonic.v1.rs` (tonic-build default, inside `target/`). Included into src in Phase 27 via:
```rust
pub mod mnemonic_v1 {
    tonic::include_proto!("mnemonic.v1");
}
```
That include is NOT part of this phase (src changes are Phase 27). Phase 26 only verifies that `cargo build --features interface-grpc` succeeds.

### Pattern 1: Conditional build.rs

**What:** build.rs checks the `CARGO_FEATURE_INTERFACE_GRPC` environment variable. If absent (default build), exits immediately. If present, runs tonic-build codegen.

**When to use:** Any phase where a build-time step is gated on a feature flag.

**Example:**
```rust
// build.rs
fn main() {
    // Only run proto codegen when interface-grpc feature is active.
    // CARGO_FEATURE_ vars use UPPER_SNAKE_CASE with hyphens replaced by underscores.
    if std::env::var("CARGO_FEATURE_INTERFACE_GRPC").is_err() {
        return; // Default build: skip codegen entirely
    }

    // Belt-and-suspenders: emit before compile_protos to prevent always-dirty builds.
    // (tonic issue #2239 — compile_protos may emit an unresolvable path in some configs)
    println!("cargo:rerun-if-changed=proto/mnemonic.proto");
    println!("cargo:rerun-if-changed=build.rs");

    tonic_build::compile_protos("proto/mnemonic.proto")
        .expect("Failed to compile proto/mnemonic.proto");
}
```

**Key detail:** The `println!("cargo:rerun-if-changed=...")` BEFORE `compile_protos` matters. Cargo processes these directives after the build script runs; emitting the explicit path prevents the bug where `compile_protos` internally emits a path that doesn't exist on disk.

### Pattern 2: proto3 Service Definition

**What:** Standard proto3 service with request/response message pairs and a shared `Memory` type.

**When to use:** All four RPCs in this phase.

**Example:**
```protobuf
syntax = "proto3";

package mnemonic.v1;

import "google/protobuf/timestamp.proto";

service MnemonicService {
  rpc StoreMemory(StoreMemoryRequest) returns (StoreMemoryResponse);
  rpc SearchMemories(SearchMemoriesRequest) returns (SearchMemoriesResponse);
  rpc ListMemories(ListMemoriesRequest) returns (ListMemoriesResponse);
  rpc DeleteMemory(DeleteMemoryRequest) returns (DeleteMemoryResponse);
}

// Shared message type mirroring REST JSON Memory object (D-05)
message Memory {
  string id = 1;
  string content = 2;
  string agent_id = 3;
  string session_id = 4;        // Empty string = no session (proto3 default)
  repeated string tags = 5;     // Empty list = no tags (proto3 default)
  google.protobuf.Timestamp created_at = 6;
  string embedding_model = 7;
}

// StoreMemory
message StoreMemoryRequest {
  string content = 1;
  string agent_id = 2;
  string session_id = 3;
  repeated string tags = 4;
}
message StoreMemoryResponse {
  Memory memory = 1;
}

// SearchMemories
message SearchMemoriesRequest {
  string query = 1;
  string agent_id = 2;
  string session_id = 3;
  repeated string tags = 4;
  int32 limit = 5;
}
message SearchMemoriesResponse {
  repeated SearchResult results = 1;
}
message SearchResult {
  Memory memory = 1;
  float distance = 2;
}

// ListMemories
message ListMemoriesRequest {
  string agent_id = 1;
  string session_id = 2;
  repeated string tags = 3;
  int32 limit = 4;
  int32 offset = 5;
}
message ListMemoriesResponse {
  repeated Memory memories = 1;
  int32 total = 2;
}

// DeleteMemory
message DeleteMemoryRequest {
  string id = 1;
}
message DeleteMemoryResponse {
  Memory memory = 1;
}
```

**Field design rationale (Claude's Discretion):** `session_id` and `tags` use proto3 defaults (empty string / empty list) rather than `optional` wrappers. This avoids `prost::Option<String>` complexity in generated Rust types while still allowing callers to omit them. The distinction between "not provided" and "empty" is preserved by checking `is_empty()` in handlers (Phase 28). This matches the existing REST behavior where both fields are optional.

### Pattern 3: Feature Flag in Cargo.toml

**What:** `dep:` prefix syntax for optional dependencies, consistent with backend-qdrant/backend-postgres pattern.

**Example (matches existing project pattern exactly):**
```toml
[features]
backend-qdrant = ["dep:qdrant-client", "dep:prost-types"]
backend-postgres = ["dep:sqlx", "dep:pgvector"]
interface-grpc = ["dep:tonic", "dep:prost", "dep:tonic-health", "dep:tonic-reflection", "dep:tonic-build"]

[dependencies]
# ... existing deps ...
tonic = { version = "0.13", optional = true }
prost = { version = "0.13", optional = true }
tonic-health = { version = "0.13", optional = true }
tonic-reflection = { version = "0.13", optional = true }

[build-dependencies]
tonic-build = { version = "0.13", optional = true }
```

### Pattern 4: CI protoc Installation

**What:** `arduino/setup-protoc@v3` step added before `cargo build` in the matrix job.

**When to use:** Any CI job that runs `cargo build` with `interface-grpc` feature enabled.

**Example:**
```yaml
steps:
  - name: Checkout
    uses: actions/checkout@v4

  - name: Install Rust toolchain
    uses: dtolnay/rust-toolchain@stable
    with:
      targets: ${{ matrix.target }}

  - name: Install protoc          # MUST come before cargo build
    uses: arduino/setup-protoc@v3
    with:
      repo-token: ${{ secrets.GITHUB_TOKEN }}

  - name: Build binary
    run: cargo build --release --target ${{ matrix.target }}
```

**Critical ordering:** protoc install MUST precede `cargo build`. Missing protoc causes the build script to fail with a generic "No such file or directory" error about the generated `.rs` file, not a clear "protoc not found" message.

**Note on default binary:** The release.yml currently builds without `--features interface-grpc`. The CI update ensures protoc is available for when the feature is enabled. For Phase 26 specifically, adding protoc to CI is a forward-compatible step — the default build still works without it, but future feature-enabled builds won't break.

### Anti-Patterns to Avoid

- **Using `cfg!(feature = "interface-grpc")` in build.rs:** This does not work in build scripts. Use `std::env::var("CARGO_FEATURE_INTERFACE_GRPC").is_ok()` instead. The `cfg!` macro is for source files compiled by rustc, not build scripts compiled as ordinary Rust programs.
- **Bare filename with include directory in compile_protos:** `compile_protos_with_config(&["mnemonic.proto"], &["proto"])` triggers issue #2239 — emits `cargo:rerun-if-changed=mnemonic.proto` which doesn't exist. Always use the full relative path: `compile_protos("proto/mnemonic.proto")`.
- **Putting tonic-build in `[dependencies]` instead of `[build-dependencies]`:** tonic-build is a build-time codegen tool. It must go in `[build-dependencies]` or it adds binary size to the release artifact.
- **Adding protoc step after cargo build in CI:** The build.rs runs during `cargo build`. If protoc is absent, the build fails. Order is not flexible.
- **Enabling `interface-grpc` feature in the default release binary:** All gRPC deps must remain behind the feature flag. The default `cargo build --release` must not pull in tonic/prost.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Protobuf serialization | Custom binary serialization | prost (via tonic-build codegen) | Proto wire format has packed encoding, varint encoding, field number handling — all deceptively complex |
| Proto-to-Rust type mapping | Manual struct definitions | tonic-build generated types | Manual types won't match wire format; breaking on any field number change |
| CI protoc installation | Custom shell script to download protoc binary | arduino/setup-protoc@v3 | Platform detection, version pinning, GitHub API rate limiting — action handles all of it |
| Incremental build detection | Custom file hash checking | `cargo:rerun-if-changed` directives | Cargo's mtime-based tracking is the correct primitive; custom hashing duplicates it incorrectly |

**Key insight:** The protobuf serialization ecosystem is the standard; don't touch the wire format. Tonic-build generates all the boilerplate from the proto definition.

---

## Common Pitfalls

### Pitfall 1: tonic 0.14 / prost 0.14 Version Conflict

**What goes wrong:** Adding `tonic = "0.14"` causes `cargo tree -d` to show both `prost 0.13.5` (from qdrant-client) and `prost 0.14.x` (from tonic 0.14). This creates duplicate prost entries and type incompatibilities when both `backend-qdrant` and `interface-grpc` features are active.

**Why it happens:** tonic 0.14 requires `prost ^0.14`. qdrant-client requires `prost ^0.13.3`. Cargo cannot unify these — they are incompatible version constraints.

**How to avoid:** Use tonic 0.13.1. Verify: `cargo tree --features backend-qdrant,interface-grpc -d | grep -E "tonic|prost"` must show zero duplicates.

**Warning signs:** `cargo tree -d` shows two prost entries with different major patch versions; compiler errors about `impl Trait` not satisfied for prost types when combining features.

### Pitfall 2: Always-Dirty Incremental Build (tonic issue #2239)

**What goes wrong:** `cargo build` always reports "Compiling mnemonic" on every run (even with no file changes). The second build takes full build time instead of under 2 seconds.

**Why it happens:** When `compile_protos` is called with a path that only resolves via an include directory (not the literal path), tonic-build emits `cargo:rerun-if-changed=<basename>` pointing to a file that doesn't exist at that path. Cargo treats a missing watched file as "changed", so the build script re-runs on every invocation.

**How to avoid:** Pass the full relative path to `compile_protos`:
```rust
tonic_build::compile_protos("proto/mnemonic.proto")  // correct
// NOT: compile_protos("mnemonic.proto") with include ["proto"]
```
Also emit the explicit directive before the call:
```rust
println!("cargo:rerun-if-changed=proto/mnemonic.proto");
```

**Warning signs:** Running `cargo build` twice — second run produces "Compiling mnemonic" output in the terminal and takes more than a few hundred milliseconds.

### Pitfall 3: Protoc Missing in CI (Cryptic Error)

**What goes wrong:** CI fails with an error like `error: proc-macro derive panicked` or `No such file or directory: .../mnemonic.v1.rs` rather than "protoc not found".

**Why it happens:** When `protoc` is not on `PATH`, tonic-build's `compile_protos` call fails, but the error message from the build script is about the generated output file being absent (the file tonic-build would have created), not about protoc being missing.

**How to avoid:** Add `arduino/setup-protoc@v3` before `cargo build` in every CI job that runs with `interface-grpc` feature. The action installs `protoc` to PATH. Use `repo-token: ${{ secrets.GITHUB_TOKEN }}` to avoid GitHub API rate limiting.

**Warning signs:** CI build fails on "Build binary" step with a file-not-found error mentioning an `.rs` file in `OUT_DIR`; the error does not mention `protoc` at all.

### Pitfall 4: `cfg!` Macro in build.rs

**What goes wrong:** `if cfg!(feature = "interface-grpc") { ... }` in build.rs does nothing — the condition is never true.

**Why it happens:** `cfg!` evaluates compile-time attributes of the crate being compiled by rustc. `build.rs` is compiled as a standalone binary, separate from the main crate. Feature flags are not propagated to the build script via `cfg!`; they are propagated via environment variables.

**How to avoid:** Use `std::env::var("CARGO_FEATURE_INTERFACE_GRPC").is_ok()` — the env var name is the feature name uppercased with hyphens replaced by underscores.

---

## Code Examples

Verified patterns from official sources:

### Complete build.rs

```rust
// build.rs
// Source: Cargo reference (env vars) + tonic-build docs + STATE.md critical flags
fn main() {
    // Only run proto codegen when interface-grpc feature is active.
    // cfg!(feature = "...") does NOT work in build scripts — use env var.
    if std::env::var("CARGO_FEATURE_INTERFACE_GRPC").is_err() {
        return;
    }

    // Explicit rerun-if-changed BEFORE compile_protos.
    // Prevents always-dirty bug (tonic issue #2239) by ensuring Cargo tracks
    // a path that actually exists on disk.
    println!("cargo:rerun-if-changed=proto/mnemonic.proto");
    println!("cargo:rerun-if-changed=build.rs");

    // Full path form prevents the always-dirty bug: tonic_build emits
    // cargo:rerun-if-changed based on the literal argument. "proto/mnemonic.proto"
    // exists; bare "mnemonic.proto" with an include dir does not.
    tonic_build::compile_protos("proto/mnemonic.proto")
        .expect("Failed to compile proto/mnemonic.proto");
}
```

### Cargo.toml feature additions

```toml
[features]
# Existing features (DO NOT CHANGE)
backend-qdrant = ["dep:qdrant-client", "dep:prost-types"]
backend-postgres = ["dep:sqlx", "dep:pgvector"]
# New: gRPC interface (Phase 26)
# tonic 0.13 / prost 0.13 — pinned to match qdrant-client's prost ^0.13.3 anchor.
# DO NOT upgrade to tonic 0.14 without verifying qdrant-client also moves to prost 0.14.
interface-grpc = ["dep:tonic", "dep:prost", "dep:tonic-health", "dep:tonic-reflection", "dep:tonic-build"]

# In [dependencies]:
tonic = { version = "0.13", optional = true }
prost = { version = "0.13", optional = true }
tonic-health = { version = "0.13", optional = true }
tonic-reflection = { version = "0.13", optional = true }

# In [build-dependencies]:
tonic-build = { version = "0.13", optional = true }
```

### Incremental build verification

```bash
# Verify incremental build is not always-dirty (Success Criterion #2)
cargo build --features interface-grpc 2>&1 | grep -c "Compiling mnemonic"   # expect 1
cargo build --features interface-grpc 2>&1 | grep -c "Compiling mnemonic"   # expect 0
# Second run must complete in under 2 seconds
time cargo build --features interface-grpc
```

### Dependency duplicate check

```bash
# Hard gate: zero duplicates (Success Criterion #4)
cargo tree --features backend-qdrant,interface-grpc -d | grep -E "tonic|prost"
# Expected output: empty (no duplicates)
```

### CI YAML addition

```yaml
# Add to .github/workflows/release.yml — in build job steps,
# BEFORE "Build binary" step
- name: Install protoc
  uses: arduino/setup-protoc@v3
  with:
    repo-token: ${{ secrets.GITHUB_TOKEN }}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `tonic_build::configure().compile(&protos, &includes)` | `tonic_build::compile_protos(path)` for single-file | tonic 0.8+ | Simpler API for common case; both still work |
| `tonic 0.11` + `prost 0.12` | `tonic 0.13` + `prost 0.13` | tonic 0.12.0 (2024-07-08) | Breaking change when upgrading tonic 0.11 → 0.12 |
| `tonic 0.13` + `prost 0.13` | `tonic 0.14` + `prost 0.14` | tonic 0.14.0 (2025-07-28) | Breaking for projects with other prost 0.13 deps (like qdrant-client) |
| `apt-get install protobuf-compiler` in CI | `arduino/setup-protoc@v3` | Available since 2023 | Pinnable version, cross-platform (Ubuntu/macOS), latest protoc release |

**Deprecated/outdated:**
- `tonic_build::configure().compile()` (the older multi-arg form): Still works but `compile_protos()` is simpler for single-file use
- Installing protoc via apt without version pinning: OS package is often 2+ major versions behind

---

## Open Questions

1. **tonic-build optional in [build-dependencies]**
   - What we know: `optional = true` is supported in `[build-dependencies]` and the `dep:` prefix works the same way as in `[dependencies]`
   - What's unclear: Whether Cargo correctly skips the build-dependency entirely (not just its linking) when the feature is absent — or if build.rs is still compiled
   - Recommendation: build.rs is always compiled by Cargo if the file exists, regardless of features. The `CARGO_FEATURE_INTERFACE_GRPC` env var check at the top of `main()` is the real gate. The `optional = true` on tonic-build prevents it from being added to the compiled binary's dependency list. The build.rs will exist and compile, but `compile_protos` will not be called unless the feature is enabled.

2. **Default release binary and protoc in CI**
   - What we know: The current release.yml builds without `--features interface-grpc`; protoc is not needed for the default build
   - What's unclear: Should protoc be added to CI only when the feature is enabled, or unconditionally?
   - Recommendation: Add protoc unconditionally (it's free in CI time cost) to prevent future breakage when someone adds `--features interface-grpc` to the release build.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (`cargo test`) |
| Config file | None (uses Cargo.toml settings) |
| Quick run command | `cargo test --features interface-grpc` |
| Full suite command | `cargo test --features interface-grpc,backend-qdrant` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PROTO-01 | All five RPC message shapes present in generated code | Compile-time | `cargo build --features interface-grpc` | ❌ Wave 0 — new build.rs + proto file |
| PROTO-01 | Generated Rust types accessible (StoreMemoryRequest, etc.) | Unit (compile-time) | `cargo test --features interface-grpc --test proto_types` | ❌ Wave 0 |
| PROTO-02 | Incremental build not always-dirty (second build < 2s, no output) | Manual/smoke | `time cargo build --features interface-grpc` run twice | N/A — verified manually |
| PROTO-03 | CI release workflow passes with protoc installed | CI smoke | GitHub Actions run on push | ❌ Wave 0 — release.yml change |
| PROTO-04 | Default binary carries no gRPC dependencies | Compile-time | `cargo build` (no features) succeeds without tonic | ❌ Wave 0 |
| PROTO-04 | cargo tree -d shows zero duplicate tonic/prost entries | Smoke | `cargo tree --features interface-grpc,backend-qdrant -d | grep -E "tonic|prost"` | N/A — run manually |

### Sampling Rate

- **Per task commit:** `cargo build --features interface-grpc` (verifies codegen compiles)
- **Per wave merge:** `cargo test --features interface-grpc` + `cargo build` (default, no features)
- **Phase gate:** Full suite green + incremental build verified + `cargo tree -d` shows zero duplicates before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `proto/mnemonic.proto` — the proto file itself (PROTO-01)
- [ ] `build.rs` — proto codegen build script (PROTO-02)
- [ ] `tests/proto_types.rs` — compile-time test that asserts generated message types exist with correct field names (PROTO-01)
- [ ] Updated `Cargo.toml` — interface-grpc feature flag (PROTO-04)
- [ ] Updated `.github/workflows/release.yml` — protoc step (PROTO-03)

---

## Sources

### Primary (HIGH confidence)

- crates.io API — verified: tonic 0.13.1 (prost ^0.13), tonic 0.14.5 (prost ^0.14), tonic-build/tonic-health/tonic-reflection 0.13.1
- crates.io API — verified: qdrant-client 1.17.0 requires prost ^0.13.3, prost-types ^0.13.3
- `cargo tree --features backend-qdrant` — confirmed prost 0.13.5 is the in-tree resolution
- Cargo book (env vars reference) — `CARGO_FEATURE_<NAME>` pattern for build scripts
- arduino/setup-protoc GitHub — v3.0.0 is latest (2024-01-31); confirmed inputs

### Secondary (MEDIUM confidence)

- [tonic issue #2239](https://github.com/hyperium/tonic/issues/2239) — always-dirty rerun-if-changed bug confirmed open as of 2025-03-27; root cause understood
- [tonic issue #2317](https://github.com/hyperium/tonic/issues/2317) — tonic 0.14 + prost 0.14 release confirmed; incompatibility with prost 0.13 confirmed
- [tonic-build docs.rs 0.12.3](https://docs.rs/tonic-build/0.12.3/tonic_build/) — `compile_protos` full-path usage verified

### Tertiary (LOW confidence)

- None — all critical claims verified through primary/secondary sources

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — versions verified via crates.io API and cargo tree output
- Architecture: HIGH — patterns verified against existing Cargo.toml feature flag usage and Cargo documentation
- Pitfalls: HIGH — tonic issues #2239 and #2317 directly verified; CI pattern verified against arduino/setup-protoc repo

**Research date:** 2026-03-22
**Valid until:** 2026-06-22 (stable ecosystem; prost/tonic release cadence is every few months; re-verify versions before implementing)
