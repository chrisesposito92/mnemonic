---
phase: 31-core-ui
plan: 01
subsystem: backend-api
tags: [stats, auth, csp, dashboard, storage-backend]
dependency_graph:
  requires: []
  provides: [stats-endpoint, auth-enabled-health, csp-middleware]
  affects: [src/service.rs, src/storage/mod.rs, src/storage/sqlite.rs, src/storage/qdrant.rs, src/storage/postgres.rs, src/server.rs, src/dashboard.rs, dashboard/vite.config.ts, tests/dashboard_integration.rs]
tech_stack:
  added: []
  patterns: [map_response-csp-middleware, scope-aware-handler, paginated-scroll-aggregation]
key_files:
  created: []
  modified:
    - src/service.rs
    - src/storage/mod.rs
    - src/storage/sqlite.rs
    - src/storage/qdrant.rs
    - src/storage/postgres.rs
    - src/server.rs
    - src/dashboard.rs
    - dashboard/vite.config.ts
    - tests/dashboard_integration.rs
decisions:
  - "map_response(add_csp) over middleware::from_fn for response-only header injection (from_fn requires Request+Next signature, map_response takes Response only)"
  - "stats_for_agent delegates to backend.stats() and filters client-side (avoids backend-specific filter logic per single-agent scope)"
  - "Qdrant stats() uses paginated scroll (10K pages) with HashMap aggregation to handle collections > 10K points"
  - "Postgres stats() uses TO_CHAR(MAX(created_at)) for consistent ISO 8601 string output matching SQLite format"
metrics:
  duration: "8 minutes"
  completed: "2026-03-23T01:35:00Z"
  tasks_completed: 2
  files_modified: 9
---

# Phase 31 Plan 01: Backend API Extensions Summary

Backend API extensions for the dashboard: GET /stats (scope-aware), auth_enabled on GET /health, CSP header middleware on /ui/, and cross-feature build verification — all 9 dashboard integration tests pass.

## Tasks Completed

| Task | Description | Commit | Files |
|------|-------------|--------|-------|
| 1 | StorageBackend::stats() + GET /stats + auth_enabled + CSP | 858c7c5 | 8 files |
| 2 | Integration tests for stats, CSP, health auth_enabled, scope | d900595 | 1 file |

## What Was Built

### StorageBackend::stats() trait method (8th method on trait)
- Added `AgentStats` struct (`agent_id: String`, `memory_count: u64`, `last_active: String`) and `StatsResponse` to `src/service.rs`
- Added `async fn stats(&self) -> Result<Vec<AgentStats>, ApiError>` to `StorageBackend` trait
- **SQLite**: `GROUP BY agent_id ORDER BY last_active DESC` query via `tokio_rusqlite::Connection::call()`
- **Qdrant**: Paginated scroll (10K per page) with `HashMap<String, (u64, String)>` client-side aggregation; handles collections > 10K points
- **Postgres**: `TO_CHAR(MAX(created_at) AT TIME ZONE 'UTC', ...)` with `GROUP BY agent_id` for consistent ISO 8601 string output

### GET /stats endpoint
- `stats_handler` added to protected router (behind auth middleware)
- Scope-aware: scoped keys (`allowed_agent_id.is_some()`) see only their agent's stats via `service.stats_for_agent()`
- Wildcard keys and open mode return all agents via `service.stats()`
- Response: `{ agents: [{ agent_id, memory_count, last_active }] }`

### auth_enabled on GET /health
- `health_handler` calls `state.key_service.count_active_keys().await` and maps count > 0 to `auth_enabled: true`
- Fail-safe: DB error defaults to `auth_enabled: true` (conservative)
- Endpoint remains PUBLIC (no auth middleware)

### CSP middleware on /ui/
- `add_csp(Response) -> Response` function using `axum::middleware::map_response`
- Policy: `default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'`
- Applied via `.layer(map_response(add_csp))` on dashboard router
- Covers all /ui/* paths including SPA fallback (FallbackBehavior::Ok)

### Vite dev proxy expansion
- Added `/memories`, `/stats`, `/keys` proxies alongside existing `/health`

### Integration tests (9 total, all pass)
- `dashboard_ui_includes_csp_header`: verifies CSP header on `/ui/` and `/ui/some/deep/path`
- `stats_endpoint_returns_agent_breakdown`: creates memories for `agent-alpha` and `agent-beta`, asserts both in stats
- `health_endpoint_includes_auth_enabled_field`: asserts `auth_enabled` boolean present and `false` in test environment
- Cross-feature build verification documented as comment block (not executable — requires external services)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed middleware::from_fn to map_response for CSP**
- **Found during:** Task 1 build verification
- **Issue:** Plan specified `middleware::from_fn(add_csp)` but `from_fn` expects a `(Request, Next) -> Response` signature; the plan's `add_csp(Response) -> Response` function only takes a Response
- **Fix:** Used `axum::middleware::map_response(add_csp)` which is the correct axum API for response-only transformations. Changed import from `middleware::{self, map_response}` to just `map_response`
- **Files modified:** `src/dashboard.rs`
- **Commit:** 858c7c5

## Verification Results

- `cargo build --features dashboard`: PASSED
- `cargo test --features dashboard --test dashboard_integration`: 9/9 PASSED
- `cargo check --features dashboard,backend-qdrant`: PASSED (cross-feature)
- `cargo check --features dashboard,backend-postgres`: PASSED (cross-feature)

## Known Stubs

None. All data flows are wired: `stats_handler` -> `service.stats()` / `service.stats_for_agent()` -> `backend.stats()` -> SQL/Qdrant scroll.

## Self-Check: PASSED
