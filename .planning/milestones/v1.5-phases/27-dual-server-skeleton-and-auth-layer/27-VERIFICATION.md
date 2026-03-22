---
phase: 27-dual-server-skeleton-and-auth-layer
verified: 2026-03-22T00:00:00Z
status: passed
score: 12/12 must-haves verified
re_verification: false
---

# Phase 27: Dual-Server Skeleton and Auth Layer — Verification Report

**Phase Goal:** `mnemonic serve` starts both REST and gRPC on separate ports simultaneously, the async Tower auth layer rejects invalid tokens and passes open-mode traffic, and grpc_port is configurable
**Verified:** 2026-03-22
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths — Plan 01 (SERVER-01, SERVER-02, SERVER-03)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `mnemonic serve` starts both REST and gRPC servers on separate ports | VERIFIED | `main.rs:272` uses `tokio::try_join!(rest_fut, grpc_fut)?` under `#[cfg(feature = "interface-grpc")]` |
| 2 | grpcurl health check against the gRPC port returns SERVING | VERIFIED | `grpc/mod.rs:73-76` registers `MnemonicServiceServer<MnemonicGrpcService>` as SERVING via `tonic_health::server::health_reporter()` and `.set_serving()` |
| 3 | REST API continues to respond on its existing port unchanged | VERIFIED | Non-feature path at `main.rs:276-278` calls `server::serve(&config, state).await?` unchanged; `cargo build` (no features) succeeds |
| 4 | MNEMONIC_GRPC_PORT env var changes the gRPC port | VERIFIED | `config.rs:179-187` has `test_grpc_port_env_override` test; passes (23/23 config tests green) |
| 5 | grpc_port defaults to 50051 and appears in config show output | VERIFIED | `config.rs:38-39` defaults `grpc_port: 50051`; `cli.rs:189,207` shows it in both JSON and human output |
| 6 | If either port fails to bind, the process exits with a clear error | VERIFIED | `tokio::try_join!` semantics: first `Err` from either future propagates and shuts both down |

### Observable Truths — Plan 02 (AUTH-01, AUTH-02, AUTH-03)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 7 | gRPC request with invalid Bearer token returns Code::Unauthenticated | VERIFIED | `auth.rs:123-125` returns `Status::unauthenticated("invalid or revoked API key").into_http()`; `test_grpc_auth_invalid_token_returns_unauthenticated` passes |
| 8 | gRPC request with no API keys configured passes through without auth (open-mode) | VERIFIED | `auth.rs:84-85` checks `count_active_keys() == 0` and calls `inner.call(req).await`; `test_grpc_auth_open_mode_bypasses` passes |
| 9 | gRPC request with a valid Bearer token has AuthContext injected into extensions | VERIFIED | `auth.rs:119-121` inserts `auth_ctx` into `req.extensions_mut()`; `test_grpc_auth_valid_token_injects_context` verifies presence in extensions |
| 10 | A malformed authorization header returns Code::InvalidArgument | VERIFIED | `auth.rs:107-110` returns `Status::invalid_argument(...).into_http()`; `test_grpc_auth_malformed_header_returns_invalid_argument` passes |
| 11 | Health check requests bypass auth even when API keys are configured | VERIFIED | `auth.rs:79-81` short-circuits on `/grpc.health.v1.Health/` URI prefix; `test_grpc_auth_health_bypass` passes |
| 12 | AuthContext in extensions enables Phase 28 scope enforcement | VERIFIED | `auth.rs:119-121` inserts `crate::auth::AuthContext` into extensions; Phase 28 handlers can extract via `request.extensions().get::<AuthContext>()` |

**Score:** 12/12 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/grpc/mod.rs` | gRPC service struct, `include_proto!`, unimplemented handlers, `serve_grpc()` with tonic-health | VERIFIED | 91 lines; contains `include_proto!("mnemonic.v1")`, `MnemonicGrpcService`, 4 `Status::unimplemented` handlers, `health_reporter`, `.layer(auth::GrpcAuthLayer {...})` |
| `src/grpc/auth.rs` | `GrpcAuthLayer` and `GrpcAuthService` Tower types for async gRPC auth | VERIFIED | 373 lines; contains `GrpcAuthLayer`, `GrpcAuthService`, `count_active_keys`, `key_service.validate`, `into_http()`, health bypass, `mem::replace`, 6 unit tests |
| `src/config.rs` | `grpc_port: u16` field with default 50051 | VERIFIED | Line 12: `pub grpc_port: u16`; line 38: `grpc_port: 50051` in `Default` impl; `test_grpc_port_env_override` test present |
| `src/main.rs` | Dual-port startup via `tokio::try_join!`, cfg-gated grpc module | VERIFIED | Line 17: `#[cfg(feature = "interface-grpc")] mod grpc`; line 272: `tokio::try_join!(rest_fut, grpc_fut)?`; fallback at line 277 |
| `src/cli.rs` | `grpc_port` in config show JSON and human output | VERIFIED | Line 189: JSON `"grpc_port": config.grpc_port`; line 207: human `grpc_port` println |
| `src/lib.rs` | `pub mod grpc` under cfg gate for test discovery | VERIFIED | Lines 13-14: `#[cfg(feature = "interface-grpc")] pub mod grpc` |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/main.rs` | `src/grpc/mod.rs` | `cfg-gated mod grpc` + `grpc::serve_grpc()` call | VERIFIED | `mod grpc` at line 17; `grpc::serve_grpc(&config, grpc_svc)` at line 271 |
| `src/main.rs` | `src/server.rs` | `tokio::try_join!` of `server::serve` and `grpc::serve_grpc` | VERIFIED | Both futures in `try_join!` at line 272 |
| `src/grpc/mod.rs` | `src/server.rs` | Shared Arc instances from AppState fields | VERIFIED | `service.clone()`, `compaction.clone()`, `key_service.clone()` passed to AppState; originals move to `MnemonicGrpcService` |
| `src/grpc/auth.rs` | `src/auth.rs` | `KeyService.validate()` and `KeyService.count_active_keys()` calls | VERIFIED | `auth.rs:84` calls `key_service.count_active_keys().await`; `auth.rs:116` calls `key_service.validate(&bearer).await` |
| `src/grpc/auth.rs` | `src/auth.rs` | `AuthContext` struct inserted into request extensions | VERIFIED | `auth.rs:120-121`: `req.extensions_mut().insert(auth_ctx)` where `auth_ctx: crate::auth::AuthContext` |
| `src/grpc/mod.rs` | `src/grpc/auth.rs` | `GrpcAuthLayer` applied via `Server::builder().layer()` | VERIFIED | `mod.rs:81-83`: `.layer(auth::GrpcAuthLayer { key_service: Arc::clone(&svc.key_service) })` |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| SERVER-01 | 27-01 | Dual-port startup via tokio::try_join! | SATISFIED | `main.rs:272`: `tokio::try_join!(rest_fut, grpc_fut)?`; feature-gated on `interface-grpc` |
| SERVER-02 | 27-01 | grpc_port config field with MNEMONIC_GRPC_PORT env var | SATISFIED | `config.rs:12` field; `config.rs:38` default 50051; `test_grpc_port_env_override` passes |
| SERVER-03 | 27-01 | Shared Arc<MemoryService>, Arc<KeyService>, Arc<CompactionService> with REST server | SATISFIED | Same Arc instances in `main.rs:244-259`; gRPC gets originals, AppState gets clones (or vice versa); EmbeddingEngine shared indirectly through Arc<MemoryService> |
| AUTH-01 | 27-02 | Bearer token auth via async Tower Layer | SATISFIED | `GrpcAuthLayer` + `GrpcAuthService` in `src/grpc/auth.rs`; wired into `serve_grpc()` via `.layer()`; 6 tests pass |
| AUTH-02 | 27-02 | Agent scope enforcement infrastructure (AuthContext injection) | SATISFIED (infra) | AuthContext injected into request extensions on valid token — handler-level enforcement deferred to Phase 28 per RESEARCH.md D-17 and explicit plan clarification |
| AUTH-03 | 27-02 | Open mode bypass when zero API keys | SATISFIED | `auth.rs:84-85`: `Ok(0) => return inner.call(req).await`; `test_grpc_auth_open_mode_bypasses` passes |

**Note on AUTH-02:** REQUIREMENTS.md marks AUTH-02 as `[x]` complete and RESEARCH.md explicitly states "Phase 27 only needs the Layer to inject AuthContext" for this requirement. The infrastructure (AuthContext injection) is fully implemented. Per-handler scope enforcement (matching agent_id against key scope) is deferred to Phase 28. This split is intentional and documented.

**Orphaned requirements check:** REQUIREMENTS.md maps SERVER-01, SERVER-02, SERVER-03, AUTH-01, AUTH-02, AUTH-03 to Phase 27. All 6 are claimed in the plan frontmatter and verified above. No orphaned requirements.

---

## Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| `src/grpc/mod.rs:35,42,49,56` | `Status::unimplemented(...)` handlers | INFO | Intentional skeleton stubs — documented in both SUMMARYs as "Phase 28 will implement". Not a blocker; these are the declared output of this phase. |

No blocker or warning anti-patterns found. The unimplemented RPC stubs are the intended output of Plan 01 (skeleton phase) and are explicitly flagged in the SUMMARY as known stubs pending Phase 28.

---

## Human Verification Required

None. All observable behaviors from the phase goal are verifiable programmatically:

- Dual-port startup: verified via code structure and `try_join!`
- Auth rejection: verified via 6 passing unit tests with real in-memory KeyService
- Open-mode bypass: verified via unit test
- `grpc_port` configurability: verified via passing env-override test
- Build correctness: `cargo build` and `cargo build --features interface-grpc` both succeed
- Full test suite: 85 tests (no feature) and 91 tests (with feature) all pass

---

## Build and Test Results

| Check | Result |
|-------|--------|
| `cargo build` (no features) | PASS — 2 warnings, 0 errors |
| `cargo build --features interface-grpc` | PASS — 3 warnings, 0 errors |
| `cargo test --lib` (no features) | PASS — 85/85 tests |
| `cargo test --features interface-grpc --lib` | PASS — 91/91 tests |
| `cargo test config::tests --lib` | PASS — 23/23 tests |
| `cargo test --features interface-grpc grpc::auth::tests --lib` | PASS — 6/6 tests |

---

## Commit Verification

All 5 commits documented in SUMMARYs confirmed present in git history:

| Commit | Description |
|--------|-------------|
| `1405411` | feat(27-01): add grpc_port to Config and update config show output |
| `fb2bc12` | feat(27-01): create gRPC module skeleton with service struct and serve_grpc |
| `58bad0f` | feat(27-01): wire dual-port startup in main.rs via tokio::try_join! |
| `78e759e` | feat(27-02): implement GrpcAuthLayer and GrpcAuthService in src/grpc/auth.rs |
| `75120c8` | feat(27-02): wire GrpcAuthLayer into serve_grpc Server::builder |

---

## Summary

Phase 27 goal is fully achieved. Both Plans 01 and 02 delivered their stated outputs and all 12 observable truths hold against the actual codebase. The dual-server skeleton compiles and starts both REST and gRPC servers via `tokio::try_join!`. The Tower auth layer correctly handles all five auth scenarios (open-mode, valid token, invalid token, missing header, malformed header) plus health check bypass, with 6 unit tests as proof. `grpc_port` is configurable via env var and TOML, and appears in `config show` output. The default binary is unchanged. All requirement IDs (SERVER-01, SERVER-02, SERVER-03, AUTH-01, AUTH-02, AUTH-03) are satisfied with code evidence.

---

_Verified: 2026-03-22_
_Verifier: Claude (gsd-verifier)_
