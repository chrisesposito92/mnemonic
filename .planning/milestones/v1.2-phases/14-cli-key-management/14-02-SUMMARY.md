---
phase: 14-cli-key-management
plan: "02"
subsystem: cli
tags: [cli, clap, dual-mode, main-rs, key-management]

requires:
  - phase: 14-cli-key-management-plan-01
    provides: cli module (Cli, Commands, KeysArgs, KeysSubcommand, run_keys), clap dependency, find_by_display_id

provides:
  - dual-mode binary dispatch: CLI path (keys subcommand) vs server path (no subcommand)
  - mnemonic keys create/list/revoke functional from terminal
  - fast CLI startup (<1s) by skipping embedding model loading on CLI path

affects: [src/main.rs]

tech-stack:
  added: []
  patterns: [dual-mode binary dispatch (CLI args parsed before any I/O), CLI path minimal init (DB only), server path unchanged]

key-files:
  created: []
  modified:
    - src/main.rs

key-decisions:
  - "CLI args parsed first via cli::Cli::parse() before any initialization — guarantees fast path without I/O"
  - "CLI path skips validate_config to avoid OpenAI key requirement when no embedding is needed"
  - "CLI path skips init_tracing for clean stdout/stderr output without log noise"
  - "--db override applied after load_config() by mutating config.db_path before db::open()"

patterns-established:
  - "Dual-mode dispatch pattern: parse args -> branch -> early return on CLI path -> server path falls through"

requirements-completed: [CLI-01, CLI-02, CLI-03]

duration: ~5min
completed: "2026-03-21"
---

# Phase 14 Plan 02: CLI/Server Dual-Mode Dispatch Summary

**main.rs restructured so `mnemonic keys <subcommand>` takes a fast DB-only path (no embedding model load), while `mnemonic` with no args starts the server unchanged**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-03-21T03:15:00Z
- **Completed:** 2026-03-21T03:17:18Z
- **Tasks:** 2/2
- **Files modified:** 1

## Accomplishments

- Restructured main.rs with CLI dispatch as the first action in main(), before any I/O or initialization
- CLI path (when `keys` subcommand detected): register_sqlite_vec -> load_config -> apply --db override -> db::open -> KeyService::new -> run_keys -> return Ok(())
- Server path: exactly unchanged from prior main.rs (reached when no subcommand is given)
- All 57 unit tests, 57 integration-compiled tests, and 53 integration tests pass — no regressions
- Help output confirms `mnemonic --help` lists `keys` subcommand; `mnemonic keys --help` shows create/list/revoke
- Human-verified end-to-end: keys create, keys list, keys revoke, scoped keys, error cases, startup speed, server backward compatibility all approved

## Task Commits

1. **Task 1: Restructure main.rs for dual-mode CLI/server dispatch** - `8752b56` (feat)
2. **Task 2: Verify end-to-end CLI key management** - human checkpoint approved

## Files Created/Modified

- `src/main.rs` - Added CLI dispatch block at top of main(); added `use clap::Parser` and `mod cli`; server path unchanged below

## Decisions Made

- CLI path calls `config::load_config()` but NOT `config::validate_config()` — validate_config would reject OpenAI embedding configs when OPENAI_API_KEY is absent, but CLI path never touches embeddings
- CLI path calls `db::register_sqlite_vec()` before `db::open()` — same ordering requirement as server path (sqlite-vec must be registered first)
- `--db` override applied by mutating `config.db_path` after `load_config()` returns, before `db::open()` — no changes needed to config.rs
- No `init_tracing()` on CLI path — clean output with no `INFO mnemonic starting` log noise

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None — all CLI commands call real KeyService methods against the real DB.

## Issues Encountered

None.

## Next Phase Readiness

- Phase 14 is complete — all `mnemonic keys` subcommands human-verified end-to-end
- v1.2 Authentication/API Keys milestone fully implemented (all 15 requirements complete across Phases 10-14)
- No blockers for milestone closure
- Ready for `/gsd:complete-milestone` to finalize v1.2

## Self-Check: PASSED

- [x] src/main.rs contains `use clap::Parser;`
- [x] src/main.rs contains `mod cli;`
- [x] src/main.rs contains `let cli_args = cli::Cli::parse();`
- [x] src/main.rs contains `if let Some(cli::Commands::Keys(keys_args)) = cli_args.command`
- [x] src/main.rs contains `cli::run_keys(keys_args.subcommand, key_service).await;`
- [x] src/main.rs contains `config.db_path = db_override;`
- [x] CLI branch does NOT contain `init_tracing` or `validate_config`
- [x] `cargo build` exits 0
- [x] `cargo test` exits 0 (all 57 + 53 tests pass)
- [x] `cargo run -- --help` shows "mnemonic" with Keys subcommand listed
- [x] `cargo run -- keys --help` shows Create, List, Revoke subcommands
- [x] Commit 8752b56 exists in git log

---
*Phase: 14-cli-key-management*
*Completed: 2026-03-21*
