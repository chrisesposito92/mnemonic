---
gsd_state_version: 1.0
milestone: v1.2
milestone_name: Authentication / API Keys
status: unknown
stopped_at: Phase 12 context gathered
last_updated: "2026-03-21T01:35:37.822Z"
progress:
  total_phases: 5
  completed_phases: 2
  total_plans: 3
  completed_plans: 3
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-20)

**Core value:** Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run
**Current focus:** Phase 12 — auth-middleware

## Current Position

Phase: 12
Plan: Not started

## Performance Metrics

**Velocity:**

- Total plans completed: 18 (11 v1.0 + 6 v1.1 + 1 v1.2)
- v1.2 plans completed: 1

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for complete log.

Recent decisions affecting v1.2:

- Use BLAKE3 (not SHA-256) for key hashing — faster, pure Rust, simpler API
- Use `constant_time_eq::constant_time_eq_32()` for key comparison — never `==` on key values
- Scope enforcement at handler layer (not service layer) — services remain auth-unaware
- `route_layer()` not `layer()` for middleware — prevents 401 on unmatched routes
- Open mode = COUNT of active keys per request, no startup flag — handles key creation/revocation live
- [Phase 10]: Used per-item #[allow(dead_code)] on Unauthorized variant and auth.rs structs (not module-level) to suppress Phase 10 dead code warnings
- [Phase 10]: Refactored ApiError::IntoResponse to per-variant body handling (Option A) to accommodate richer 401 response body with auth_mode and hint fields
- [Phase 10]: Per-item #[allow(dead_code)] on key_service field (AppState) and KeyService stub methods — no module-level suppression
- [Phase 10]: Bare module paths in main.rs for auth (auth::KeyService) matching compaction/service pattern
- [Phase 11-keyservice-core]: rand::rand_core re-export path used for OsRng import with rand 0.9 (not standalone rand_core crate)
- [Phase 11-keyservice-core]: ORDER BY created_at DESC, id DESC for deterministic list ordering when timestamps collide within same second

### Pending Todos

None.

### Blockers/Concerns

- ~~Display ID for keys list~~ — resolved Phase 11: `display_id = hashed_key[..8]` (hash-derived, not raw prefix)
- Open mode + invalid token edge case: must return 401, not passthrough (PITFALLS.md Auth Pitfall 10)

## Session Continuity

Last session: 2026-03-21T01:35:37.820Z
Stopped at: Phase 12 context gathered
Resume file: .planning/phases/12-auth-middleware/12-CONTEXT.md
