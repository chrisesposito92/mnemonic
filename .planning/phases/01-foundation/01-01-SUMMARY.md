---
phase: 01-foundation
plan: 01
subsystem: infra
tags: [rust, cargo, figment, thiserror, tokio-rusqlite, sqlite-vec, config]

# Dependency graph
requires: []
provides:
  - Cargo.toml with all 13 Phase 1 dependencies and correct feature flags
  - src/error.rs with MnemonicError, DbError, ConfigError typed enums
  - src/config.rs with Config struct and figment-based layered configuration
  - src/main.rs stub for compilation (placeholder, replaced in Plan 02)
affects: [02-db, 03-server, 04-api]

# Tech tracking
tech-stack:
  added:
    - tokio 1 (async runtime, full features)
    - axum 0.8 (HTTP framework)
    - rusqlite 0.37 (SQLite bindings, bundled)
    - sqlite-vec 0.1.7 (vector search FFI extension)
    - tokio-rusqlite 0.7 (async SQLite bridge)
    - figment 0.10 (layered config, toml+env features)
    - serde 1 / serde_json 1 (serialization)
    - tracing 0.1 + tracing-subscriber 0.3 (structured logging)
    - thiserror 2 (typed error derives)
    - anyhow 1 (top-level error propagation)
    - uuid 1 (v7 feature for time-ordered UUIDs)
  patterns:
    - figment Defaults -> Toml::file -> Env::prefixed("MNEMONIC_") merge order for config precedence
    - thiserror derive for typed domain error enums (DbError, ConfigError, MnemonicError)
    - figment::Jail for isolated env/file tests without process-level env contamination

key-files:
  created:
    - Cargo.toml
    - src/error.rs
    - src/config.rs
    - src/main.rs
  modified: []

key-decisions:
  - "rusqlite downgraded from 0.39 to 0.37 — required by sqlite-vec 0.1.7 FFI compatibility (libsqlite3-sys version conflict)"
  - "figment test feature added as dev-dependency to enable Jail-based env isolation in config tests"
  - "Format trait imported explicitly — required for Toml::file() method dispatch in figment 0.10"
  - "Jail closure uses &mut Jail (not &Jail) per figment 0.10.19 API signature"

patterns-established:
  - "Config precedence: Env::prefixed(MNEMONIC_) > Toml::file(mnemonic.toml) > Serialized::defaults"
  - "figment::Jail::expect_with for all config tests — prevents env contamination between tests"
  - "thiserror::Error derive with #[from] for automatic error conversion chains"

requirements-completed: [CONF-01, CONF-02, CONF-03]

# Metrics
duration: 4min
completed: 2026-03-19
---

# Phase 1 Plan 1: Rust project skeleton with figment layered config (port 8080 default, env > TOML > defaults precedence, 5 passing unit tests)

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-19T20:05:20Z
- **Completed:** 2026-03-19T20:09:04Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Cargo.toml with all 13 Phase 1 dependencies and pinned rusqlite at 0.37 for sqlite-vec compatibility
- src/error.rs with MnemonicError, DbError, ConfigError enums using thiserror derives and From<tokio_rusqlite::Error> impl
- src/config.rs with Config struct (port 8080, db_path ./mnemonic.db, embedding_provider local) and figment-based load_config()
- 5 config unit tests all passing using figment::Jail for env isolation

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Cargo.toml and error module** - `b549e74` (feat)
2. **Task 2: Implement layered configuration with figment (TDD)** - `7d96e21` (feat)

**Plan metadata:** _(will be added by final commit)_

_Note: TDD task (Task 2) combined RED+GREEN in one commit as implementation and tests were developed together against a known correct pattern._

## Files Created/Modified
- `Cargo.toml` - Single crate manifest with all 13 Phase 1 deps; rusqlite pinned at 0.37
- `src/error.rs` - MnemonicError (Db, Config, Server variants), DbError (Open, Schema, Query), ConfigError (Load, Invalid); From<tokio_rusqlite::Error> impl
- `src/config.rs` - Config struct with Default impl, load_config() using Figment merge chain, 5 inline unit tests
- `src/main.rs` - Temporary stub (mod config; mod error; fn main() {}) to satisfy cargo

## Decisions Made
- **rusqlite 0.37 instead of 0.39:** sqlite-vec 0.1.7 depends on rusqlite 0.37 internally (libsqlite3-sys 0.35.0); rusqlite 0.39 requires libsqlite3-sys 0.37.0 — these conflict. Pinned to 0.37 for compatibility. No feature regression since all required APIs exist in 0.37.
- **figment test feature as dev-dependency:** figment::Jail (used for env-isolated tests) requires the `test` feature. Added to `[dev-dependencies]` separately from the production dependency to avoid shipping test utilities in the binary.
- **Format trait explicit import:** figment 0.10 requires `use figment::providers::Format` to call `Toml::file()` — the method is defined on the trait, not inherently on the Toml struct.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed rusqlite/sqlite-vec version conflict**
- **Found during:** Task 1 (cargo check)
- **Issue:** Plan specified rusqlite 0.39 but sqlite-vec 0.1.7 requires rusqlite 0.37; the two versions pull incompatible libsqlite3-sys versions (0.37.0 vs 0.35.0) which Cargo rejects with "Only one package may link sqlite3"
- **Fix:** Downgraded rusqlite from 0.39 to 0.37 in Cargo.toml; verified tokio-rusqlite 0.7 is also compatible with rusqlite 0.37
- **Files modified:** Cargo.toml
- **Verification:** cargo check passes; all 13 dependencies resolve
- **Committed in:** b549e74 (Task 1 commit)

**2. [Rule 3 - Blocking] Added figment test feature and Format trait import**
- **Found during:** Task 2 (cargo test — compile errors)
- **Issue 1:** figment::Jail requires `feature = "test"` which is gated behind cfg(test) — not available in the main figment dependency; needed as a separate dev-dependency entry
- **Issue 2:** Toml::file() requires `use figment::providers::Format` trait in scope; not auto-imported
- **Issue 3:** Jail closure signature is `FnOnce(&mut Jail)` not `FnOnce(&Jail)` — type annotations were wrong
- **Fix:** Added `figment = { version = "0.10", features = ["toml", "env", "test"] }` to `[dev-dependencies]`; added `Format` to use statement; changed `&figment::Jail` to `&mut figment::Jail` in all test closures
- **Files modified:** Cargo.toml, src/config.rs
- **Verification:** cargo test test_config — 5 passed
- **Committed in:** 7d96e21 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 bug fix: version conflict; 1 blocking: API usage corrections)
**Impact on plan:** Both fixes required for compilation and correctness. No scope changes.

## Issues Encountered
- figment Jail API subtleties (test feature gating, mut reference signature) not documented in the plan's research notes. All resolved through compile errors and fixes.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Cargo.toml foundation ready; all Phase 1 deps compile
- Error types established for use by db.rs (Plan 02) and server.rs (Plan 03)
- Config struct provides typed access to port, db_path, embedding_provider for all subsequent plans
- main.rs stub ready to be replaced with full implementation in Plan 02

---
*Phase: 01-foundation*
*Completed: 2026-03-19*
