# Stack Research

**Domain:** gRPC interface addition to existing Rust memory server (v1.5)
**Researched:** 2026-03-22
**Confidence:** HIGH — versions verified via crates.io API and tonic GitHub source; architecture confirmed via official docs

## Context

This is a **subsequent milestone** stack. The existing validated stack (axum 0.8, tokio 1, sqlx 0.8, prost-types 0.13, qdrant-client 1, etc.) is unchanged. This document covers only the **new dependencies required for v1.5 gRPC**.

---

## New Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `tonic` | `0.14` | gRPC server runtime | Current stable (0.14.5, released 2026-02-19). Internally uses axum 0.8, hyper 1, tower 0.5 — exactly what mnemonic already uses. The standard gRPC framework for Rust with first-class async/await support. |
| `tonic-prost` | `0.14` | Prost codec for tonic messages | Tonic 0.14 extracted prost integration into its own crate (`tonic-prost`). This is the runtime codec for encoding/decoding protobuf messages in tonic services. Previously bundled in tonic core — now a separate dep. |
| `tonic-reflection` | `0.14` | gRPC server reflection | Implements the `grpc.reflection.v1` protocol. Lets `grpcurl`, Evans, and other tooling discover services without needing the `.proto` file. Same version family as tonic. Use the `server` feature. |
| `prost` | `0.14` | Protobuf message types at runtime | Tonic 0.13 used prost 0.13; tonic 0.14 requires prost 0.14. These are incompatible across minor versions. The existing Cargo.toml has `prost-types = "0.13"` gated behind `backend-qdrant` — must bump to `0.14`. |
| `prost-types` | `0.14` | Protobuf well-known types | Used by `tonic-reflection`. Must match the `prost` version in use. Existing `prost-types = "0.13"` (optional, backend-qdrant) conflicts — bump to `0.14`. |

---

## New Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tonic-prost-build` | `0.14` | Proto file → Rust codegen | `[build-dependencies]` only. Replaces old `tonic-build` starting in tonic 0.14. Wraps `prost-build` and generates both message structs and gRPC service traits from `.proto` files. Always needed. |
| `prost-build` | `0.14` | Underlying protoc wrapper | Transitive dep of `tonic-prost-build`; no need to add directly. Requires `protoc` binary on build machine — not bundled. |
| `tonic-health` | `0.14` | Standard gRPC health check service | Optional. Provides `grpc.health.v1.Health` service per gRPC spec. Add if the grpc `health` endpoint is in scope for v1.5. Skip if using a custom health unary RPC in the mnemonic proto. |

---

## Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `protoc` (system) | Compiles `.proto` files during `cargo build` | Required by `prost-build`. Not bundled. Install via `brew install protobuf` (macOS) or `apt install protobuf-compiler` (Linux). Version 3.x or 4.x both work. |
| `arduino/setup-protoc@v3` | GitHub Actions step to install protoc in CI | Add before `cargo build` in release workflow. Avoids hardcoding protoc binaries in the repo. |
| `grpcurl` | CLI for testing gRPC endpoints | Install locally. Works with `tonic-reflection` to discover services without `.proto` file. |

---

## Cargo.toml Changes

```toml
[dependencies]
# --- NEW for v1.5 gRPC ---
tonic            = { version = "0.14", features = ["transport"] }
tonic-prost      = "0.14"
tonic-reflection = { version = "0.14", features = ["server"] }
prost            = "0.14"

# UPDATED: was "0.13" in backend-qdrant feature — must bump to match tonic 0.14
prost-types      = { version = "0.14", optional = true }

[build-dependencies]
# --- NEW for v1.5 gRPC ---
tonic-prost-build = "0.14"
```

Note on `prost-types` version bump: The `backend-qdrant` feature currently pins `prost-types = "0.13"`. Bumping to `0.14` may conflict with `qdrant-client 1.x`, which transitively brings in its own prost version. Run `cargo tree -d` before starting to surface any duplicate version conflicts. Cargo can in principle resolve both `prost 0.13` and `prost 0.14` as separate crates as long as mnemonic's API layer does not pass prost types across the boundary — which it does not.

---

## System Requirement: protoc

`prost-build` (used by `tonic-prost-build`) requires `protoc` (Protocol Buffers compiler) present during build. It does NOT bundle protoc as of prost-build 0.11+.

**Local development:**
```bash
brew install protobuf          # macOS
apt install protobuf-compiler  # Ubuntu/Debian
```

**CI / GitHub Actions** — add before `cargo build` in the release workflow:
```yaml
- name: Install protoc
  uses: arduino/setup-protoc@v3
  with:
    repo-token: ${{ secrets.GITHUB_TOKEN }}
```

**Alternative:** Use `protoc-bin-vendored = "3.2.0"` in `[build-dependencies]` to vendor a protoc binary alongside the project. Avoids CI setup at the cost of ~5MB binaries per platform checked into dev tooling.

---

## Proto File Layout

```
mnemonic/
  proto/
    mnemonic/v1/
      mnemonic.proto      # service definition: Store, Search, List, Delete, Health
  build.rs                # calls tonic_prost_build::compile_protos(...)
```

`build.rs` pattern:
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .file_descriptor_set_path(
            std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap())
                .join("mnemonic_descriptor.bin"),
        )
        .compile_protos(&["proto/mnemonic/v1/mnemonic.proto"], &["proto"])?;
    Ok(())
}
```

The `.bin` file is consumed by `tonic-reflection` at runtime:
```rust
let descriptor = include_bytes!(concat!(env!("OUT_DIR"), "/mnemonic_descriptor.bin"));
```

---

## Architecture: Dual-Port Server Pattern

PROJECT.md specifies gRPC runs on a **separate port** from REST. The correct pattern is `tokio::join!` across two independently-bound listeners — no multiplexing crate needed:

```rust
// In serve subcommand (simplified pseudocode)
let rest_future = axum::serve(rest_listener, rest_router);

let grpc_future = tonic::transport::Server::builder()
    .add_service(
        tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(MNEMONIC_DESCRIPTOR)
            .build_v1()?
    )
    .add_service(MnemonicServer::with_interceptor(svc, grpc_auth_interceptor))
    .serve_with_incoming(grpc_listener_stream);

tokio::join!(rest_future, grpc_future);
```

Both servers share the same `AppState` (same `Arc<dyn StorageBackend>`, same embedding engine) via `Arc` clones — no duplication of resources.

---

## Auth Pattern: Tonic Interceptor

The existing auth validates `Authorization: Bearer mnk_...` HTTP headers. For gRPC, the equivalent is gRPC metadata under the `authorization` key (lowercase, per gRPC metadata conventions). Tonic interceptors access metadata via `request.metadata()`.

**Recommended: synchronous tonic interceptor** applied per-service with `with_interceptor()`:

```rust
fn grpc_auth_interceptor(
    req: tonic::Request<()>,
) -> Result<tonic::Request<()>, tonic::Status> {
    match req.metadata().get("authorization") {
        Some(token) => {
            // Reuse existing validate_api_key() from the auth module
            // Return Err(Status::unauthenticated("...")) on failure
            Ok(req)
        }
        None if auth_is_enabled() => {
            Err(tonic::Status::unauthenticated("missing authorization metadata"))
        }
        None => Ok(req), // open mode — no keys configured
    }
}
```

Applied as: `MnemonicServer::with_interceptor(svc, grpc_auth_interceptor)`

This mirrors the existing axum `route_layer()` auth middleware — the interceptor bridges gRPC metadata to the shared validation logic. No new auth crates needed.

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `tonic 0.14` | `tonic 0.13` | Never — 0.13 is incompatible with prost 0.14 and uses an older hyper. There is no reason to use 0.13 for a new integration. |
| `tonic-prost-build` (build-dep) | old `tonic-build` | `tonic-build` is the pre-0.14 codegen crate. Do not use — it is for tonic ≤ 0.13. |
| Separate port (`tokio::join!`) | Single-port multiplexing (content-type routing) | Use single-port multiplexing only if TLS termination proxy requires it or if firewall rules prevent a second port. For mnemonic's use case (agent localhost/intranet), separate ports are simpler. |
| Sync interceptor (`with_interceptor`) | `tonic-async-interceptor` or `tonic-middleware` | Use async interceptor if token validation requires a network round-trip (e.g., remote token introspection). Mnemonic's `mnk_...` tokens validate against local SQLite — sync is sufficient. |
| `arduino/setup-protoc` in CI | `protoc-bin-vendored` | Use `protoc-bin-vendored` if the CI environment is air-gapped or if cross-compiling makes setup-protoc unreliable. |

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `grpc-web` / `tonic-web` | Adds CORS and envelope complexity. gRPC-Web is for browser clients. Mnemonic targets agent frameworks with native gRPC support. | Native tonic transport |
| Streaming RPCs | Explicitly out of scope per PROJECT.md — "Unary RPCs mirroring REST behavior — no streaming." | Unary handlers only |
| `tonic-middleware` crate | Async middleware for scenarios requiring response interception. The `mnk_...` bearer token check is synchronous and doesn't need this. Adds a dependency for no benefit. | Built-in `with_interceptor()` |
| gRPC for compaction/keys endpoints | Explicitly out of scope per PROJECT.md — "gRPC support for compaction/keys — hot-path only in v1.5." | Keep those REST-only |
| TLS for gRPC in v1.5 | Increases build complexity (rustls or aws-lc feature selection). Agents typically connect on localhost or private networks. Defer to a future milestone. | Plain-text `transport` feature only; TLS is opt-in via `tls-ring` feature later |
| Direct `tonic` dep in `backend-qdrant` | `qdrant-client` already owns tonic transitively. Adding tonic directly risks version conflicts. | Let qdrant-client manage its tonic version; mnemonic's gRPC uses tonic 0.14 |

---

## Version Compatibility

| Package | Version | Compatible With | Notes |
|---------|---------|-----------------|-------|
| `tonic 0.14` | prost 0.14, axum 0.8, hyper 1, tower 0.5, tokio 1 | All already in Cargo.toml — no conflicts expected |
| `prost 0.14` | prost-types 0.14 | Must bump existing `prost-types = "0.13"` (backend-qdrant feature) |
| `tonic 0.14` + `qdrant-client 1` | Cargo resolves prost 0.13 and 0.14 as separate crates | Run `cargo tree -d` to confirm no link error at boundary |
| `tonic-prost-build 0.14` | protoc >= 3.12 | protoc 3.x and 4.x (libprotobuf 21+) both work |
| `tonic-reflection 0.14` | prost 0.14, prost-types 0.14, tonic 0.14 | Same version family — use consistently |

---

## Stack Patterns by Variant

**If TLS is needed in a future milestone:**
- Add `features = ["tls-ring"]` to the tonic dependency (uses rustls via aws-lc-rs)
- Or `features = ["tls-native-roots"]` if matching the existing reqwest native-tls stack
- Use `Server::builder().tls_config(tls)` before `.serve()`

**If single-port multiplexing is ever desired (not v1.5):**
- Tonic 0.14's router can be converted to axum routes via `.into_router()`
- Route based on: `content_type.as_bytes().starts_with(b"application/grpc")`
- Requires axum to run in HTTP/2 mode (add `hyper` direct dep with http2 feature)

**If streaming is added in a future milestone:**
- Change proto service methods to `rpc StreamSearch(SearchRequest) returns (stream SearchResult)`
- Change handler return types to `ResponseStream = ReceiverStream<Result<SearchResult, Status>>`
- No new crates needed — tonic's `transport` feature handles streaming

---

## Sources

- crates.io API (direct JSON) — tonic 0.14.5 (2026-02-19), tonic-reflection 0.14.5, tonic-prost-build 0.14.5, prost 0.14.3, prost-build 0.14.3, tonic-health 0.14.5 — versions verified (HIGH confidence)
- [tonic v0.14.5 Cargo.toml](https://raw.githubusercontent.com/hyperium/tonic/v0.14.5/tonic/Cargo.toml) — axum 0.8, hyper 1, tower 0.5, h2 0.4 dependency versions confirmed (HIGH confidence)
- [tonic-prost Cargo.toml v0.14.5](https://raw.githubusercontent.com/hyperium/tonic/v0.14.5/tonic-prost/Cargo.toml) — prost 0.14 requirement confirmed (HIGH confidence)
- [tonic-reflection Cargo.toml v0.14.5](https://raw.githubusercontent.com/hyperium/tonic/v0.14.5/tonic-reflection/Cargo.toml) — prost 0.14, prost-types 0.14 confirmed (HIGH confidence)
- [tonic v0.14.0 release notes](https://github.com/hyperium/tonic/releases/tag/v0.14.0) — prost extraction into tonic-prost / tonic-prost-build confirmed (HIGH confidence)
- [prost-build docs.rs 0.14.3](https://docs.rs/prost-build/latest/prost_build/) — protoc system requirement (no bundled protoc) confirmed (HIGH confidence)
- [tonic authentication example](https://github.com/hyperium/tonic/blob/master/examples/src/authentication/server.rs) — `authorization` metadata key, `with_interceptor()` pattern (HIGH confidence)
- [tonic transport::Server docs](https://docs.rs/tonic/latest/tonic/transport/struct.Server.html) — `add_service`, `serve`, `layer` API (HIGH confidence)
- WebSearch results — dual-port `tokio::join!` pattern confirmed by multiple community sources (MEDIUM confidence — no single canonical code snippet from tonic docs)
- [tonic-middleware crates.io](https://crates.io/crates/tonic-middleware) — evaluated and excluded for this use case (HIGH confidence in exclusion)

---

*Stack research for: Mnemonic v1.5 gRPC (tonic) additions*
*Researched: 2026-03-22*
