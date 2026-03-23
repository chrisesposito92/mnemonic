# Roadmap: Mnemonic

## Milestones

- ✅ **v1.0 MVP** — Phases 1-5 (shipped 2026-03-20)
- ✅ **v1.1 Memory Compaction** — Phases 6-9 (shipped 2026-03-20)
- ✅ **v1.2 Authentication / API Keys** — Phases 10-14 (shipped 2026-03-21)
- ✅ **v1.3 CLI** — Phases 15-20 (shipped 2026-03-21)
- ✅ **v1.4 Pluggable Storage Backends** — Phases 21-25 (shipped 2026-03-22)
- ✅ **v1.5 gRPC** — Phases 26-29 (shipped 2026-03-22)
- 🚧 **v1.6 Web UI/Dashboard** — Phases 30-32 (in progress)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-5) — SHIPPED 2026-03-20</summary>

- [x] Phase 1: Foundation (3/3 plans) — completed 2026-03-19
- [x] Phase 2: Embedding (2/2 plans) — completed 2026-03-19
- [x] Phase 3: Service and API (3/3 plans) — completed 2026-03-19
- [x] Phase 4: Distribution (2/2 plans) — completed 2026-03-19
- [x] Phase 5: Config & Embedding Provider Cleanup (1/1 plan) — completed 2026-03-20

</details>

<details>
<summary>✅ v1.1 Memory Compaction (Phases 6-9) — SHIPPED 2026-03-20</summary>

- [x] Phase 6: Foundation (2/2 plans) — completed 2026-03-20
- [x] Phase 7: Summarization Engine (1/1 plan) — completed 2026-03-20
- [x] Phase 8: Compaction Core (2/2 plans) — completed 2026-03-20
- [x] Phase 9: HTTP Integration (1/1 plan) — completed 2026-03-20

</details>

<details>
<summary>✅ v1.2 Authentication / API Keys (Phases 10-14) — SHIPPED 2026-03-21</summary>

- [x] Phase 10: Auth Schema Foundation (2/2 plans) — completed 2026-03-20
- [x] Phase 11: KeyService Core (1/1 plan) — completed 2026-03-21
- [x] Phase 12: Auth Middleware (1/1 plan) — completed 2026-03-21
- [x] Phase 13: HTTP Wiring and REST Key Endpoints (2/2 plans) — completed 2026-03-21
- [x] Phase 14: CLI Key Management (2/2 plans) — completed 2026-03-21

</details>

<details>
<summary>✅ v1.3 CLI (Phases 15-20) — SHIPPED 2026-03-21</summary>

- [x] Phase 15: serve subcommand + CLI scaffolding (1/1 plan) — completed 2026-03-21
- [x] Phase 16: recall subcommand (2/2 plans) — completed 2026-03-21
- [x] Phase 17: remember subcommand (2/2 plans) — completed 2026-03-21
- [x] Phase 18: search subcommand (2/2 plans) — completed 2026-03-21
- [x] Phase 19: compact subcommand (2/2 plans) — completed 2026-03-21
- [x] Phase 20: output polish (2/2 plans) — completed 2026-03-21

</details>

<details>
<summary>✅ v1.4 Pluggable Storage Backends (Phases 21-25) — SHIPPED 2026-03-22</summary>

- [x] Phase 21: Storage Trait and SQLite Backend (2/2 plans) — completed 2026-03-21
- [x] Phase 22: Config Extension, Backend Factory, and Config CLI (2/2 plans) — completed 2026-03-21
- [x] Phase 23: Qdrant Backend (2/2 plans) — completed 2026-03-21
- [x] Phase 24: Postgres Backend (2/2 plans) — completed 2026-03-21
- [x] Phase 25: Config Redaction Fix & Tech Debt Cleanup (1/1 plan) — completed 2026-03-22

</details>

<details>
<summary>✅ v1.5 gRPC (Phases 26-29) — SHIPPED 2026-03-22</summary>

- [x] Phase 26: Proto Foundation (2/2 plans) — completed 2026-03-22
- [x] Phase 27: Dual-Server Skeleton and Auth Layer (2/2 plans) — completed 2026-03-22
- [x] Phase 28: Core RPC Handlers, Health, and Discoverability (2/2 plans) — completed 2026-03-22
- [x] Phase 29: StorageBackend Routing Fix (1/1 plan) — completed 2026-03-22

</details>

### v1.6 Web UI/Dashboard (In Progress)

**Milestone Goal:** Embed a lightweight operational dashboard into the mnemonic binary for visual memory exploration, agent/session monitoring, and compaction triggering — served at `/ui`, feature-gated behind `dashboard`, zero impact on default binary.

- [x] **Phase 30: Dashboard Foundation** — Build pipeline, rust-embed integration, feature gate, and CI wiring (completed 2026-03-22)
- [x] **Phase 31: Core UI** — Auth flow, memory browsing, search, agent breakdown, and GET /stats endpoint (completed 2026-03-23)
- [ ] **Phase 32: Operational Actions** — Compaction panel with dry-run diff preview and UI polish

## Phase Details

### Phase 30: Dashboard Foundation
**Goal**: The `dashboard` Cargo feature compiles, the binary serves the embedded SPA at `/ui`, and CI verifies both the dashboard build and the default binary regression gate
**Depends on**: Phase 29
**Requirements**: BUILD-01, BUILD-02, BUILD-03
**Success Criteria** (what must be TRUE):
  1. `cargo build --features dashboard` succeeds and `GET /ui/` returns `200 text/html` with the embedded app shell
  2. `cargo build` (default features, no dashboard) produces a binary with identical behavior to v1.5 — all 286 existing tests pass unchanged
  3. CI release workflow runs `npm ci && npm run build` in `dashboard/` before `cargo build --release --features dashboard`, producing a release artifact with an embedded UI
  4. A separate CI job runs `cargo build` (default features) + `cargo test` as a regression gate — failure blocks the release
  5. Build fails with a clear compile-time error if `--features dashboard` is set but `dashboard/dist/index.html` is missing
**Plans:** 2/2 plans complete
Plans:
- [x] 30-01-PLAN.md — Frontend scaffold, Rust feature gate, developer docs
- [x] 30-02-PLAN.md — Full-router integration tests + CI release workflow
**UI hint**: yes

### Phase 31: Core UI
**Goal**: Users can browse, filter, and search memories from the dashboard, see per-agent breakdowns, and the dashboard correctly handles auth-gated deployments
**Depends on**: Phase 30
**Requirements**: BROWSE-01, BROWSE-02, BROWSE-03, BROWSE-04, BROWSE-05, OPS-01, AUTH-01, AUTH-02
**Success Criteria** (what must be TRUE):
  1. User can view a paginated memory list showing content preview, agent_id, session_id, tags, and created_at — and filter it by agent_id, session_id, or tag without a page reload
  2. User can type a query into the search bar and see semantically ranked results with distance scores returned from `GET /memories/search`
  3. User can expand a memory row to read its full content and all metadata fields
  4. User can view a per-agent breakdown table (memory count, last-active timestamp) populated from the new `GET /stats` endpoint
  5. Dashboard header shows a live health indicator and active storage backend name from `GET /health`; when API keys are active, the dashboard prompts for an `mnk_...` bearer token stored only in component state (never localStorage), and all `/ui/` responses include a Content-Security-Policy header
**Plans:** 4/4 plans complete
Plans:
- [x] 31-01-PLAN.md — Backend: GET /stats endpoint + CSP header + integration tests
- [x] 31-02-PLAN.md — App shell: auth gate, hash router, header, tab bar, login screen
- [ ] 31-03-PLAN.md — Memories tab: paginated table, filters, expandable rows
- [ ] 31-04-PLAN.md — Search tab + Agents tab: semantic search with distance bars, agent breakdown
**UI hint**: yes

### Phase 32: Operational Actions
**Goal**: Users can trigger memory compaction from the dashboard with a dry-run diff preview before committing, and all UI surfaces handle empty states and async loading gracefully
**Depends on**: Phase 31
**Requirements**: OPS-02
**Success Criteria** (what must be TRUE):
  1. User can select an agent scope and trigger a dry-run compaction, seeing a before/after diff (N memories → M compacted) before any data is mutated
  2. User can confirm the compaction after reviewing the dry-run diff, executing `POST /memories/compact` and seeing the result reflected in the memory list
  3. All dashboard views display appropriate empty states (zero memories, zero agents, zero search results) instead of blank or broken layouts
  4. All async data fetches show loading skeleton states while in flight, and unhandled API errors surface via an error boundary rather than a silent blank panel
**Plans**: TBD
**UI hint**: yes

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation | v1.0 | 3/3 | Complete | 2026-03-19 |
| 2. Embedding | v1.0 | 2/2 | Complete | 2026-03-19 |
| 3. Service and API | v1.0 | 3/3 | Complete | 2026-03-19 |
| 4. Distribution | v1.0 | 2/2 | Complete | 2026-03-19 |
| 5. Config Cleanup | v1.0 | 1/1 | Complete | 2026-03-20 |
| 6. Foundation | v1.1 | 2/2 | Complete | 2026-03-20 |
| 7. Summarization Engine | v1.1 | 1/1 | Complete | 2026-03-20 |
| 8. Compaction Core | v1.1 | 2/2 | Complete | 2026-03-20 |
| 9. HTTP Integration | v1.1 | 1/1 | Complete | 2026-03-20 |
| 10. Auth Schema Foundation | v1.2 | 2/2 | Complete | 2026-03-20 |
| 11. KeyService Core | v1.2 | 1/1 | Complete | 2026-03-21 |
| 12. Auth Middleware | v1.2 | 1/1 | Complete | 2026-03-21 |
| 13. HTTP Wiring and REST Key Endpoints | v1.2 | 2/2 | Complete | 2026-03-21 |
| 14. CLI Key Management | v1.2 | 2/2 | Complete | 2026-03-21 |
| 15. serve subcommand + CLI scaffolding | v1.3 | 1/1 | Complete | 2026-03-21 |
| 16. recall subcommand | v1.3 | 2/2 | Complete | 2026-03-21 |
| 17. remember subcommand | v1.3 | 2/2 | Complete | 2026-03-21 |
| 18. search subcommand | v1.3 | 2/2 | Complete | 2026-03-21 |
| 19. compact subcommand | v1.3 | 2/2 | Complete | 2026-03-21 |
| 20. output polish | v1.3 | 2/2 | Complete | 2026-03-21 |
| 21. Storage Trait and SQLite Backend | v1.4 | 2/2 | Complete | 2026-03-21 |
| 22. Config Extension, Backend Factory, and Config CLI | v1.4 | 2/2 | Complete | 2026-03-21 |
| 23. Qdrant Backend | v1.4 | 2/2 | Complete | 2026-03-21 |
| 24. Postgres Backend | v1.4 | 2/2 | Complete | 2026-03-21 |
| 25. Config Redaction Fix & Tech Debt Cleanup | v1.4 | 1/1 | Complete | 2026-03-22 |
| 26. Proto Foundation | v1.5 | 2/2 | Complete | 2026-03-22 |
| 27. Dual-Server Skeleton and Auth Layer | v1.5 | 2/2 | Complete | 2026-03-22 |
| 28. Core RPC Handlers, Health, and Discoverability | v1.5 | 2/2 | Complete | 2026-03-22 |
| 29. StorageBackend Routing Fix | v1.5 | 1/1 | Complete | 2026-03-22 |
| 30. Dashboard Foundation | v1.6 | 2/2 | Complete    | 2026-03-22 |
| 31. Core UI | v1.6 | 2/4 | Complete    | 2026-03-23 |
| 32. Operational Actions | v1.6 | 0/TBD | Not started | - |
