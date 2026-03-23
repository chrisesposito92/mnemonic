---
phase: 30-dashboard-foundation
plan: 02
subsystem: ui
tags: [integration-tests, ci, dashboard, build_router, axum, github-actions]

# Dependency graph
requires:
  - phase: 30-01
    provides: dashboard/ Preact SPA + src/dashboard.rs + build_router() dashboard merge

provides:
  - tests/dashboard_integration.rs with 6 tests proving /ui is mounted via build_router()
  - .github/workflows/release.yml with Node.js build, dual binary variants, regression gate

affects: [ci-release-workflow, 31-memory-browser]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "#![cfg(feature = \"dashboard\")] at top of test file — test file only compiled with --features dashboard"
    - "build_router(test_state) as the test entry point — exercises full merged router, not isolated dashboard::router()"
    - "actions/setup-node@v4 with node-version-file — node version from dashboard/.node-version for CI/local consistency"
    - "Parallel regression job — runs cargo build + cargo test on default features while matrix builds run"
    - "needs: [build, regression] — release gated on both jobs succeeding"

key-files:
  created:
    - tests/dashboard_integration.rs (6 integration tests exercising build_router() with --features dashboard)
  modified:
    - .github/workflows/release.yml (Node.js build steps, dual artifacts, regression job, release needs gate)

key-decisions:
  - "Test boundary is build_router(test_state) not dashboard::router() — proves /ui is actually mounted in merged router"
  - "Regression job uses debug mode (cargo build, not cargo build --release) — saves CI time, only proves compilation and test pass"
  - "node-version-file: dashboard/.node-version — avoids hardcoded Node version, CI and local dev guaranteed same version"
  - "Trailing slash test accepts 200 or 301/308 — documents and tests axum nest_service behavior as a contract"

requirements-completed: [BUILD-01, BUILD-02, BUILD-03]

# Metrics
duration: 4min
completed: 2026-03-22
---

# Phase 30 Plan 02: Dashboard Integration Tests and CI Wiring Summary

**Six dashboard integration tests prove /ui is mounted in the merged build_router(), and CI now builds dashboard assets before cargo, produces dual binary variants per platform, and gates release on a parallel regression job**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-22T21:16:23Z
- **Completed:** 2026-03-22T21:20:16Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Created `tests/dashboard_integration.rs` with 6 integration tests using `build_router(test_state)` — the same function called by the real server — proving `/ui` is actually mounted in the merged router (not just that `dashboard::router()` works in isolation)
- Tests cover: GET /ui/ returns 200 text/html, response body contains `mnemonic-root` mount point, trailing-slash behavior documented and deterministic, SPA fallback returns index.html for unknown paths, /health still works alongside dashboard, asset requests return valid responses
- Compile-time error verification for missing `dist/` documented in comment block at top of test file
- Updated `.github/workflows/release.yml` with: Node.js setup before Rust toolchain (version from `.node-version` file), dashboard build step (`npm ci && npm run build`), separate slim and dashboard binary build steps, dual artifact staging and upload, parallel `regression` job running default features build+test, and `release` job now requires both `build` and `regression`

## Task Commits

Each task was committed atomically:

1. **Task 1: Add dashboard integration tests exercising build_router()** - `69e59b6` (feat)
2. **Task 2: Update CI release workflow with Node.js build, dual artifacts, regression job** - `7515cc3` (feat)

## Files Created/Modified

- `tests/dashboard_integration.rs` - 6 integration tests only compiled with `--features dashboard`
- `.github/workflows/release.yml` - Updated with Node.js build, dual artifacts, regression job, gated release

## Decisions Made

- Test boundary is `build_router(test_state)` rather than `dashboard::router()` alone — this proves the dashboard is actually accessible through the full production-equivalent router merge chain, addressing review concern #2
- `regression` job uses `cargo build` (debug mode) rather than `cargo build --release` — saves several minutes of CI time while still proving default binary compiles and all tests pass
- Node version read from `dashboard/.node-version` via `node-version-file:` — guarantees CI and local developers use the same Node version without maintaining a hardcoded value in two places
- Trailing-slash test accepts 200 or 301/308 redirect — documents axum `nest_service` behavior as an explicit contract rather than assuming one behavior

## Deviations from Plan

None — plan executed exactly as written. The test file structure and CI workflow structure match the plan's specification precisely.

## Known Stubs

None — all tests exercise live code paths with real AppState.

## Next Phase Readiness

- Dashboard integration tests prove BUILD-01 through the full router merge chain
- CI now builds dashboard assets before each cargo build across all three matrix platforms
- Dual binary variants (slim + dashboard) produced per platform
- Default binary regression gate prevents accidental feature flag leakage
- Ready for Phase 31 (memory browser UI) — the full serving and testing infrastructure is validated

---
*Phase: 30-dashboard-foundation*
*Completed: 2026-03-22*
