---
phase: 28-core-rpc-handlers-health-discoverability
plan: "02"
subsystem: grpc
tags: [grpc, tonic, testing, integration-tests, scope-enforcement, health, reflection]
dependency_graph:
  requires:
    - 28-01 (all 4 RPC handler implementations, FILE_DESCRIPTOR_SET, enforce_scope)
    - src/grpc/mod.rs (MnemonicGrpcService, MnemonicService trait, proto module)
    - src/grpc/auth.rs (AuthContext, test_key_service pattern)
    - src/service.rs (MemoryService, CreateMemoryRequest, SearchParams, ListParams)
    - tests/integration.rs (MockEmbeddingEngine pattern reference)
  provides:
    - tests/grpc_integration.rs with 14 integration tests
    - Per-handler scope enforcement coverage (STATE.md critical research flag resolved)
    - Health and reflection smoke tests
  affects:
    - src/grpc/mod.rs (FILE_DESCRIPTOR_SET visibility: pub(crate) -> pub)
tech_stack:
  added: []
  patterns:
    - MockEmbeddingEngine with deterministic hash vectors (mirrors integration.rs pattern)
    - Direct handler call pattern via MnemonicService trait (no TCP listener needed)
    - AuthContext injection via tonic::Request extensions_mut()
    - CompactionService::new(backend, audit_db, embedding, summarization, embedding_model)
key_files:
  created:
    - tests/grpc_integration.rs
  modified:
    - src/grpc/mod.rs
decisions:
  - "FILE_DESCRIPTOR_SET changed from pub(crate) to pub — required for test file access outside the crate"
  - "Used MockEmbeddingEngine (hash-based, no model download) instead of LocalEngine::new() — faster tests, no HuggingFace Hub dependency"
  - "CompactionService::new takes (backend, audit_db, embedding, summarization, embedding_model) — plan's suggested (memory_service, None) was incorrect"
metrics:
  duration: "~10 minutes"
  completed_date: "2026-03-22"
  tasks_completed: 2
  files_modified: 2
---

# Phase 28 Plan 02: Integration Tests for gRPC Handlers Summary

**One-liner:** 14 integration tests covering all 4 gRPC handler happy paths, input validation errors, per-handler scope enforcement (PermissionDenied), and health/reflection smoke tests.

## What Was Built

### Task 1: Handler integration tests (happy path + error cases)

**tests/grpc_integration.rs** — Created with `#![cfg(feature = "interface-grpc")]` gate. Contains:

- **Test harness** — `test_grpc_service()` builds `MnemonicGrpcService` with in-memory SQLite and `MockEmbeddingEngine` (hash-based, 384 dims, no model download). Mirrors `build_test_state()` from `tests/integration.rs`.
- **`request_with_auth()`** helper — injects `AuthContext` into `tonic::Request` extensions to simulate `GrpcAuthLayer` behavior.

**Handler happy-path tests (4 tests):**
- `test_grpc_store_memory` — StoreMemory returns memory with non-empty ID, correct content and agent_id
- `test_grpc_search_memories` — SearchMemories returns non-empty results with non-negative float distance
- `test_grpc_list_memories` — ListMemories returns memories with positive total count
- `test_grpc_delete_memory` — DeleteMemory returns deleted memory with matching ID

**Input validation tests (3 tests):**
- `test_grpc_store_memory_empty_content` — whitespace-only content returns `Code::InvalidArgument`, message mentions "content"
- `test_grpc_search_memories_empty_query` — empty query returns `Code::InvalidArgument`
- `test_grpc_delete_memory_empty_id` — empty id returns `Code::InvalidArgument`

**Error case tests (1 test):**
- `test_grpc_delete_memory_not_found` — non-existent ID returns `Code::NotFound`

### Task 2: Per-handler scope enforcement tests and health/reflection smoke tests

Scope enforcement tests (4 tests — STATE.md critical research flag resolved):
- `test_grpc_store_memory_scope_enforcement` — scoped key with mismatched agent_id returns PermissionDenied; error message includes both allowed and requested agent IDs
- `test_grpc_search_memories_scope_enforcement` — search with mismatched agent_id returns PermissionDenied
- `test_grpc_list_memories_scope_enforcement` — list with mismatched agent_id returns PermissionDenied (catches "Pitfall 4" from RESEARCH.md — easy to miss)
- `test_grpc_delete_memory_scope_enforcement` — delete of a memory owned by a different agent returns PermissionDenied (uses D-08 ownership lookup pattern)

Health and reflection smoke tests (2 tests):
- `test_grpc_health_serving` — `health_reporter.set_serving::<MnemonicServiceServer<MnemonicGrpcService>>()` does not panic
- `test_grpc_reflection_builds` — `FILE_DESCRIPTOR_SET` is non-empty and tonic-reflection service builds from it

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1+2  | be03896 | test(28-02): handler integration tests (happy path and error cases) |

Note: Both tasks were written in one pass since they share the same file. The commit includes all 14 tests.

## Verification

All success criteria met:
- `cargo test --features interface-grpc -- test_grpc_` — 14 tests pass
- `cargo test --features interface-grpc` — full suite: 223 tests passing, 1 ignored, 0 failed
- `grep "scope_enforcement" tests/grpc_integration.rs` — returns 4 functions
- `grep "health_serving\|reflection_builds" tests/grpc_integration.rs` — returns 2 functions
- 396 lines in tests/grpc_integration.rs (well above 200-line minimum)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] FILE_DESCRIPTOR_SET visibility widened from pub(crate) to pub**
- **Found during:** Task 1 (first compile attempt)
- **Issue:** `error[E0603]: constant FILE_DESCRIPTOR_SET is private` — test file is outside the crate, cannot access `pub(crate)` items
- **Fix:** Changed visibility from `pub(crate)` to `pub` in `src/grpc/mod.rs`
- **Files modified:** `src/grpc/mod.rs` (line 17)
- **Commit:** be03896

**2. [Rule 1 - Bug] CompactionService::new signature mismatch in plan's suggested harness**
- **Found during:** Task 1 (reading source code)
- **Issue:** Plan suggested `CompactionService::new(Arc::clone(&memory_service), None)` (2 args) but actual signature is `(backend, audit_db, embedding, summarization, embedding_model)` (5 args)
- **Fix:** Used correct 5-argument constructor matching `tests/integration.rs` `build_test_state()` pattern
- **Files modified:** `tests/grpc_integration.rs` (test harness constructor)
- **Commit:** be03896

## Known Stubs

None. All tests use real MemoryService implementations backed by in-memory SQLite.

## Self-Check: PASSED
