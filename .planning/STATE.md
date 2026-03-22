---
gsd_state_version: 1.0
milestone: v1.5
milestone_name: gRPC
status: roadmap_complete
stopped_at: Phase 26
last_updated: "2026-03-22T06:00:00Z"
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-22)

**Core value:** Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run
**Current focus:** Milestone v1.5 gRPC — Phase 26: Proto Foundation

## Current Position

Phase: 26 (Proto Foundation)
Plan: —
Status: Not started
Last activity: 2026-03-22 — Roadmap created for v1.5 gRPC

```
v1.5 Progress: [░░░░] 0/4 phases
```

## Performance Metrics

**Velocity:**

- Total plans completed: 45 (11 v1.0 + 6 v1.1 + 8 v1.2 + 11 v1.3 + 9 v1.4)
- Total phases completed: 25

**By Milestone:**

| Milestone | Phases | Plans | Timeline |
|-----------|--------|-------|----------|
| v1.0 MVP | 5 | 11 | 1 day |
| v1.1 Memory Compaction | 4 | 6 | 1 day |
| v1.2 Authentication | 5 | 8 | 2 days |
| v1.3 CLI | 6 | 11 | 2 days |
| v1.4 Pluggable Storage Backends | 5 | 9 | 2 days |
| v1.5 gRPC | 4 | TBD | in progress |

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for complete log.

**v1.5 open decisions (resolve in Phase 26):**
- tonic/prost version: 0.14 vs 0.12 — must run `cargo tree -d` empirically before writing any handler code; do not assume either version
- Tower auth layer vs block_in_place in sync interceptor: Tower Layer is required (KeyService is async); validate `tokio::task::block_in_place` safety during Phase 27 planning

### Critical Research Flags

- **Phase 26 (HARD GATE):** Run `cargo tree -d | grep -E "tonic|prost"` after adding tonic to Cargo.toml. Zero duplicate entries required. If duplicates appear, downgrade to tonic 0.12 / prost 0.13. Document the chosen version in Cargo.toml.
- **Phase 26 (build.rs):** tonic-build has a known always-dirty incremental build bug (#2239). Prevention: emit `println!("cargo:rerun-if-changed=proto/mnemonic.proto")` with explicit path. Verify: two sequential `cargo build` calls — second must complete under 2 seconds.
- **Phase 26 (CI):** protoc must be installed in release.yml in the same commit as build.rs. Use `arduino/setup-protoc@v3` or `apt-get install protobuf-compiler`. Missing this causes cryptic missing-file errors, not a clear "protoc not found".
- **Phase 27 (auth):** Use async Tower Layer on `Server::builder()` — NOT a sync `tonic::service::Interceptor`. Sync interceptor cannot call `KeyService::count_active_keys().await` or `KeyService::verify().await`. Using `block_on()` inside tokio runtime panics.
- **Phase 27 (dual-port):** Use `tokio::try_join!` across two independent TcpListener binds — NOT same-port HTTP+gRPC multiplexing (documented body-type mismatch bugs: tonic #1964, axum #2825).
- **Phase 28 (scope enforcement):** Every gRPC handler MUST call `enforce_scope(auth_ctx, agent_id)`. This is not type-enforced. Add per-handler integration test asserting `Code::PermissionDenied` for mismatched agent_id.

### Pending Todos

None.

### Blockers/Concerns

- tonic/prost version conflict with qdrant-client is an empirical open item — must be resolved at start of Phase 26 before any code is written

## Session Continuity

Last session: 2026-03-22
Stopped at: Roadmap complete — ready for Phase 26 planning
Resume file: None
