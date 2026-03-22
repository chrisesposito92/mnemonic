---
phase: 23-qdrant-backend
plan: 01
subsystem: storage
tags: [qdrant, backend, feature-flag, storage-backend, grpc]
dependency_graph:
  requires:
    - Phase 21 (StorageBackend trait definition in src/storage/mod.rs)
    - Phase 22 (Config.qdrant_url, Config.qdrant_api_key fields, create_backend() factory)
  provides:
    - QdrantBackend struct with store(), get_by_id(), delete() methods
    - score_to_distance() and build_filter() helpers
    - point_to_memory() and payload extraction helpers
    - Feature-gated Cargo dependency wiring (backend-qdrant -> qdrant-client)
  affects:
    - src/storage/mod.rs (module declaration, re-export, factory wiring)
    - Cargo.toml (qdrant-client, prost-types optional dependencies)
tech_stack:
  added:
    - qdrant-client = "1" (optional, gated by backend-qdrant feature)
    - prost-types = "0.13" (optional, gated by backend-qdrant feature, for DatetimeRange Timestamp)
  patterns:
    - #[cfg(feature = "backend-qdrant")] conditional compilation throughout
    - Builder pattern for Qdrant client construction (Qdrant::from_url().api_key().build())
    - CreateCollectionBuilder + VectorParamsBuilder for collection schema
    - CreateFieldIndexCollectionBuilder for payload indexing
    - serde_json::json!(...).try_into::<Payload>() for payload construction
    - PointStruct::new(uuid_string, vec_f32, payload) for point construction
    - score_to_distance: 1.0 - score for lower-is-better distance semantics
    - Julian Day Number algorithm for ISO 8601 <-> Unix epoch conversion without chrono
key_files:
  created:
    - src/storage/qdrant.rs
  modified:
    - Cargo.toml
    - src/storage/mod.rs
decisions:
  - "Added prost-types as optional dep under backend-qdrant feature to enable DatetimeRange Timestamp construction (prost_types not re-exported by qdrant_client)"
  - "api_key() takes impl AsOptionApiKey — pass key.as_str() not &String directly"
  - "Payload::try_from() returns QdrantError, not serde_json::Error — error mapping updated"
  - "Value::as_list() returns &[Value] slice, not ListValue struct — no .values field access"
  - "Implemented now_iso8601() and iso8601_to_epoch() using JDN algorithm to avoid adding chrono as a dependency"
metrics:
  duration: 580s
  completed: "2026-03-21"
  tasks_completed: 3
  files_modified: 3
requirements-completed: [QDRT-01]
---

# Phase 23 Plan 01: QdrantBackend Foundation Summary

**One-liner:** QdrantBackend struct with gRPC client construction, collection auto-creation, store/get_by_id/delete CRUD, score-to-distance conversion, and payload filter building — wired into the backend-qdrant feature flag.

## What Was Built

Three files changed to lay the QdrantBackend foundation:

**`src/storage/qdrant.rs`** (new file, 543 lines):
- `QdrantBackend { client: Qdrant, collection: String }` struct
- `QdrantBackend::new(config: &Config)` — connects to Qdrant via gRPC, calls `ensure_collection()`
- `ensure_collection()` — idempotently creates `mnemonic_memories` collection with 384-dim cosine vectors and keyword indexes on `agent_id`, `session_id`, `tags`
- `store()` — upserts a PointStruct with UUID string ID, embedding vector, and full payload
- `get_by_id()` — retrieves point by UUID string ID using GetPointsBuilder, returns `Option<Memory>`
- `delete()` — fetch-then-delete pattern (required to return the deleted Memory)
- `list()`, `search()`, `fetch_candidates()`, `write_compaction_result()` — stubbed with `todo!("Implemented in Plan 02")`
- `score_to_distance(score: f32) -> f64` — `1.0 - score` conversion (per D-08)
- `build_filter()` — builds `Filter::must()` with matches conditions for agent/session/tag and `DatetimeRange` conditions for after/before
- `get_payload_string()`, `get_payload_string_list()`, `point_to_memory()` — payload extraction helpers
- `now_iso8601()`, `iso8601_to_epoch()` — time utilities using Julian Day Number algorithm (no chrono dependency)
- 10 unit tests covering score conversion, filter building, timestamp formatting

**`Cargo.toml`** (2 additions):
- `backend-qdrant = ["dep:qdrant-client", "dep:prost-types"]`
- `qdrant-client = { version = "1", optional = true }`
- `prost-types = { version = "0.13", optional = true }`

**`src/storage/mod.rs`** (7 line delta):
- Added `#[cfg(feature = "backend-qdrant")] pub mod qdrant;`
- Added `#[cfg(feature = "backend-qdrant")] pub use qdrant::QdrantBackend;`
- Replaced `todo!("QdrantBackend construction")` with `qdrant::QdrantBackend::new(config).await?`

## Verification Results

All plan verification criteria passed:

| Check | Result |
|-------|--------|
| `cargo check` (no features) | Passed — default binary unchanged |
| `cargo check --features backend-qdrant` | Passed — QdrantBackend compiles |
| `cargo test` (no features) | Passed — all existing tests pass unchanged |
| `cargo test --features backend-qdrant --lib storage::qdrant::tests` | Passed — 10/10 unit tests |
| `grep "todo!" src/storage/mod.rs` | Only postgres arm — qdrant arm wired |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Added prost-types as optional dependency**
- **Found during:** Task 2 - compilation with backend-qdrant feature
- **Issue:** `prost_types::Timestamp` used in `DatetimeRange` construction but `prost_types` is a transitive dependency of `qdrant-client` that Rust doesn't make directly importable without an explicit declaration
- **Fix:** Added `prost-types = { version = "0.13", optional = true }` to `[dependencies]` and `"dep:prost-types"` to the `backend-qdrant` feature array
- **Files modified:** Cargo.toml
- **Commit:** 8364b0b

**2. [Rule 1 - Bug] Fixed api_key() call to pass &str not &String**
- **Found during:** Task 2 - first compilation attempt
- **Issue:** `builder.api_key(key)` where `key: &String` doesn't satisfy `AsOptionApiKey` trait — the crate implements it for `&str` but not `&String`
- **Fix:** Changed to `builder.api_key(key.as_str())`
- **Files modified:** src/storage/qdrant.rs
- **Commit:** 8364b0b

**3. [Rule 1 - Bug] Fixed Payload::try_into() error type**
- **Found during:** Task 2 - compilation
- **Issue:** `Payload::try_from(serde_json::Value)` returns `QdrantError`, not `serde_json::Error`; the error type annotation in the closure was wrong
- **Fix:** Changed error type annotation to `qdrant_client::QdrantError`
- **Files modified:** src/storage/qdrant.rs
- **Commit:** 8364b0b

**4. [Rule 1 - Bug] Fixed Value::as_list() return type**
- **Found during:** Task 2 - compilation
- **Issue:** `Value::as_list()` returns `Option<&[Value]>` (a slice), not `Option<ListValue>` — accessing `.values` field on a slice doesn't work
- **Fix:** Rewrote `get_payload_string_list()` to iterate over the slice directly with `items.iter().filter_map(...)`
- **Files modified:** src/storage/qdrant.rs
- **Commit:** 8364b0b

## Known Stubs

The following trait methods are intentionally stubbed for Plan 02:

| Method | File | Reason |
|--------|------|--------|
| `list()` | src/storage/qdrant.rs | Plan 02 — uses scroll API pagination |
| `search()` | src/storage/qdrant.rs | Plan 02 — uses query API with vector search |
| `fetch_candidates()` | src/storage/qdrant.rs | Plan 02 — uses scroll with with_vectors=true |
| `write_compaction_result()` | src/storage/qdrant.rs | Plan 02 — upsert-then-delete compaction |

These stubs do not prevent the plan's goal — Plan 01's goal is the foundation (struct + construction + store/get/delete). Plan 02 implements the remaining 4 methods.

## Self-Check: PASSED

- src/storage/qdrant.rs: FOUND
- Cargo.toml: FOUND
- src/storage/mod.rs: FOUND
- Commit b01daf5 (chore: Cargo.toml task 1): FOUND
- Commit 8364b0b (feat: qdrant.rs task 2): FOUND
- Commit b264130 (feat: mod.rs wiring task 3): FOUND
