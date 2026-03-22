---
phase: 22-config-extension-backend-factory-and-config-cli
plan: "02"
subsystem: cli+server+storage
tags: [config, cli, health, backend-factory, storage]
dependency_graph:
  requires: [22-01]
  provides: [Config subcommand, run_config_show(), health backend field, create_backend() wired everywhere]
  affects: [src/cli.rs, src/main.rs, src/server.rs, tests/integration.rs]
tech_stack:
  added: []
  patterns: [config subcommand pattern, redact_option helper, AppState.backend_name]
key_files:
  created: []
  modified:
    - src/cli.rs
    - src/main.rs
    - src/server.rs
    - tests/integration.rs
decisions:
  - "Config subcommand dispatches before any DB/embedding init — lightest init tier (D-17)"
  - "backend_name stored as plain String in AppState — no need for Arc, no StorageBackend method required (D-23)"
  - "redact_option() returns serde_json::Value (not Option<String>) so JSON null and '****' are typed correctly (D-20)"
metrics:
  duration: "~7 minutes"
  completed: "2026-03-21"
  tasks: 2
  files: 4
requirements-completed: [CONF-04]
---

# Phase 22 Plan 02: Config CLI, Health Backend Field, and Backend Factory Wiring Summary

Config subcommand added to CLI with human-readable and JSON output with secret redaction; GET /health extended with backend field from AppState; all three production backend creation sites switched from hardcoded SqliteBackend::new() to create_backend() factory.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add Config subcommand to CLI and dispatch in main.rs | 6838bb7 | src/cli.rs, src/main.rs |
| 2 | Update health endpoint and replace hardcoded backends with factory | 9d34956 | src/server.rs, src/main.rs, src/cli.rs, tests/integration.rs |

## What Was Built

**Task 1: Config subcommand**
- Added `ConfigArgs` and `ConfigSubcommand::Show` types to `src/cli.rs` following the KeysArgs/KeysSubcommand pattern
- Added `Config(ConfigArgs)` variant to `Commands` enum
- Added `run_config_show(json_mode: bool)` with human-readable output (grouped: Server, Storage, Embedding, LLM sections) and JSON output mode
- Added `redact_option()` helper: any `Some(_)` field returns `"****"` as `serde_json::Value`; `None` returns `Value::Null`
- Added Config dispatch arm in `main.rs` before the Serve arm — dispatches before any DB/embedding initialization and returns immediately

**Task 2: Health endpoint and factory wiring**
- Added `backend_name: String` field to `AppState` in `src/server.rs`
- Updated `health_handler` to accept `State(state): State<AppState>` and return `{"status":"ok","backend":"<provider>"}`
- Replaced hardcoded `SqliteBackend::new(db_arc.clone())` in `main.rs` server path with `storage::create_backend(&config, db_arc.clone()).await`
- Added `backend_name: config.storage_provider.clone()` to `AppState` construction in `main.rs`
- Replaced `SqliteBackend::new(conn_arc)` in `cli::init_db_and_embedding()` with `crate::storage::create_backend(&config, conn_arc).await`
- Replaced `SqliteBackend::new(conn_arc.clone())` in `cli::init_compaction()` with `crate::storage::create_backend(&config, conn_arc.clone()).await`
- Added `backend_name: "sqlite".to_string()` to both `AppState` construction sites in `tests/integration.rs` (required by new struct field)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Added backend_name to integration test AppState construction**
- **Found during:** Task 2 (compile errors after adding backend_name to AppState)
- **Issue:** Two AppState construction sites in `tests/integration.rs` were missing the new `backend_name` field, causing `error[E0063]: missing field 'backend_name'`
- **Fix:** Added `backend_name: "sqlite".to_string()` to both test helper AppState literals
- **Files modified:** tests/integration.rs
- **Commit:** 9d34956

## Verification Results

- `cargo build`: succeeded with 2 pre-existing warnings, 0 errors
- `cargo run -- config show`: outputs Server, Storage, Embedding, LLM sections with secrets redacted as ****
- `cargo run -- config show --json`: outputs valid JSON with null for unset keys, redacted secrets
- `cargo run -- help`: shows "config" as a subcommand
- `cargo test`: 273 tests pass, 0 failures (80 + 80 + 55 + 4 + 54 = 273)
- No occurrence of `SqliteBackend::new` in `src/main.rs` or `src/cli.rs`

## Known Stubs

None. All production paths use the create_backend() factory. The factory itself has stubs for qdrant/postgres (tracked in 22-01-SUMMARY.md) but those are behind disabled feature flags.

## Self-Check: PASSED
