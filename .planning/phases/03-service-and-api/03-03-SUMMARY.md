---
phase: 03-service-and-api
plan: 03
subsystem: tests
tags: [integration-tests, api, axum, mock-engine, tdd]
dependency_graph:
  requires: [03-02]
  provides: [api-integration-tests]
  affects: [tests/integration.rs, src/service.rs]
tech_stack:
  added: []
  patterns: [axum-oneshot-pattern, shared-test-state, mock-embedding-engine]
key_files:
  created: []
  modified:
    - tests/integration.rs
    - src/service.rs
decisions:
  - SearchParams.q made Option<String> so missing q returns 400 (service validation) not 422 (axum extractor rejection)
  - MockEmbeddingEngine uses deterministic hash-based 384-dim vectors for reproducible tests without model download
  - build_test_state() pattern: shared AppState + service Arc allows data insertion then multiple oneshot requests
metrics:
  duration: 2 min
  completed: "2026-03-19T21:40:03Z"
  tasks_completed: 2
  files_modified: 2
---

# Phase 3 Plan 3: API Integration Tests Summary

11 API integration tests using axum oneshot pattern with MockEmbeddingEngine for deterministic, model-free test execution.

## What Was Built

Extended `tests/integration.rs` with:

1. **MockEmbeddingEngine** - Returns deterministic L2-normalized 384-dim vectors via hash-based generation. No model download required; different inputs produce different vectors (correct for KNN ranking).

2. **Test infrastructure helpers**:
   - `build_test_state()` - Creates shared `AppState` + `Arc<MemoryService>` with in-memory SQLite and MockEmbeddingEngine
   - `build_test_app()` - Simple wrapper for single-request tests
   - `json_request()` - Builds `Request<Body>` with `application/json` content-type
   - `response_json()` - Deserializes axum response body to `serde_json::Value`

3. **11 integration tests** covering all 9 requirements:

| Test | Requirements | Verifies |
|------|-------------|---------|
| `test_health` | API-05 | GET /health returns 200 `{status: ok}` |
| `test_post_memory` | API-01, API-06 | POST /memories returns 201 with id, content, agent_id, session_id, tags, embedding_model, created_at |
| `test_post_memory_validation` | API-01, API-06 | empty content returns 400 with error field |
| `test_list_memories` | API-03, AGNT-01 | paginated list with total count; agent_id filter isolation |
| `test_search_memories` | API-02 | semantic search returns ranked results with distance field |
| `test_search_missing_q` | API-02, API-06 | missing q param returns 400 |
| `test_delete_memory` | API-04 | DELETE returns 200 with deleted object; subsequent GET returns total=0 |
| `test_delete_not_found` | API-04, API-06 | nonexistent id returns 404 with error field |
| `test_agent_isolation` | AGNT-01, AGNT-03 | agents only retrieve their own memories via agent_id filter |
| `test_session_filter` | AGNT-02 | session_id scopes list results to specific session |
| `test_search_agent_filter` | AGNT-03 | search with agent_id filter returns only that agent's results |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Validation] Made SearchParams.q optional for correct 400 responses**
- **Found during:** Task 2 implementation
- **Issue:** `SearchParams.q: String` (required) caused axum's `Query` extractor to return 422 Unprocessable Entity when q is absent, but the plan's `must_haves` requires 400 Bad Request
- **Fix:** Changed `q` to `Option<String>` and updated `search_memories()` to extract/validate q, returning `ApiError::BadRequest("q parameter is required")` for missing/empty values
- **Files modified:** `src/service.rs`
- **Commit:** d77ea34

## Commits

| Hash | Message |
|------|---------|
| a28f0c1 | feat(03-03): add MockEmbeddingEngine and API test infrastructure |
| d77ea34 | feat(03-03): add 11 API integration tests covering all Phase 3 requirements |

## Verification

- All 11 new tests pass (0.01s, no model download needed)
- All 5 database integration tests still pass (no regressions)
- All 10 unit tests still pass
- Tests requiring LocalEngine model download excluded from deterministic run (model tests: test_local_embedding_384_dimensions, test_local_embedding_normalized, test_semantic_similarity, test_empty_input_error, test_embed_reuse)

## Self-Check: PASSED

- tests/integration.rs: contains MockEmbeddingEngine, build_test_state, build_test_app, json_request, response_json, all 11 test functions
- src/service.rs: SearchParams.q is Option<String>
- Commits a28f0c1 and d77ea34 exist
