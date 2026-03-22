---
phase: 24
slug: postgres-backend
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-21
updated: 2026-03-21
---

# Phase 24 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (workspace root) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Postgres-specific run** | `cargo test --features backend-postgres --lib storage::postgres::tests` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 24-01-01 | 01 | 1 | PGVR-01 | build + unit | `cargo test --features backend-postgres --lib storage::postgres::tests::test_pgvr01_postgres_backend_has_storage_backend_impl` | ✅ | ✅ green |
| 24-01-02 | 01 | 1 | PGVR-02 | unit | `cargo test --features backend-postgres --lib storage::postgres::tests::test_pgvr02_search_sql_contains_cosine_distance_operator` | ✅ | ✅ green |
| 24-01-03 | 01 | 1 | PGVR-03 | unit | `cargo test --features backend-postgres --lib storage::postgres::tests::test_pgvr03_write_compaction_result_transaction_sql_structure` | ✅ | ✅ green |
| 24-01-04 | 01 | 1 | PGVR-04 | unit | `cargo test --features backend-postgres --lib storage::postgres::tests::test_pgvr04_all_query_methods_namespace_isolated_by_agent_id` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Tests Added (Nyquist Gap-Fill)

All 4 tests added to `src/storage/postgres.rs` in the `#[cfg(test)] mod tests` block.
Run all postgres tests: `cargo test --features backend-postgres --lib storage::postgres::tests`

| Test Name | Requirement | What it Verifies |
|-----------|-------------|-----------------|
| `test_pgvr01_postgres_backend_has_storage_backend_impl` | PGVR-01 | Compile-time proof that `PostgresBackend: StorageBackend` (all 7 methods present) |
| `test_pgvr02_search_sql_contains_cosine_distance_operator` | PGVR-02 | search() SQL contains `embedding <=> $1::vector AS distance` and `ORDER BY distance ASC` |
| `test_pgvr02_search_threshold_uses_cosine_distance_in_sql` | PGVR-02 | threshold filter uses `embedding <=> $1::vector <= $N` in SQL (not post-filtering) |
| `test_pgvr03_write_compaction_result_transaction_sql_structure` | PGVR-03 | INSERT SQL uses `$7::timestamptz`, `$8::vector`; DELETE uses `id = ANY($1)` |
| `test_pgvr03_transaction_api_compiles` | PGVR-03 | `pool.begin()` / `tx.commit()` compiles with sqlx PgPool (type-level) |
| `test_pgvr04_list_where_clause_includes_agent_id` | PGVR-04 | list() WHERE clause builder produces `agent_id = $1` |
| `test_pgvr04_search_where_clause_includes_agent_id` | PGVR-04 | search() WHERE clause builder produces `agent_id = $2` (embedding at $1) |
| `test_pgvr04_fetch_candidates_sql_includes_agent_id_where` | PGVR-04 | fetch_candidates() literal SQL contains `WHERE agent_id = $1` |
| `test_pgvr04_all_query_methods_namespace_isolated_by_agent_id` | PGVR-04 | Summary: all 3 query methods reference agent_id in their conditions |

Total postgres tests: 11 (2 pre-existing + 9 new). All pass.

---

## Wave 0 Requirements

- [x] `src/storage/postgres.rs` — module with `#[cfg(feature = "backend-postgres")]` gate
- [x] Unit tests for PGVR-01 through PGVR-04 (9 tests added)
- [x] Integration test documentation for docker-based Postgres setup (see Manual-Only Verifications)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Atomicity under crash | PGVR-03 | Requires process kill mid-transaction | `docker run -e POSTGRES_PASSWORD=test -p 5432:5432 pgvector/pgvector:pg17`, start compaction, kill process, verify DB consistency via psql |
| Full end-to-end runtime | PGVR-01 | Requires live Postgres+pgvector | `POSTGRES_URL=postgres://postgres:test@localhost/mnemonic cargo run --features backend-postgres` |

---

## Validation Sign-Off

- [x] All tasks have automated verify commands
- [x] Sampling continuity: all 4 requirements have named passing tests
- [x] Wave 0 covers all MISSING references (PGVR-01 through PGVR-04)
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** complete

---

## Validation Audit 2026-03-21

| Metric | Count |
|--------|-------|
| Gaps found | 4 |
| Resolved | 4 |
| Escalated | 0 |
