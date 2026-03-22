---
phase: 30-dashboard-foundation
plan: 01
subsystem: ui
tags: [preact, typescript, tailwind, vite, rust-embed, axum-embed, dashboard, feature-gate]

# Dependency graph
requires:
  - phase: 29-storage-backend-routing-fix
    provides: Stable build_router() in src/server.rs with merge() composition pattern

provides:
  - dashboard/ Preact+TypeScript+Tailwind v4+Vite frontend project producing dist/index.html
  - src/dashboard.rs module serving embedded SPA at /ui via rust-embed + axum-embed
  - dashboard Cargo feature gate (dep:rust-embed + dep:axum-embed) behind #[cfg(feature = "dashboard")]
  - README.md Dashboard section documenting npm build prerequisite and feature flag usage

affects: [30-02-plan, 31-memory-browser, 32-dashboard-polish, ci-release-workflow]

# Tech tracking
tech-stack:
  added:
    - rust-embed 8.11 (compile-time asset embedding, optional dep behind dashboard feature)
    - axum-embed 0.1 (axum ServeEmbed with FallbackBehavior::Ok for SPA routing)
    - preact 10.29 (lightweight React-compatible UI library)
    - "@preact/preset-vite 2.10.5 (Preact HMR + JSX transform for Vite)"
    - tailwindcss 4.2 + @tailwindcss/vite (CSS-first config, no PostCSS)
    - vite 8.0.1 (frontend build tool)
    - vite-plugin-singlefile 2.3.2 (inlines JS+CSS into single index.html)
    - typescript 5.9 (type safety for frontend)
  patterns:
    - dep: syntax in Cargo.toml features — dashboard = ["dep:rust-embed", "dep:axum-embed"]
    - "#[cfg(feature = \"dashboard\")] mod dashboard; in main.rs and lib.rs (mirrors interface-grpc pattern)"
    - Dashboard router merged at top level in build_router() outside protected router (D-15 prevents auth blocking assets)
    - RustEmbed derive macro without #[allow_missing] — compile-time error when dist/ absent is the safety gate
    - FallbackBehavior::Ok with nest_service(/ui) for SPA fallback routing (D-14 hash routing)
    - Tailwind v4 CSS-first @theme block — no tailwind.config.js or PostCSS
    - vite-plugin-singlefile after tailwindcss() in plugins array (Research Pitfall 2)
    - base:/ui/ in vite.config.ts for multi-file fallback asset URL correctness

key-files:
  created:
    - dashboard/.node-version (pins Node 22 for CI and local dev)
    - dashboard/.gitignore (node_modules/ + dist/ gitignored)
    - dashboard/package.json (preact + tailwindcss + vite-plugin-singlefile + @tailwindcss/vite)
    - dashboard/package-lock.json (required for npm ci in CI)
    - dashboard/tsconfig.json (preact JSX, ESNext target, strict mode)
    - dashboard/vite.config.ts (base=/ui/, health proxy, singlefile after tailwindcss)
    - dashboard/index.html (mnemonic-root mount point, not brittle "app")
    - dashboard/src/vite-env.d.ts (Vite client types)
    - dashboard/src/index.css (@import tailwindcss + @theme CSS vars dark palette)
    - dashboard/src/main.tsx (Preact render into mnemonic-root)
    - dashboard/src/App.tsx (Mnemonic Dashboard heading + HealthCard, ui-monospace font)
    - dashboard/src/components/HealthCard.tsx (fetch /health with AbortSignal.timeout(10_000))
    - src/dashboard.rs (RustEmbed DashboardAssets + axum-embed ServeEmbed at /ui)
  modified:
    - Cargo.toml (rust-embed + axum-embed optional deps + dashboard feature)
    - Cargo.lock (updated with new optional deps)
    - src/server.rs (build_router() extended with dashboard merge at top level)
    - src/main.rs (mod dashboard behind #[cfg(feature = "dashboard")] + tracing info)
    - src/lib.rs (pub mod dashboard behind feature gate)
    - README.md (## Dashboard section + TOC entry)

key-decisions:
  - "vite-plugin-singlefile MUST come after tailwindcss() in plugins array — otherwise CSS not inlined (Research Pitfall 2)"
  - "base: '/ui/' in vite.config.ts ensures multi-file fallback asset URLs resolve correctly under axum-embed /ui mount"
  - "AbortSignal.timeout(10_000) in HealthCard.tsx — addresses review concern #6 for fetch timeout"
  - "mnemonic-root mount point ID (not brittle 'app') — addresses review concern #8"
  - "dashboard router merged at top level in build_router() (not inside protected router) — D-15 prevents auth blocking /ui/ assets"
  - "No #[allow_missing = true] on DashboardAssets — compile-time error when dist/ absent is the BUILD-01 safety gate"
  - "FallbackBehavior::Ok for SPA fallback — all /ui/* paths return index.html with 200 for hash routing"

patterns-established:
  - "Pattern: Dashboard feature gate follows exact dep: syntax and #[cfg] pattern as interface-grpc"
  - "Pattern: Dashboard router merged outside protected router prevents auth middleware interference"
  - "Pattern: Vite base:/ui/ + vite-plugin-singlefile ensures SPA works in both single-file and multi-file fallback modes"

requirements-completed: [BUILD-01, BUILD-02]

# Metrics
duration: 6min
completed: 2026-03-22
---

# Phase 30 Plan 01: Dashboard Foundation Summary

**Preact+TypeScript+Tailwind v4 SPA scaffolded with Vite, producing single dist/index.html, embedded into Mnemonic binary at /ui via rust-embed+axum-embed behind dashboard Cargo feature gate**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-22T21:03:03Z
- **Completed:** 2026-03-22T21:09:00Z
- **Tasks:** 2
- **Files modified:** 19

## Accomplishments

- Scaffolded `dashboard/` Preact+TypeScript project with Tailwind v4 CSS-first config, Vite 8, and vite-plugin-singlefile (produces single 21KB `dist/index.html` with all JS+CSS inlined)
- Created `src/dashboard.rs` with `RustEmbed` + `axum-embed` `ServeEmbed` serving the embedded SPA at `/ui` with `FallbackBehavior::Ok` for hash-routing SPA fallback
- Wired `dashboard` Cargo feature gate into `Cargo.toml`, `src/server.rs`, `src/main.rs`, `src/lib.rs` using exact same `dep:` syntax pattern as `interface-grpc`
- `cargo build --features dashboard` compiles successfully; `cargo build` (default) unaffected with all 87 lib tests passing

## Task Commits

Each task was committed atomically:

1. **Task 1: Scaffold dashboard/ Preact+TypeScript+Tailwind v4+Vite frontend** - `59fb9a7` (feat)
2. **Task 2: Wire Rust feature gate, dashboard module, server integration, and developer docs** - `72c1e11` (feat)

## Files Created/Modified

- `dashboard/.node-version` - Pins Node 22 for CI and local dev consistency
- `dashboard/.gitignore` - Gitignores node_modules/ and dist/
- `dashboard/package.json` - preact + tailwindcss + vite-plugin-singlefile + @tailwindcss/vite
- `dashboard/package-lock.json` - Lockfile for deterministic npm ci in CI
- `dashboard/tsconfig.json` - TypeScript config with Preact JSX, strict mode, ESNext
- `dashboard/vite.config.ts` - base=/ui/, health proxy only, vite-plugin-singlefile after tailwindcss()
- `dashboard/index.html` - Stable mnemonic-root mount point
- `dashboard/src/vite-env.d.ts` - Vite client types
- `dashboard/src/index.css` - @import tailwindcss + @theme dark palette CSS vars
- `dashboard/src/main.tsx` - Preact render into mnemonic-root
- `dashboard/src/App.tsx` - Mnemonic Dashboard heading + HealthCard, ui-monospace font
- `dashboard/src/components/HealthCard.tsx` - fetch /health with AbortSignal.timeout(10_000), status dot, timeout/error distinction
- `src/dashboard.rs` - RustEmbed DashboardAssets from dashboard/dist/ + axum-embed ServeEmbed at /ui
- `Cargo.toml` - rust-embed 8.11 + axum-embed 0.1 optional deps + dashboard feature
- `Cargo.lock` - Updated with new optional deps
- `src/server.rs` - build_router() extended: let mut router + dashboard merge at top level
- `src/main.rs` - mod dashboard + tracing info behind #[cfg(feature = "dashboard")]
- `src/lib.rs` - pub mod dashboard behind feature gate
- `README.md` - ## Dashboard section + TOC entry

## Decisions Made

- `vite-plugin-singlefile` MUST come after `tailwindcss()` in plugins array (Research Pitfall 2) — verified: single-file output produced with no external CSS/JS references
- `base: '/ui/'` in `vite.config.ts` ensures multi-file fallback (D-09) asset URLs resolve correctly when axum-embed serves from the `/ui` nest_service mount — even though singlefile succeeded, the base ensures robustness
- `AbortSignal.timeout(10_000)` with timeout vs generic error distinction addresses review concern #6
- `mnemonic-root` mount point ID (not `app`) addresses review concern #8 about brittle assertions
- No `#[allow_missing = true]` — the compile-time error when `dashboard/dist/` is absent IS the BUILD-01 safety gate
- Proxy only `/health` in `vite.config.ts` (no `/memories` or `/keys`) — Phase 30 proof-of-life only needs health endpoint (addresses Codex review concern about scope creep)

## Deviations from Plan

None — plan executed exactly as written. vite-plugin-singlefile successfully inlined CSS and JS into a single `dist/index.html` (single-file path worked; D-09 multi-file fallback not needed).

## Issues Encountered

- `npm create vite@latest . --template preact-ts` is interactive and cancelled in non-TTY; resolved by creating all files manually from the plan's specified templates. No functional impact.

## Known Stubs

None — `HealthCard.tsx` fetches live data from `GET /health`. The `version` field displays `--` when absent from the API response (intentional: current health endpoint doesn't return version; Phase 31 may add it).

## Next Phase Readiness

- `dashboard/dist/index.html` exists and builds successfully — ready for Plan 30-02 (CI wiring + integration tests)
- `cargo build --features dashboard` compiles — ready for dashboard integration tests
- `cargo build` (default) still passes all 87 lib tests — no regression
- The `/ui` route is live and serves the health card SPA when the binary is built with `--features dashboard`

---
*Phase: 30-dashboard-foundation*
*Completed: 2026-03-22*
