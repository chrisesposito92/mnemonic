---
gsd_state_version: 1.0
milestone: v1.2
milestone_name: Authentication / API Keys
status: unknown
stopped_at: "Checkpoint: Task 2 - Verify end-to-end CLI key management (human-verify)"
last_updated: "2026-03-21T03:18:14.780Z"
progress:
  total_phases: 5
  completed_phases: 5
  total_plans: 8
  completed_plans: 8
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-20)

**Core value:** Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run
**Current focus:** Phase 14 — cli-key-management

## Current Position

Phase: 14 (cli-key-management) — EXECUTING
Plan: 2 of 2

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
- [Phase 12]: route_layer() not layer() for middleware scoping prevents 401 on unmatched routes
- [Phase 12]: Open mode (zero active keys) passes through unconditionally — revoking only key re-enables open mode by design (D-05)
- [Phase 12]: Test for revoked-token requires second active key to keep auth mode on; single key revoke = open mode
- [Phase 13]: enforce_scope as free function (not method) keeps scope logic uniform and pure across all 4 relevant handlers
- [Phase 13]: delete_memory_handler uses direct DB ownership lookup (get_memory_agent_id) instead of enforce_scope — path param has no agent_id to compare against
- [Phase 13]: Option<Extension<AuthContext>> (optional) preserves open-mode and wildcard-key behavior without middleware changes
- [Phase 13-http-wiring-and-rest-key-endpoints]: Key handlers skip enforce_scope — any authenticated key can manage keys; scope enforcement is for memory access only
- [Phase 13-http-wiring-and-rest-key-endpoints]: POST /keys returns 201 (creates resource); DELETE /keys/:id returns 200 with body for consistency with memory delete pattern
- [Phase 14]: #[derive(Clone)] added to KeyService — Arc<Connection> is Clone, required for CLI test pattern
- [Phase 14]: CLI module (src/cli.rs) as self-contained module — wired into main.rs in Plan 02
- [Phase 14]: CLI args parsed first via cli::Cli::parse() before any initialization — guarantees fast path without I/O
- [Phase 14]: CLI path skips validate_config to avoid OpenAI key requirement when embeddings not needed

### Pending Todos

None.

### Blockers/Concerns

- ~~Display ID for keys list~~ — resolved Phase 11: `display_id = hashed_key[..8]` (hash-derived, not raw prefix)
- Open mode + invalid token edge case: must return 401, not passthrough (PITFALLS.md Auth Pitfall 10)

## Session Continuity

Last session: 2026-03-21T03:18:08.443Z
Stopped at: Checkpoint: Task 2 - Verify end-to-end CLI key management (human-verify)
Resume file: None
