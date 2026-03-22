---
phase: 22-config-extension-backend-factory-and-config-cli
plan: "01"
subsystem: config+storage
tags: [config, storage, backend-factory, tdd]
dependency_graph:
  requires: [21-02]
  provides: [Config.storage_provider, Config.qdrant_url, Config.qdrant_api_key, Config.postgres_url, create_backend()]
  affects: [src/config.rs, src/storage/mod.rs, Cargo.toml]
tech_stack:
  added: [backend-qdrant feature flag, backend-postgres feature flag]
  patterns: [match-arm provider validation, async factory function, cfg feature gates]
key_files:
  created: []
  modified:
    - src/config.rs
    - src/storage/mod.rs
    - Cargo.toml
    - src/cli.rs
decisions:
  - "Feature gate errors (qdrant/postgres) go at create_backend() time, not validate_config() — keeps config portable across builds (D-09)"
  - "backend-qdrant and backend-postgres added as empty feature flags to Cargo.toml — zero dependency overhead, enables cfg conditionals without warnings"
  - "ConfigError::Load used to wrap create_backend error messages — reuses existing error infrastructure without adding a new variant"
metrics:
  duration: "~5 minutes"
  completed: "2026-03-21"
  tasks: 2
  files: 4
requirements-completed: [CONF-01, CONF-02]
---

# Phase 22 Plan 01: Config Extension and Backend Factory Summary

Config struct extended with 4 storage fields; validate_config() expanded with storage_provider match block; create_backend() factory function added to storage/mod.rs with feature-gate stubs for qdrant/postgres.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Extend Config struct and validate_config() | 5521c25 | src/config.rs, src/cli.rs |
| 2 | Add create_backend() factory function | b8f18e4 | src/storage/mod.rs, Cargo.toml |

## What Was Built

**Task 1: Config struct extension**
- Added `storage_provider: String` (defaults to `"sqlite"`) to Config
- Added `qdrant_url: Option<String>`, `qdrant_api_key: Option<String>`, `postgres_url: Option<String>` (all `None` by default)
- Expanded `validate_config()` with storage_provider match arm after existing LLM validation block
- Validation: qdrant requires MNEMONIC_QDRANT_URL, postgres requires MNEMONIC_POSTGRES_URL, unknown providers produce list of valid options
- All existing tests updated (`test_config_defaults` now asserts new fields)
- 9 new tests added covering all validation branches + env/TOML override paths

**Task 2: create_backend() factory function**
- Added `pub async fn create_backend(config: &Config, sqlite_conn: Arc<Connection>) -> Result<Arc<dyn StorageBackend>, ApiError>` to src/storage/mod.rs
- Returns `SqliteBackend::new(sqlite_conn)` for `"sqlite"` provider
- Returns `ApiError::Internal(MnemonicError::Config(...))` for qdrant/postgres when built without feature flags
- Returns clear error for unknown providers
- Added `backend-qdrant` and `backend-postgres` as empty Cargo features (zero dependencies)
- 4 new tests covering all factory branches

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed incomplete struct literal in cli.rs test helper**
- **Found during:** Task 1 (compile error after adding new Config fields)
- **Issue:** `test_key_service()` in `src/cli.rs:836` used an exhaustive struct literal that was missing the 4 new fields
- **Fix:** Converted to `Config { port: 0, db_path: ":memory:".to_string(), ..Config::default() }` using struct update syntax
- **Files modified:** src/cli.rs
- **Commit:** 5521c25

**2. [Rule 2 - Missing critical functionality] Added feature flags to Cargo.toml**
- **Found during:** Task 2 (compiler warnings about unexpected cfg conditions)
- **Issue:** `#[cfg(feature = "backend-qdrant")]` and `#[cfg(feature = "backend-postgres")]` produced warnings since the features were not declared in Cargo.toml
- **Fix:** Added `[features]` section with `backend-qdrant = []` and `backend-postgres = []`
- **Files modified:** Cargo.toml
- **Commit:** b8f18e4

## Verification Results

- `cargo test --lib config::tests`: 22 passed, 0 failed
- `cargo test --lib storage::tests`: 6 passed, 0 failed
- `cargo test` (full suite): all test suites pass, 0 failures

## Known Stubs

- `create_backend()` for `"qdrant"` arm: `todo!("QdrantBackend construction — implemented in Phase 23")` inside `#[cfg(feature = "backend-qdrant")]` block — intentional, Phase 23 implements this
- `create_backend()` for `"postgres"` arm: `todo!("PostgresBackend construction — implemented in Phase 24")` inside `#[cfg(feature = "backend-postgres")]` block — intentional, Phase 24 implements this

These stubs are behind feature flags that are not enabled by default. Without the flags, the function returns a clear runtime error. The stubs only activate when the feature is enabled, and they will be replaced in Phases 23-24.

## Self-Check: PASSED
