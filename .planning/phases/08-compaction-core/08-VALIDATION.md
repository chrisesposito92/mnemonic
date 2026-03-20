---
phase: 8
slug: compaction-core
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-20
audited: 2026-03-20
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
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
| 08-01-01 | 01 | 1 | DEDUP-01 | unit | `cargo test --lib -- compaction::tests` | ✅ src/compaction.rs | ✅ green |
| 08-01-02 | 01 | 1 | DEDUP-02 | unit | `cargo test --lib -- compaction::tests` | ✅ src/compaction.rs | ✅ green |
| 08-01-03 | 01 | 1 | DEDUP-03 | unit | `cargo test --lib -- compaction::tests` | ✅ src/compaction.rs | ✅ green |
| 08-01-04 | 01 | 1 | DEDUP-04 | unit | `cargo test --lib -- compaction::tests` | ✅ src/compaction.rs | ✅ green |
| 08-02-01 | 02 | 2 | DEDUP-01,02,03,04 | integration | `cargo test -- test_compact` | ✅ tests/integration.rs | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Test Inventory

### Unit Tests (src/compaction.rs — 10 tests)

| Test | Requirement | Verifies |
|------|-------------|----------|
| `test_cosine_similarity_identical` | DEDUP-01 | Identical vectors → 1.0 |
| `test_cosine_similarity_orthogonal` | DEDUP-01 | Orthogonal vectors → 0.0 |
| `test_cosine_similarity_opposite` | DEDUP-01 | Opposite vectors → -1.0 |
| `test_tier1_concat_chronological_order` | DEDUP-02 | Content joined chronologically ascending |
| `test_union_tags_dedup` | DEDUP-02 | Tag union with deduplication |
| `test_cluster_two_similar` | DEDUP-01 | Above-threshold → 1 cluster |
| `test_cluster_below_threshold` | DEDUP-01 | Below-threshold → 0 clusters |
| `test_cluster_first_match` | DEDUP-01 | Greedy first-match assignment |
| `test_cluster_both_assigned_skip` | DEDUP-01 | Both-assigned pairs skipped |
| `test_empty_candidates` | DEDUP-01 | Empty input → 0 pairs, 0 clusters |

### Integration Tests (tests/integration.rs — 12 compaction tests)

| Test | Requirement | Verifies |
|------|-------------|----------|
| `test_compact_runs_exists` | Audit | compact_runs table DDL |
| `test_compact_runs_agent_id_index` | Audit | Agent ID index on compact_runs |
| `test_compact_atomic_write` | DEDUP-01,02,03 | Sources deleted, merged memory with correct tags/created_at |
| `test_compact_dry_run` | DEDUP-01,03 | Cluster preview without data modification |
| `test_compact_no_clusters` | DEDUP-01 | Different content → no clusters formed |
| `test_compact_agent_isolation` | DEDUP-03 | Agent A compaction doesn't affect Agent B |
| `test_compact_max_candidates_truncation` | DEDUP-04 | truncated=true when candidates exceed cap |
| `test_compact_with_mock_summarizer` | DEDUP-02 | Tier 2 LLM path produces MOCK_SUMMARY content |
| `test_compact_http_basic` | DEDUP-01,03 | POST /compact endpoint returns valid response |
| `test_compact_http_dry_run` | DEDUP-01,03 | HTTP dry_run endpoint returns preview |
| `test_compact_http_agent_isolation` | DEDUP-03 | HTTP endpoint respects agent namespace |
| `test_compact_http_validation` | All | HTTP validation rejects bad requests |

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 15s (actual: ~8s)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** complete

---

## Validation Audit 2026-03-20

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

All 4 DEDUP requirements (DEDUP-01 through DEDUP-04) are fully covered by 10 unit tests and 12 integration tests. Full test suite: 34 passed, 0 failed, 1 ignored.
