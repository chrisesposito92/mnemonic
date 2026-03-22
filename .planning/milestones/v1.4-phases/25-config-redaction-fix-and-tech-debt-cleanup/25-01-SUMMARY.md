---
phase: 25-config-redaction-fix-and-tech-debt-cleanup
plan: "01"
subsystem: cli+storage+docs
tags: [security, redaction, dead-code, tech-debt, conf-03, postgres]
dependency_graph:
  requires:
    - Phase 22 Plan 02 (run_config_show, redact_option helper)
    - Phase 24 Plan 01 (PostgresBackend, now_iso8601 in postgres.rs)
  provides:
    - postgres_url fully redacted in both JSON and human-readable config show output (CONF-03)
    - now_iso8601() moved into #[cfg(test)] with no dead_code annotation
    - requirements-completed frontmatter populated across all v1.4 SUMMARY.md files
  affects:
    - src/cli.rs
    - src/storage/postgres.rs
    - .planning/phases/21-storage-trait-and-sqlite-backend/21-01-SUMMARY.md
    - .planning/phases/22-config-extension-backend-factory-and-config-cli/22-01-SUMMARY.md
    - .planning/phases/22-config-extension-backend-factory-and-config-cli/22-02-SUMMARY.md
    - .planning/phases/23-qdrant-backend/23-01-SUMMARY.md
    - .planning/phases/23-qdrant-backend/23-02-SUMMARY.md
tech_stack:
  added: []
  patterns:
    - redact_option() call at JSON output site (matches qdrant_api_key, llm_api_key pattern)
    - is_some() guard + hardcoded **** for human-readable output (matches qdrant_api_key pattern)
    - #[cfg(test)] scoping of test-only helpers to avoid dead_code annotations
key_files:
  created: []
  modified:
    - src/cli.rs
    - src/storage/postgres.rs
    - .planning/phases/21-storage-trait-and-sqlite-backend/21-01-SUMMARY.md
    - .planning/phases/22-config-extension-backend-factory-and-config-cli/22-01-SUMMARY.md
    - .planning/phases/22-config-extension-backend-factory-and-config-cli/22-02-SUMMARY.md
    - .planning/phases/23-qdrant-backend/23-01-SUMMARY.md
    - .planning/phases/23-qdrant-backend/23-02-SUMMARY.md
decisions:
  - "postgres_url treated as a secret field identical to qdrant_api_key — uses redact_option() in JSON, is_some()+**** in human-readable"
  - "now_iso8601() moved entirely into #[cfg(test)] mod tests — no #[allow(dead_code)] needed, function is reachable within test module"
  - "CONF-03 closed in Phase 25 not Phase 22 — 22-02-SUMMARY.md lists CONF-04 only, Phase 25 closes CONF-03"
metrics:
  duration: "~3 minutes"
  completed: "2026-03-22"
  tasks: 3
  files: 7
requirements-completed: [CONF-03]
---

# Phase 25 Plan 01: Config Redaction Fix and Tech Debt Cleanup Summary

**One-liner:** Patched postgres_url credential leak in both JSON and human-readable config show paths, moved now_iso8601() into the test module to eliminate the #[allow(dead_code)] annotation, and backfilled requirements-completed frontmatter across all 5 remaining v1.4 SUMMARY.md files.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Fix postgres_url redaction in config show and add Nyquist test (CONF-03) | 25e8872 | src/cli.rs |
| 2 | Move now_iso8601() into #[cfg(test)] and remove dead_code annotation | 50dda53 | src/storage/postgres.rs |
| 3 | Backfill requirements-completed frontmatter in v1.4 SUMMARY.md files | c6de6b0 | 5 SUMMARY.md files |

## What Was Built

**Task 1 — CONF-03 Closure (src/cli.rs):**

- Added `test_conf03_postgres_url_redacted_in_json` Nyquist test proving `redact_option()` works for postgres DSNs — verifies `Some("postgres://user:secret@localhost/mnemonic")` returns `"****"` and serialized output does not contain "secret"
- Fixed JSON output path in `run_config_show()`: `"postgres_url": config.postgres_url` changed to `"postgres_url": redact_option(&config.postgres_url)`
- Fixed human-readable output path: `if let Some(ref url) = config.postgres_url { println!(...url) }` changed to `if config.postgres_url.is_some() { println!("  postgres_url     ****") }` — matching the existing qdrant_api_key pattern

**Task 2 — Dead Code Cleanup (src/storage/postgres.rs):**

- Removed `now_iso8601()` from production scope (had `#[allow(dead_code)]` annotation)
- Inserted identical function body inside `#[cfg(test)] mod tests { }` after `use super::*;` — reachable within the test module, no annotation needed
- Doc comment updated to clarify test-only nature: "Not used in production — production code relies on Postgres NOW() server-side"

**Task 3 — Frontmatter Backfill (5 SUMMARY.md files):**

- `21-01-SUMMARY.md`: `requirements-completed: [STOR-01, STOR-02]` — StorageBackend trait + SqliteBackend
- `22-01-SUMMARY.md`: `requirements-completed: [CONF-01, CONF-02]` — storage_provider config + backend-specific fields
- `22-02-SUMMARY.md`: `requirements-completed: [CONF-04]` — config show subcommand + health backend field (CONF-03 excluded — closed here in Phase 25)
- `23-01-SUMMARY.md`: `requirements-completed: [QDRT-01]` — QdrantBackend store/get_by_id/delete foundation
- `23-02-SUMMARY.md`: `requirements-completed: [QDRT-01, QDRT-02, QDRT-03, QDRT-04]` — all 7 methods complete

## Verification Results

1. `cargo test --lib` — 84 tests pass, 0 failures (includes new Nyquist test)
2. `cargo build --features backend-postgres` — succeeds, no dead_code warning on now_iso8601
3. `grep -rn "allow(dead_code).*now_iso8601|postgres_url.*config.postgres_url[^)]" src/` — no matches
4. All 6 SUMMARY.md files (5 updated + 21-02 already had it) show `requirements-completed`

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None.

## Self-Check: PASSED

- `src/cli.rs` contains `"postgres_url": redact_option(&config.postgres_url)` — FOUND
- `src/cli.rs` contains `println!("  postgres_url     ****")` — FOUND
- `src/cli.rs` contains `fn test_conf03_postgres_url_redacted_in_json` — FOUND
- `src/storage/postgres.rs` has `fn now_iso8601` at line 495 (inside mod tests at line 490) — FOUND
- No `#[allow(dead_code)]` near `now_iso8601` — CONFIRMED
- Commit 25e8872 — FOUND
- Commit 50dda53 — FOUND
- Commit c6de6b0 — FOUND
