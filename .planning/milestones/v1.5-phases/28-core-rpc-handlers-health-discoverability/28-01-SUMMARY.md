---
phase: 28-core-rpc-handlers-health-discoverability
plan: "01"
subsystem: grpc
tags: [grpc, tonic, reflection, handlers, auth]
dependency_graph:
  requires:
    - 27-02 (GrpcAuthLayer Tower middleware)
    - src/service.rs (MemoryService, CreateMemoryRequest, SearchParams, ListParams)
    - src/error.rs (ApiError variants)
    - src/auth.rs (AuthContext, KeyService)
    - proto/mnemonic.proto (protobuf message definitions)
  provides:
    - Working gRPC server with all 4 CRUD handlers
    - tonic-reflection for grpcurl discoverability
    - grpc_status_from_response test helper (in auth.rs tests)
  affects:
    - build.rs (descriptor set generation)
    - src/grpc/mod.rs (handler implementations, helpers)
    - src/grpc/auth.rs (reflection bypass)
tech_stack:
  added: []
  patterns:
    - tonic-reflection via FILE_DESCRIPTOR_SET from build.rs file_descriptor_set_path
    - enforce_scope duplicated from server.rs (scope isolation per agent_id)
    - api_error_to_status mapping ApiError to tonic::Status codes
    - memory_to_proto converting service Memory to proto Memory with prost_types::Timestamp
key_files:
  created: []
  modified:
    - build.rs
    - src/grpc/mod.rs
    - src/grpc/auth.rs
decisions:
  - "Duplicate enforce_scope from server.rs into grpc/mod.rs rather than sharing — avoids axum-specific import coupling"
  - "Reflection bypass added to GrpcAuthLayer so grpcurl list works even when auth is active"
  - "f64 -> f32 cast for distance in search results (proto uses float, service uses f64)"
  - "Pass first tag only for repeated tags filter (REST SearchParams supports single tag filter)"
metrics:
  duration: "~15 minutes"
  completed_date: "2026-03-22"
  tasks_completed: 2
  files_modified: 3
---

# Phase 28 Plan 01: Core RPC Handlers, Health, and Discoverability Summary

**One-liner:** All four gRPC handlers (StoreMemory, SearchMemories, ListMemories, DeleteMemory) implemented with scope enforcement, error mapping, and tonic-reflection wired for grpcurl discoverability.

## What Was Built

### Task 1: build.rs reflection descriptor and helper functions

**build.rs** — Switched from the `tonic_build::compile_protos("proto/mnemonic.proto")` shorthand to `tonic_build::configure().file_descriptor_set_path(...).compile_protos(...)`. The shorthand does not support `file_descriptor_set_path`; the builder pattern is required for tonic-reflection support.

**src/grpc/mod.rs** — Added four helper items:
- `FILE_DESCRIPTOR_SET` constant loaded via `tonic::include_file_descriptor_set!("mnemonic_descriptor")`
- `api_error_to_status()` mapping all 5 `ApiError` variants to corresponding `tonic::Status` codes (InvalidArgument, NotFound, PermissionDenied, Unauthenticated, Internal)
- `enforce_scope()` duplicated from `src/server.rs` — identical logic for scope enforcement from scoped API keys
- `memory_to_proto()` converting `crate::service::Memory` to `proto::Memory` with RFC 3339 `created_at` parsed via `prost_types::Timestamp::from_str`

### Task 2: Implement all 4 RPC handlers and wire reflection in serve_grpc

**src/grpc/mod.rs** — Replaced all 4 unimplemented stubs with real implementations:

- **store_memory**: Extracts `AuthContext` from request extensions, validates non-empty content, calls `enforce_scope`, delegates to `MemoryService::create_memory`, returns `StoreMemoryResponse` with converted proto Memory.

- **search_memories**: Validates non-empty query, enforces scope, maps proto `SearchMemoriesRequest` fields to `SearchParams`, delegates to `MemoryService::search_memories`, casts `item.distance` from `f64` to `f32` for proto `SearchResult`.

- **list_memories**: Enforces scope (no required fields), maps to `ListParams` with limit/offset pagination, delegates to `MemoryService::list_memories`, casts `total` from `u64` to `i32`.

- **delete_memory**: Validates non-empty id, for scoped keys calls `get_memory_agent_id` to verify ownership before deletion, delegates to `MemoryService::delete_memory`.

**serve_grpc()** — Wired tonic-reflection service with `FILE_DESCRIPTOR_SET` before the `Server::builder()` chain, added `.add_service(reflection_service)` between health and main service.

**src/grpc/auth.rs** — Added reflection path bypass after the health check bypass so `grpcurl list` works when auth is active.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 9f7ecf2 | feat(28-01): build.rs reflection descriptor and grpc helper functions |
| 2 | cf4891d | feat(28-01): implement all 4 RPC handlers and wire tonic-reflection |

## Verification

All success criteria met:
- `cargo build --features interface-grpc` exits 0 (3 dead-code warnings on unused struct fields — pre-existing, not introduced here)
- `cargo test --features interface-grpc` exits 0 — 54 unit tests + 6 gRPC auth tests pass
- `grep -c "Status::unimplemented" src/grpc/mod.rs` returns 0 — all stubs replaced
- `api_error_to_status`, `enforce_scope`, `memory_to_proto`, `FILE_DESCRIPTOR_SET` all present in grpc/mod.rs
- `reflection_service` wired in `serve_grpc()`
- `"/grpc.reflection.v1.ServerReflection/"` bypass present in auth.rs

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None. All handlers delegate to real MemoryService implementations.

## Self-Check: PASSED

- `build.rs` exists and contains `file_descriptor_set_path` — FOUND
- `src/grpc/mod.rs` exists with all 4 handler implementations — FOUND
- `src/grpc/auth.rs` exists with reflection bypass — FOUND
- Commit 9f7ecf2 exists — FOUND
- Commit cf4891d exists — FOUND
