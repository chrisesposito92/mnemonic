---
phase: 3
slug: service-and-api
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-19
validated: 2026-03-19
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (already configured) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~8 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 8 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | API-01 | integration | `cargo test test_post_memory` | ✅ | ✅ green |
| 03-01-02 | 01 | 1 | API-06 | integration | `cargo test test_post_memory_validation` | ✅ | ✅ green |
| 03-02-01 | 02 | 1 | API-02, AGNT-03 | integration | `cargo test test_search_memories` | ✅ | ✅ green |
| 03-02-02 | 02 | 1 | API-03 | integration | `cargo test test_list_memories` | ✅ | ✅ green |
| 03-03-01 | 03 | 2 | API-04 | integration | `cargo test test_delete_memory` | ✅ | ✅ green |
| 03-03-02 | 03 | 2 | AGNT-01, AGNT-02 | integration | `cargo test test_agent_isolation` | ✅ | ✅ green |
| 03-03-03 | 03 | 2 | API-05 | integration | `cargo test test_health` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `tests/integration.rs` — integration tests for all API endpoints (11 tests)
- [x] `src/service.rs` — MemoryService struct with all four CRUD+search methods
- [x] MockEmbeddingEngine in test helpers for fast API tests without model loading

*All Wave 0 items delivered in Plan 03 execution.*

---

## Coverage Detail

| Requirement | Tests Covering | Verified Behavior |
|-------------|---------------|-------------------|
| API-01 | test_post_memory, test_post_memory_validation | POST /memories 201, content validation |
| API-02 | test_search_memories, test_search_agent_filter | Semantic search with distance, agent filter |
| API-03 | test_list_memories | Paginated list with total count, agent filter |
| API-04 | test_delete_memory, test_delete_not_found | DELETE 200 with object, 404 for missing |
| API-05 | test_health | GET /health 200 {status: ok} |
| API-06 | test_post_memory_validation, test_search_missing_q, test_delete_not_found | 400/404 JSON error bodies |
| AGNT-01 | test_agent_isolation, test_list_memories | Agent-scoped memory retrieval |
| AGNT-02 | test_session_filter | Session-scoped list filtering |
| AGNT-03 | test_search_agent_filter, test_agent_isolation | Agent-scoped search and isolation |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Memory persists across server restarts | API-01 | Requires process restart | Store memory, stop server, restart, verify GET returns it |

*All other behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s (actual: ~8s)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** complete

---

## Validation Audit 2026-03-19

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |
| Total tests covering phase | 11 |
| Requirements covered | 9/9 |
