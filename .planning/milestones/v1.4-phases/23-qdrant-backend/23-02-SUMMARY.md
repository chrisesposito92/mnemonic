---
phase: 23-qdrant-backend
plan: 02
subsystem: storage
tags: [qdrant, backend, feature-flag, storage-backend, grpc, list, search, compaction]
dependency_graph:
  requires:
    - Phase 23 Plan 01 (QdrantBackend struct, store/get_by_id/delete, helpers)
    - qdrant-client 1.17 (ScrollPointsBuilder, QueryPointsBuilder, CountPointsBuilder, VectorsOutput)
  provides:
    - Complete QdrantBackend with all 7 StorageBackend methods implemented
    - list() with scroll API, client-side sort, accurate count
    - search() with query API, score-to-distance conversion, threshold filtering
    - fetch_candidates() with with_vectors(true) and truncation detection
    - write_compaction_result() with upsert-first-then-delete order
  affects:
    - src/storage/qdrant.rs (all 4 remaining stub methods replaced)
tech_stack:
  added:
    - ScrollPointsBuilder (qdrant-client scroll API)
    - QueryPointsBuilder (qdrant-client query API with Vec<f32> -> Query conversion)
    - CountPointsBuilder (exact count for list() total)
    - VectorsOutput.get_vector() / vector_output::Vector::Dense for embedding extraction
  patterns:
    - Client-side sort after scroll for created_at DESC ordering
    - Double build_filter() call (scroll + count) for accurate total in list()
    - score_to_distance (1.0 - score) applied in search() before threshold filter
    - extract_vector_from_point() helper: RetrievedPoint -> Vec<f32> via VectorsOutput
    - Upsert-then-delete order in write_compaction_result() (non-atomic, documented)
    - tracing::warn! on partial failure in write_compaction_result()
key_files:
  created: []
  modified:
    - src/storage/qdrant.rs
decisions:
  - "list() calls build_filter() twice to get separate filter instances for scroll and count — Qdrant filter values are not Clone in general, so the builder is called twice with same params"
  - "CountPointsBuilder.exact(true) used for accurate total count in list() — matches SqliteBackend COUNT(*) semantics"
  - "Vec<f32> implements Into<Query> directly via qdrant_client::qdrant_client::conversions::query module — no DenseVector wrapping needed"
  - "VectorsOutput.get_vector() returns Option<vector_output::Vector> — must match Dense variant to get Vec<f32> data"
  - "extract_vector_from_point() added as private helper for fetch_candidates() — reusable if future methods need vector extraction from RetrievedPoint"
metrics:
  duration: 162s
  completed: "2026-03-21"
  tasks_completed: 2
  files_modified: 1
requirements-completed: [QDRT-01, QDRT-02, QDRT-03, QDRT-04]
---

# Phase 23 Plan 02: QdrantBackend Remaining Methods Summary

**One-liner:** All 7 StorageBackend methods implemented on QdrantBackend — list() with scroll+count, search() with query API and score conversion, fetch_candidates() with embeddings, write_compaction_result() with upsert-first order.

## What Was Built

One file changed to complete the QdrantBackend implementation:

**`src/storage/qdrant.rs`** (all 4 remaining stubs replaced):

- `list()` — Uses `ScrollPointsBuilder` with payload filter (agent_id, session_id, tag, date range), fetches `offset+limit+1` points, sorts client-side by `created_at DESC` (ISO 8601 lexicographic sort), applies integer offset/limit slicing, and calls `CountPointsBuilder` with `exact(true)` for accurate total count. Builds the filter twice (scroll + count) to get two separate filter instances.

- `search()` — Uses `QueryPointsBuilder` with the pre-computed `Vec<f32>` embedding (which implements `Into<Query>` natively), applies payload filter natively during search (no over-fetch needed per D-17), maps `ScoredPoint.score` through `score_to_distance()` for lower-is-better distance semantics, then filters by threshold.

- `fetch_candidates()` — Uses `ScrollPointsBuilder` with `agent_id` filter and `with_vectors(true)`, over-fetches by `max_candidates+1` for truncation detection (per D-20), sorts by `created_at DESC` client-side (per D-21), and extracts embeddings via the new `extract_vector_from_point()` helper.

- `write_compaction_result()` — Upserts the merged memory point first (safe on failure), then deletes source points in a separate API call (non-atomic, per D-11). Contains doc comment stating "NOT atomic" and logs a `tracing::warn!` if deletion fails after upsert (per D-12).

**New helper function added:**

- `extract_vector_from_point(pt: &RetrievedPoint) -> Result<Vec<f32>, ApiError>` — Extracts the default dense vector from a `RetrievedPoint` by accessing `pt.vectors` (Option<VectorsOutput>), calling `.get_vector()` to get the default unnamed vector, and matching the `Vector::Dense` variant to get the `Vec<f32>` data field.

**New imports added:**

- `ScrollPointsBuilder`, `QueryPointsBuilder`, `CountPointsBuilder`, `RetrievedPoint` from `qdrant_client::qdrant`
- `Vector` from `qdrant_client::qdrant::vector_output`

## Verification Results

All plan verification criteria passed:

| Check | Result |
|-------|--------|
| `grep -c "todo!" src/storage/qdrant.rs` | 0 — all stubs removed |
| `cargo check --features backend-qdrant` | Passed — compiles with 0 errors |
| `cargo test` (no features) | Passed — 54/54 tests pass |
| `cargo test --features backend-qdrant --lib storage::qdrant::tests` | Passed — 10/10 unit tests |
| `cargo build --features backend-qdrant` | Passed — binary produced |
| `grep "score_to_distance" src/storage/qdrant.rs` | Found in search() at line 542 |
| `grep "upsert_points" src/storage/qdrant.rs` | Found at line 674, before delete_points at 686 |
| `grep "with_vectors(true)" src/storage/qdrant.rs` | Found in fetch_candidates() at line 602 |

## Deviations from Plan

None - plan executed exactly as written.

The plan's template code required minor adjustments:
- The plan's starter code used `scroll_result.result.len()` for total counting. The executor used `CountPointsBuilder.exact(true)` (the preferred approach also described in the plan's "IMPORTANT NOTE ON TOTAL COUNT" section) — this is the plan's own recommended implementation, not a deviation.
- The plan's `extract_vector_from_point` function signature referenced `qdrant_client::qdrant::RetrievedPoint` — the actual type path used in imports is `qdrant_client::qdrant::RetrievedPoint` and `qdrant_client::qdrant::vector_output::Vector` for the match variant. This matches exactly what the crate exposes.

## Known Stubs

None — all 7 StorageBackend trait methods are fully implemented with no `todo!()` remaining.

## Self-Check: PASSED

- src/storage/qdrant.rs: FOUND
- Commit 5c25d8f (feat(23-02): implement list() and search()): FOUND
- Commit abc391a (feat(23-02): implement fetch_candidates() and write_compaction_result()): FOUND
