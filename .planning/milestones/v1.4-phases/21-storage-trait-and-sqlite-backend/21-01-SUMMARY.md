---
phase: 21-storage-trait-and-sqlite-backend
plan: "01"
subsystem: storage
tags: [storage, trait, sqlite, abstraction, refactor]
dependency_graph:
  requires: []
  provides: [StorageBackend trait, SqliteBackend, StoreRequest, CandidateRecord, MergedMemoryRequest]
  affects: [src/storage/mod.rs, src/storage/sqlite.rs, src/lib.rs, src/main.rs]
tech_stack:
  added: []
  patterns: [async_trait, Arc<dyn Trait>, tokio-rusqlite db.call]
key_files:
  created:
    - src/storage/mod.rs
    - src/storage/sqlite.rs
  modified:
    - src/lib.rs
    - src/main.rs
decisions:
  - "StorageBackend uses #[async_trait] for dyn-compatibility (native async fn in traits not dyn-compatible in early 2026)"
  - "write_compaction_result() replaces separate insert_merged_memory + delete_source_memories methods to preserve atomicity at the backend layer"
  - "ApiError reused as trait return type (no new StorageError type) — consistent with existing service.rs/compaction.rs patterns"
  - "Distances passed through from sqlite-vec as lower-is-better per D-02 — no conversion needed for SQLite backend"
metrics:
  duration: "~8 minutes"
  completed: "2026-03-21"
  tasks: 2
  files: 4
requirements-completed: [STOR-01, STOR-02]
---

# Phase 21 Plan 01: Storage Trait and SQLite Backend Summary

**One-liner:** Defined `StorageBackend` async trait with 7 methods and `SqliteBackend` wrapping `Arc<Connection>`, extracting all memory SQL from service.rs and compaction.rs into a new `src/storage/` module.

## What Was Built

Created `src/storage/` module with:

- **`src/storage/mod.rs`**: `StorageBackend` async trait (7 methods), three shared input types (`StoreRequest`, `CandidateRecord`, `MergedMemoryRequest`), re-export of `SqliteBackend`.
- **`src/storage/sqlite.rs`**: `SqliteBackend` struct wrapping `Arc<Connection>` with full implementation of all 7 `StorageBackend` methods. All SQL extracted verbatim from `service.rs` and `compaction.rs` with zero behavioural changes.
- **`src/lib.rs`**: Added `pub mod storage;` (alphabetical, after `server`).
- **`src/main.rs`**: Added `mod storage;` (alphabetical, after `server`).

### StorageBackend Trait Methods

| Method | SQL source | Purpose |
|--------|-----------|---------|
| `store()` | service.rs:115-133 | Atomic INSERT memories + vec_memories |
| `get_by_id()` | service.rs:316-333 | SELECT by ID, returns Option<Memory> |
| `list()` | service.rs:245-289 | Filtered count + paginated results |
| `search()` | service.rs:172-216 | KNN via sqlite-vec + threshold filter |
| `delete()` | service.rs:313-347 | SELECT then atomic DELETE both tables |
| `fetch_candidates()` | compaction.rs:187-216 | JOIN + embedding bytes + over-fetch |
| `write_compaction_result()` | compaction.rs:344-398 | Atomic INSERT merged + DELETE sources |

## Verification

- `cargo build`: Finished with zero errors (warnings are pre-existing dead_code for now-unused SqliteBackend::new)
- `cargo test --lib storage`: 4 tests pass (2 in mod.rs, 2 in sqlite.rs — compile-time Send+Sync and dyn-compatibility proofs)
- `cargo test --lib`: 67 tests pass, zero regressions from adding module declarations

## Deviations from Plan

### Auto-fixed Issues

None - plan executed exactly as written.

**One design clarification:** The plan's unit test spec for `test_storage_backend_send_sync` mentioned testing `dyn StorageBackend` directly for Send+Sync, which is not possible (bare `dyn Trait` doesn't implement traits). The implementation uses `Arc<dyn StorageBackend>` as the argument type to `_takes_backend()`, which is the correct compile-time proof pattern (same as `test_both_engines_as_trait_object` in embedding.rs as the plan referenced).

## Known Stubs

None — this plan only creates new files and module declarations. No data flows through `SqliteBackend` until Plan 02 wires `MemoryService` and `CompactionService` to use `Arc<dyn StorageBackend>`.

## Commits

| Hash | Description |
|------|-------------|
| 502638d | feat(21-01): define StorageBackend trait and shared input types |
| a9fa303 | feat(21-01): implement SqliteBackend with all StorageBackend methods |

## Self-Check: PASSED

- FOUND: src/storage/mod.rs
- FOUND: src/storage/sqlite.rs
- FOUND: commit 502638d (feat: define StorageBackend trait and shared input types)
- FOUND: commit a9fa303 (feat: implement SqliteBackend with all StorageBackend methods)
