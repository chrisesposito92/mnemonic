---
phase: 14-cli-key-management
plan: "01"
subsystem: cli
tags: [cli, clap, auth, key-management]
dependency_graph:
  requires: [phase-11-keyservice-core, phase-13-http-wiring-and-rest-key-endpoints]
  provides: [cli-module, find-by-display-id, clap-structs]
  affects: [src/cli.rs, src/auth.rs, Cargo.toml, src/lib.rs]
tech_stack:
  added: [clap 4 with derive feature]
  patterns: [clap derive macros, dual-mode binary CLI module, in-memory KeyService test helper]
key_files:
  created:
    - src/cli.rs
  modified:
    - Cargo.toml
    - src/auth.rs
    - src/lib.rs
decisions:
  - "#[derive(Clone)] added to KeyService (Arc<Connection> is Clone) — required for CLI test pattern where service is used after handler call"
  - "truncate() helper keeps table column widths consistent at 20 chars (17 + '...')"
  - "is_display_id() exposed as pub(crate) for direct testability"
metrics:
  duration_seconds: 162
  completed_date: "2026-03-21"
  tasks_completed: 2
  files_modified: 4
---

# Phase 14 Plan 01: CLI Module Build Summary

**One-liner:** clap 4 derive CLI module with keys create/list/revoke handlers and find_by_display_id lookup, all tested with in-memory KeyService.

## What Was Built

### Task 1: clap dependency + find_by_display_id (commit 10c2389)

**Cargo.toml** — Added `clap = { version = "4", features = ["derive"] }` between `candle-transformers` and `constant_time_eq`.

**src/auth.rs** — Two additions:
1. `#[derive(Clone)]` on `KeyService` — `Arc<Connection>` is already Clone, so this is free. Required for CLI tests that need to hold a reference while the handler takes ownership.
2. `find_by_display_id(&self, display_id: &str) -> Result<Vec<ApiKey>, DbError>` — queries `api_keys WHERE display_id = ?1`. Returns `Vec<ApiKey>` so caller handles 0 (not found), 1 (exact match), or >1 (ambiguous) cases.
3. `test_find_by_display_id` test: creates a key, verifies exact match by display_id, verifies empty result for non-existent display_id.

### Task 2: CLI module (commit d60548e)

**src/lib.rs** — Added `pub mod cli;` after `pub mod auth;`.

**src/cli.rs** — New file with:

- **Clap derive structs:** `Cli` (top-level with optional `--db` global flag), `Commands` enum, `KeysArgs`, `KeysSubcommand` (Create/List/Revoke variants)
- **`run_keys(subcommand, key_service)`** — single dispatcher called from main.rs
- **`cmd_create(key_service, name, agent_id)`** — calls `KeyService::create`, prints raw `mnk_...` token on first stdout line (pipeable), then ID/Name/Scope metadata, then "Save this key" warning to stderr
- **`cmd_list(key_service)`** — calls `KeyService::list`, prints empty-state hint or formatted table with ID/NAME/SCOPE/CREATED/STATUS columns; truncates name/scope at 20 chars; STATUS shows "active" or "revoked (YYYY-MM-DD)"
- **`cmd_revoke(key_service, id)`** — checks `is_display_id()`, branches to display_id lookup (0/1/n results) or direct UUID revoke
- **`is_display_id(input)`** — `pub(crate)` helper, true iff len==8 and all chars are ASCII hex digits
- **`truncate(s, max_len)`** — clips to `max_len` with "..." suffix
- **10 unit tests** covering all helpers and all handler success paths

## Test Results

```
running 57 tests
.........................................................
test result: ok. 57 passed; 0 failed; 0 ignored; 0 filtered; finished in 0.02s
```

Previously 46 tests; 11 new tests added (1 in auth, 10 in cli).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality] Added #[derive(Clone)] to KeyService**
- **Found during:** Task 2 — plan specified tests calling `cmd_create(ks.clone(), ...)` then checking `ks.list()` afterward
- **Issue:** KeyService had no Clone impl; test pattern requires using service after handing clone to handler
- **Fix:** Added `#[derive(Clone)]` to KeyService struct in auth.rs — zero-cost since Arc<Connection> is already Clone
- **Files modified:** src/auth.rs
- **Commit:** 10c2389 (included in Task 1 commit)

## Known Stubs

None — all handler functions call real KeyService methods against the real DB. No placeholder data or TODO output.

## Self-Check: PASSED

- [x] src/cli.rs exists and exports Cli, Commands, KeysArgs, KeysSubcommand, run_keys
- [x] src/auth.rs contains find_by_display_id
- [x] Cargo.toml contains clap = { version = "4", features = ["derive"] }
- [x] src/lib.rs contains pub mod cli
- [x] Commits 10c2389 and d60548e exist in git log
- [x] cargo check exits 0 (warnings only, no errors)
- [x] cargo test --lib exits 0 with 57 tests passing
