---
gsd_state_version: 1.0
milestone: v1.6
milestone_name: Web UI/Dashboard
status: Ready to plan
stopped_at: Phase 32 context gathered
last_updated: "2026-03-23T02:15:14.067Z"
progress:
  total_phases: 3
  completed_phases: 2
  total_plans: 6
  completed_plans: 6
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-22)

**Core value:** Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run
**Current focus:** Phase 31 — core-ui

## Current Position

Phase: 32
Plan: Not started

## Performance Metrics

**Velocity:**

- Total plans completed: 52 (11 v1.0 + 6 v1.1 + 8 v1.2 + 11 v1.3 + 9 v1.4 + 7 v1.5)
- Total phases completed: 29

**By Milestone:**

| Milestone | Phases | Plans | Timeline |
|-----------|--------|-------|----------|
| v1.0 MVP | 5 | 11 | 1 day |
| v1.1 Memory Compaction | 4 | 6 | 1 day |
| v1.2 Authentication | 5 | 8 | 2 days |
| v1.3 CLI | 6 | 11 | 2 days |
| v1.4 Pluggable Storage Backends | 5 | 9 | 2 days |
| v1.5 gRPC | 4 | 7 | 1 day |
| Phase 30-dashboard-foundation P01 | 356 | 2 tasks | 19 files |
| Phase 30-dashboard-foundation P02 | 4min | 2 tasks | 2 files |
| Phase 31-core-ui P01 | 8 | 2 tasks | 9 files |
| Phase 31 P02 | 334 | 2 tasks | 7 files |
| Phase 31-core-ui P04 | 31541581s | 2 tasks | 4 files |
| Phase 31 P03 | 346s | 2 tasks | 11 files |

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for complete log.

Recent decisions relevant to v1.6:

- rust-embed 8.11 + axum-embed 0.1 chosen for compile-time asset embedding; both optional deps behind `dashboard` feature
- vite-plugin-singlefile chosen to produce single index.html (verify compatibility in Phase 30 before committing)
- Hash routing (#/path) chosen over history routing to avoid SPA hard-reload 404s at zero cost
- Bearer token stored in Preact component state only — never localStorage; CSP header on all /ui/ responses
- Dashboard router merged at top level (not inside protected router) to prevent auth middleware blocking asset loads
- [Phase 30-dashboard-foundation]: vite-plugin-singlefile must come after tailwindcss() in plugins array (Research Pitfall 2) — verified: single-file output produced
- [Phase 30-dashboard-foundation]: Dashboard router merged outside protected router in build_router() — D-15 prevents auth middleware blocking /ui/ assets
- [Phase 30-dashboard-foundation]: No #[allow_missing = true] on DashboardAssets — compile-time error when dist/ absent is the BUILD-01 safety gate
- [Phase 30-dashboard-foundation]: build_router(test_state) as test boundary — proves /ui is mounted in merged router, not just that dashboard::router() works in isolation
- [Phase 30-dashboard-foundation]: Regression CI job uses debug mode (cargo build not --release) — saves CI time, proves compile and tests pass
- [Phase 30-dashboard-foundation]: node-version-file: dashboard/.node-version — guarantees CI and local dev use same Node version
- [Phase 31-core-ui]: map_response(add_csp) over middleware::from_fn for response-only CSP header injection -- from_fn requires Request+Next, map_response takes Response only
- [Phase 31-core-ui]: stats_for_agent filters client-side after backend.stats() to avoid backend-specific filter logic per single-agent scope
- [Phase 31-core-ui]: Qdrant stats() uses 10K-page scroll with HashMap aggregation to handle collections > 10K points (review concern #4)
- [Phase 31]: fetchHealth never sends auth token -- health endpoint is always public (review concern #2)
- [Phase 31]: apiFetch throws UnauthorizedError on 401/403 so callers trigger re-auth (review concern #8)
- [Phase 31]: LoginScreen validates against /memories?limit=1 not /health (health is public, need real auth test)
- [Phase 31]: handleUnauthorized stored in useRef so Plans 03/04 tab components receive it as onUnauthorized prop
- [Phase 31-core-ui]: Distance bar fill uses (1-distance)*100 clamped 0-100% -- handles backends that return L2 distances > 1.0 (review concern #5)
- [Phase 31-core-ui]: Search triggers on Enter/button only (not on-type) per D-09 -- prevents excessive embedding model calls
- [Phase 31-core-ui]: Empty agent_id shown as (none) in AgentsTab and SearchTab filter -- consistent UX, no blank cells
- [Phase 31]: Agent dropdown populated from GET /stats to show all agents regardless of current page (review concern #1)
- [Phase 31]: AbortController cleanup in both useEffects prevents stale response race conditions on filter/page change (review concern #6)
- [Phase 31]: Session/tag options accumulated across fetches via Set merge to avoid losing options on page turn

### Pending Todos

None.

### Blockers/Concerns

- [Phase 31 risk] GET /stats with Qdrant backend requires non-SQL aggregation path — inspect src/storage/qdrant.rs during Phase 31 implementation
- [Phase 30 risk] vite-plugin-singlefile compatibility with @preact/preset-vite + @tailwindcss/vite is MEDIUM confidence — verify in Phase 30; fallback is multi-file output with axum-embed asset routing

## Session Continuity

Last session: 2026-03-23T02:15:14.065Z
Stopped at: Phase 32 context gathered
Resume file: .planning/phases/32-operational-actions/32-CONTEXT.md
