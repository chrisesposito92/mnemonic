---
phase: 23
slug: qdrant-backend
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-21
audited: 2026-03-21
---

# Phase 23 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (features: backend-qdrant) |
| **Quick run command** | `cargo test --features backend-qdrant --lib storage::qdrant::tests` |
| **Full suite command** | `cargo test --features backend-qdrant` |
| **Estimated runtime** | ~4 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features backend-qdrant --lib storage::qdrant::tests`
- **After every plan wave:** Run `cargo test --features backend-qdrant`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 4 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 23-01-01 | 01 | 1 | QDRT-01 | unit | `cargo test --features backend-qdrant --lib storage::qdrant::tests` | ✅ | ✅ green |
| 23-01-02 | 01 | 1 | QDRT-01, QDRT-02 | unit | `cargo test --features backend-qdrant --lib storage::qdrant::tests` | ✅ | ✅ green |
| 23-02-01 | 02 | 2 | QDRT-02, QDRT-03, QDRT-04 | unit | `cargo test --features backend-qdrant --lib storage::qdrant::tests` | ✅ | ✅ green |
| 23-02-02 | 02 | 2 | QDRT-01 | build | `cargo build --features backend-qdrant` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Automated Test Coverage (16 tests)

| Test | Requirement | Behavior Verified |
|------|-------------|-------------------|
| `test_score_to_distance_identical` | QDRT-02 | score=1.0 -> distance=0.0 |
| `test_score_to_distance_opposite` | QDRT-02 | score=-1.0 -> distance=2.0 |
| `test_score_to_distance_midpoint` | QDRT-02 | score=0.0 -> distance=1.0 |
| `test_score_to_distance_typical_similar` | QDRT-02 | score=0.85 -> distance=0.15 |
| `test_build_filter_none_when_no_params` | QDRT-04 | No params -> None filter |
| `test_build_filter_agent_id_only` | QDRT-04 | agent_id -> Some filter |
| `test_build_filter_all_params` | QDRT-04 | All params -> Some filter |
| `test_build_filter_session_id_only` | QDRT-04 | session_id -> Some filter |
| `test_build_filter_tag_only` | QDRT-04 | tag -> Some filter |
| `test_build_filter_date_range_only` | QDRT-04 | after/before -> Some filter |
| `test_now_iso8601_format` | QDRT-01 | Timestamp format YYYY-MM-DDTHH:MM:SSZ |
| `test_iso8601_epoch_origin` | QDRT-01 | 1970-01-01T00:00:00Z = epoch 0 |
| `test_iso8601_epoch_known_date` | QDRT-01 | 2026 date parses to valid epoch range |
| `test_point_to_memory_valid_payload` | QDRT-01 | Full payload -> correct Memory struct |
| `test_point_to_memory_missing_required_field` | QDRT-01 | Missing field -> ApiError |
| `test_get_payload_string_list_with_values` | QDRT-01 | List payload -> Vec<String> extraction |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Qdrant connectivity at startup | QDRT-01 | Requires running Qdrant instance | `docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant` then `MNEMONIC_STORAGE_PROVIDER=qdrant MNEMONIC_QDRANT_URL=http://localhost:6334 cargo run --features backend-qdrant` |
| Multi-agent isolation end-to-end | QDRT-04 | Requires live Qdrant with data | Store memories under two agent_ids, search as each — results must not cross |
| Compaction non-transactional safety | QDRT-03 | Requires live Qdrant with clusterable data | Run compact with multiple similar memories, verify merged result exists and sources deleted |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 20s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** complete

---

## Validation Audit 2026-03-21

| Metric | Count |
|--------|-------|
| Gaps found | 6 |
| Resolved | 6 |
| Escalated | 0 |
