---
phase: 05-config-cleanup
plan: 01
subsystem: config
tags: [rust, config, embedding, anyhow, validate_config]

# Dependency graph
requires:
  - phase: 04-distribution
    provides: fully assembled binary with embedding engine selection in main.rs
provides:
  - validate_config() function in config.rs gating startup on valid provider+key combinations
  - match-based embedding engine selection driven by embedding_provider config field
  - dead code removed: MnemonicError::Server, ConfigError::Invalid, AppState.db/config/embedding, SearchResult struct
  - mnemonic.toml.example documents openai_api_key field
  - README config table accurately describes provider+key relationship
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "validate_config() called immediately after load_config() for fail-fast startup validation"
    - "match on config.embedding_provider.as_str() drives engine selection — no API key heuristics"

key-files:
  created: []
  modified:
    - src/config.rs
    - src/main.rs
    - src/error.rs
    - src/server.rs
    - src/service.rs
    - tests/integration.rs
    - mnemonic.toml.example
    - README.md

key-decisions:
  - "validate_config() returns anyhow::Result<()> — consistent with main.rs error handling chain, no new error variant needed"
  - "AppState slimmed to service-only — db, config, embedding were passed to MemoryService and not used by axum handlers directly"
  - "embedding_provider match uses unreachable!() for unknown arm — valid because validate_config() runs first and would have exited"

patterns-established:
  - "validate_config pattern: call after load_config, before any I/O, return anyhow::Result for clean propagation"

requirements-completed: [CONF-02, CONF-03, EMBD-04]

# Metrics
duration: 5min
completed: 2026-03-20
---

# Phase 5 Plan 1: Config Cleanup Summary

**validate_config() wires embedding_provider to engine selection with fail-fast startup validation; four dead code items removed for zero compiler warnings**

## Performance

- **Duration:** ~4 min
- **Started:** 2026-03-20T01:30:32Z
- **Completed:** 2026-03-20T01:34:42Z
- **Tasks:** 2 completed
- **Files modified:** 8

## Accomplishments
- Added `validate_config()` to `src/config.rs` with 4 unit tests — rejects `openai+no-key` and unknown providers with descriptive error messages
- Replaced API-key-presence heuristic in `main.rs` with `match config.embedding_provider.as_str()` making the config field meaningful
- Removed four dead code items: `MnemonicError::Server`, `ConfigError::Invalid`, three unused `AppState` fields (`db`, `config`, `embedding`), and `SearchResult` struct
- Updated `mnemonic.toml.example` to document all config fields including commented `openai_api_key` field
- Updated README config table and TOML example block to accurately reflect post-wiring behavior

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire embedding_provider config, add validation, remove dead code** - `9038cdc` (feat)
2. **Task 2: Update mnemonic.toml.example and README config table** - `85f93c7` (feat)

**Plan metadata:** TBD (docs: complete plan)

## Files Created/Modified
- `src/config.rs` - Added `validate_config()` function + 4 unit tests
- `src/main.rs` - Wired `validate_config()` call; rewrote engine selection to match on `embedding_provider`; removed unused AppState fields from construction
- `src/error.rs` - Removed `MnemonicError::Server(String)` and `ConfigError::Invalid(String)` dead variants
- `src/server.rs` - Slimmed `AppState` to `service`-only field
- `src/service.rs` - Removed unused `SearchResult` struct
- `tests/integration.rs` - Updated `build_test_state()` AppState construction to `service`-only
- `mnemonic.toml.example` - Added commented `openai_api_key` field with documentation comment
- `README.md` - Updated config table description and TOML example block

## Decisions Made
- `validate_config()` returns `anyhow::Result<()>` — consistent with the `main.rs` error handling chain, no new error variant was needed for validation-only errors
- `AppState` slimmed to `service`-only — `db`, `config`, and `embedding` were all passed into `MemoryService` during construction and never accessed by axum handlers directly
- The `_ => unreachable!()` arm in the engine match is safe because `validate_config()` runs before any I/O and would have returned `Err` for unknown providers

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 5 is the final cleanup phase — all plans complete
- Zero compiler warnings; all 35 tests (14 unit + 21 integration) pass
- `embedding_provider` config field is now meaningful and validated at startup
- v1.0 milestone: all 24 requirements satisfied, all gap closure complete

---
*Phase: 05-config-cleanup*
*Completed: 2026-03-20*
