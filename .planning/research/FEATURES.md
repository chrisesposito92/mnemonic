# Feature Research

**Domain:** Embedded operational web dashboard for a developer-facing memory server binary
**Researched:** 2026-03-22
**Confidence:** HIGH

## Scope Note

This research covers only the **new dashboard features for v1.6**. The existing REST API (9 endpoints), gRPC (4 RPCs), CLI (7 subcommands), and pluggable storage backends are already built and out of scope. The question is: what does a well-designed embedded operational dashboard look like for a developer tool like mnemonic — and what do developers expect from comparable tools (Prometheus UI, Qdrant Web UI, RedisInsight, VectorAdmin)?

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features that any developer-facing data store UI must have. Missing these makes the dashboard feel unfinished or broken.

| Feature | Why Expected | Complexity | API Dependency |
|---------|--------------|------------|----------------|
| Memory list table with pagination | Every database browser shows records in a paged table; the primary interaction surface | LOW | `GET /memories` with `limit`/`offset` params |
| Filter by `agent_id` | Multi-agent namespacing is a first-class mnemonic concept; all peers (Qdrant, RedisInsight) allow namespace/collection scoping | LOW | `GET /memories?agent_id=...` |
| Filter by `session_id` | Session grouping is a defined field; operators need to inspect individual sessions | LOW | `GET /memories?session_id=...` |
| Filter by `tag` | Tags are a user-defined field; a browser without tag filtering is frustrating | LOW | `GET /memories?tag=...` |
| Memory detail view | Clicking a row should expand/show full content, metadata, timestamps — comparable to Qdrant's "point detail panel" and RedisInsight's key detail view | LOW | `GET /memories/{id}` via existing list response; no extra endpoint needed |
| Semantic search bar | The core mnemonic value proposition is semantic search; the dashboard must expose it visually — Qdrant's console and Agent Zero's dashboard both feature search prominently | MEDIUM | `GET /memories/search?q=...&agent_id=...` |
| Health / status indicator | Prometheus UI, Qdrant dashboard, and every embedded admin panel show server health at a glance. Expected top-of-page persistent indicator | LOW | `GET /health` — returns `{status, backend}` |
| Active storage backend display | Users switching between SQLite/Qdrant/Postgres need to know which backend is live at a glance | LOW | `GET /health` — returns `backend` field |
| Delete memory action | All data browsers provide a delete affordance for individual records (RedisInsight, Qdrant points panel, Agent Zero memory dashboard) | LOW | `DELETE /memories/{id}` |
| Agent breakdown / namespace summary | VectorAdmin provides a "Database Statistics" component with per-namespace counts; Prometheus shows per-job breakdowns; operators need agent-level visibility | MEDIUM | Requires aggregation: `GET /memories?agent_id=X` with count strategy; no dedicated stats endpoint exists yet |
| Total memory count | The "number of records" is the first thing any database UI shows — Qdrant shows collection point count, Prometheus shows active series count | LOW | Count derived from `GET /memories` response; can be the total field from list response |
| Compaction trigger (dry-run + execute) | Compaction is an operational action; making it accessible from the UI reduces CLI dependency — Qdrant's UI has snapshot management as an in-browser action | MEDIUM | `POST /memories/compact` with `dry_run=true` then `dry_run=false` |
| Auth-aware behavior | If API keys are active, the dashboard must pass the key as `Authorization: Bearer mnk_...` or display a login prompt. All peer tools respect auth (Qdrant, Prometheus). Missing this = dashboard is broken when auth is on | MEDIUM | All existing protected endpoints; key entered once in UI session |
| Responsive to existing REST API | Dashboard must be a consumer of the existing API, not a parallel path. No new Rust backend logic for the happy path | LOW | All 9 existing REST endpoints — no new endpoints needed for core features |

### Differentiators (Competitive Advantage)

Features that peers don't have or implement poorly, which align with mnemonic's "zero-config, agent-first" positioning.

| Feature | Value Proposition | Complexity | API Dependency |
|---------|-------------------|------------|----------------|
| Agent activity timeline / "last seen" | Show each agent's most recent memory timestamp. RedisInsight does not show per-namespace last-active. This directly answers "which agents are using my mnemonic instance?" | MEDIUM | Derived from `GET /memories?agent_id=X&limit=1&sort=created_at` or summary endpoint |
| Compaction dry-run diff view | Show a before/after preview (N memories → M compacted) before committing. POST /memories/compact with dry_run=true returns the mapping. No peer tool surfaces this visually | MEDIUM | `POST /memories/compact` with `dry_run=true`; response includes `old_ids`, `new_id` mapping |
| Copy memory ID / content to clipboard | One-click copy for memory IDs and content. Reduces friction for pasting into agent code, curl commands, or issue reports. Simple feature with outsized DX value | LOW | No API dependency — client-side clipboard |
| Zero-config UI access | Dashboard served at `/ui` with no separate installation, no npm, no Docker. Peers (Qdrant, RedisInsight) require separate deployment or Electron app. This is a genuine differentiator for the single-binary story | LOW | Rust-embed + axum serve at `/ui` — no API dependency |
| Memory content preview in table | Show first 80 characters of content in the list row. Avoids the click-to-expand round-trip for quick scanning. Qdrant shows payload previews; Agent Zero's table shows content previews | LOW | Embedded in list response — no extra call |
| Tag display in list table | Show tags as colored badges in the table row. Makes filtered browsing faster. Qdrant shows payload fields inline | LOW | Embedded in list response |
| Backend badge in header | Persistent pill showing "sqlite" / "qdrant" / "postgres" in the header. Makes the active backend unambiguous at all times without navigating to a settings page | LOW | `GET /health` |
| Feature-gated — zero binary impact when off | When built without the `dashboard` feature flag, the binary is unchanged in size and behavior. Comparable to Qdrant and Prometheus which have optional UI modes | MEDIUM | Build-time feature gate via Cargo.toml; rust-embed only included when `dashboard` feature is active |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Write/edit memory from dashboard | Operators want to correct a stored memory | Modifying agent-written memories creates hidden state inconsistency; agents re-read memories and may behave unexpectedly if edited out-of-band; adds form complexity and validation logic | Use the REST API or CLI directly for edits; dashboard is read-and-observe, not write |
| Real-time auto-refresh (polling) | "Live" dashboard feels modern | Polling on short intervals hammers the server and embedding model if search is triggered; mnemonic is not an event-driven system; polling adds complexity for uncertain user value | Manual refresh button (used by Prometheus UI for time-range scrubbing); optional user-controlled refresh interval if requested |
| Memory graph / vector visualization | 2D vector projection like Qdrant's Visualize tab looks impressive | Requires UMAP/t-SNE dimensionality reduction; this is a compute-intensive operation that is not in the mnemonic codebase; adds a heavyweight JS dependency (d3, plotly); primary users are developers not researchers | Out of scope for v1.6; defer to v2+ if users request it |
| API key management UI (create/revoke) | "Everything should be in the dashboard" | Key creation must emit the raw key exactly once; doing this securely in a browser UI requires careful UX to prevent accidental disclosure; the CLI (`mnemonic keys create`) is already excellent for this | CLI handles key management; dashboard shows auth status (keys active / open mode) only |
| Dark/light theme toggle | Modern design | Adds CSS complexity; tailwind dark: classes require a theme toggle implementation; the dashboard is a developer ops tool, not a consumer product; default dark or default light is sufficient for v1.6 | Ship a single carefully-chosen default theme; theme toggle is a P3 enhancement |
| Bulk delete / bulk operations | "Select all + delete" | Dangerous without undo; bulk delete on wrong filter could destroy an agent's entire memory namespace; adds confirmation UX complexity | Individual delete with confirmation dialog; compaction as the safe "reduce" operation |
| Memory creation from dashboard | "Complete CRUD in the UI" | The dashboard is an operational visibility tool, not a memory authoring tool; agents write memories, operators observe them; a create form adds auth complexity and blurs the tool's purpose | CLI `mnemonic remember` and REST API are the write paths |
| Custom theme / branding | White-labeling for teams | Scope creep without clear user demand; the user base is developers who value utility over aesthetics | Plain Tailwind utility classes; clean and functional is sufficient |

---

## Feature Dependencies

```
[Dashboard served at /ui]
    └──requires──> [rust-embed crate embedding compiled frontend assets]
    └──requires──> [axum route at /ui/* serving embedded files]
    └──requires──> [dashboard Cargo feature flag activating the route and embed]
    └──requires──> [frontend build step (Preact + Tailwind + Vite) producing dist/]
    └──requires──> [dist/ committed to repo OR generated in CI before rust compile]

[Memory list table]
    └──requires──> [GET /memories with limit/offset — already built]
    └──enhances──> [filter bar (agent_id, session_id, tag)]

[Filter bar]
    └──requires──> [GET /memories?agent_id=&session_id=&tag=]
    └──enhances──> [memory list table]
    └──enhances──> [agent breakdown]

[Semantic search]
    └──requires──> [GET /memories/search?q=&agent_id=&limit= — already built]
    └──enhances──> [memory list table] (results replace list when query is active)

[Memory detail view]
    └──requires──> [memory object from list response — no extra endpoint]
    └──enhances──> [memory list table]

[Delete action]
    └──requires──> [DELETE /memories/{id} — already built]
    └──requires──> [memory list table] (triggers refresh after delete)

[Compaction panel]
    └──requires──> [POST /memories/compact — already built]
    └──requires──> [agent_id filter] (scope compaction to selected agent)
    └──enhances──> [compaction dry-run diff view]

[Agent breakdown view]
    └──requires──> [GET /memories (aggregate by agent_id on client, or new summary endpoint)]
    └──requires──> [memory list table]
    └──note: no dedicated aggregate endpoint exists; client can group list results]

[Auth-aware behavior]
    └──requires──> [localStorage or session key storage for API key]
    └──requires──> [Authorization: Bearer header on all API calls]
    └──requires──> [GET /health to detect open vs auth mode]
    └──conflicts──> [key management UI] (key creation is CLI-only per anti-features)

[Health indicator]
    └──requires──> [GET /health — already built, returns {status, backend}]
    └──no other dependencies]

[Feature gate (dashboard flag)]
    └──requires──> [rust-embed inclusion behind #[cfg(feature = "dashboard")]]
    └──requires──> [axum /ui route registration behind the same feature flag]
    └──conflicts──> [default binary size] (resolved by feature gating — zero cost when off)
```

### Dependency Notes

- **Agent breakdown has no dedicated API endpoint.** The existing `GET /memories` with `?agent_id=X` can be called per known agent, but there is no `GET /agents` or `GET /stats/agents` endpoint. The dashboard must either: (a) call `GET /memories?limit=1000` and group client-side, or (b) a new lightweight `GET /stats` endpoint returning per-agent counts is built as part of v1.6. Option (b) is strongly preferred because grouping 10,000 records client-side is wasteful.
- **Frontend build output must be available at Rust compile time.** rust-embed embeds the files from a directory path at compile time. This means `npm run build` (or equivalent Vite build) must run before `cargo build`. CI and local dev workflows need a documented build order. The `build.rs` script is the correct place to encode this dependency.
- **Auth detection via `GET /health` is insufficient alone.** The health endpoint does not indicate whether auth is currently active. A 401 response from `GET /memories` is the correct signal that auth is required. The dashboard should attempt `GET /memories` on load and prompt for an API key only if it receives 401.
- **Compaction requires agent_id scope.** The compaction endpoint supports per-agent scoping. The dashboard UI should require the user to select an agent before triggering compaction, preventing accidental cross-namespace compaction.

---

## MVP Definition

### Launch With (v1.6)

Minimum feature set for the dashboard to be useful as an operational visibility tool.

- [ ] Memory list table with content preview, agent_id, session_id, tags, created_at — **core value**
- [ ] Filter bar: agent_id, session_id, tag — **drives 80% of actual usage**
- [ ] Semantic search bar (query + agent_id scope) — **showcase mnemonic's core capability**
- [ ] Memory detail view (expand row or side panel) — **required for full content inspection**
- [ ] Delete memory action with confirmation — **needed to clean up test data**
- [ ] Health indicator + backend badge in header — **operational awareness at a glance**
- [ ] Agent breakdown table (per-agent count + last-active) — **requires new `GET /stats` endpoint or client-side grouping**
- [ ] Compaction panel with dry-run preview then execute — **the only write-side operational action that belongs in a dashboard**
- [ ] Auth-aware: detect open vs keyed mode, prompt for API key if 401, persist in sessionStorage — **required for any deployment with auth enabled**
- [ ] Feature-gated behind `dashboard` Cargo feature — **zero binary impact when off**

### Add After Validation (v1.6.x)

Features to add once the core dashboard is in use by real users.

- [ ] Manual refresh button — add when users report the table feeling stale
- [ ] Configurable page size (10 / 25 / 50) — add when users report scrolling pain on large datasets
- [ ] Copy-to-clipboard for memory ID and content — trivial to add, queue for first patch
- [ ] Timestamp display toggle (relative "2 hours ago" vs absolute ISO) — small DX win

### Future Consideration (v2+)

- [ ] Vector visualization (2D projection via UMAP) — requires embedding endpoint and heavy JS; defer until users explicitly request it
- [ ] Dark/light theme toggle — nice-to-have, not operational
- [ ] Session timeline view — useful but requires chronological ordering by session which the current list API supports; defer for user-validated demand
- [ ] Memory edit form — only if users consistently request it; high risk of data corruption

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Memory list table + pagination | HIGH | LOW | P1 |
| Filter bar (agent/session/tag) | HIGH | LOW | P1 |
| Health indicator + backend badge | HIGH | LOW | P1 |
| Auth-aware (detect 401, prompt for key) | HIGH | MEDIUM | P1 |
| Semantic search bar | HIGH | LOW | P1 |
| Memory detail view (expand) | HIGH | LOW | P1 |
| Delete memory action | MEDIUM | LOW | P1 |
| Agent breakdown table | HIGH | MEDIUM | P1 (requires new `/stats` endpoint or grouping strategy) |
| Compaction panel (dry-run + execute) | HIGH | MEDIUM | P1 |
| Feature gate (`dashboard` Cargo flag) | HIGH | LOW | P1 — architectural requirement |
| rust-embed build integration | HIGH | LOW | P1 — foundational |
| Copy-to-clipboard (ID, content) | MEDIUM | LOW | P2 |
| Manual refresh button | LOW | LOW | P2 |
| Configurable page size | LOW | LOW | P2 |
| Relative timestamp display | LOW | LOW | P2 |
| Dark/light theme toggle | LOW | MEDIUM | P3 |
| Vector visualization | LOW | HIGH | P3 |
| Memory edit form | LOW | HIGH | P3 |

**Priority key:**
- P1: Must have for v1.6 launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

---

## Competitor Feature Analysis

Analysis of comparable embedded operational dashboards: Prometheus web UI, Qdrant Web UI, RedisInsight, VectorAdmin, and Agent Zero's Memory Dashboard.

| Feature | Prometheus UI | Qdrant Web UI | RedisInsight | VectorAdmin | Our Approach |
|---------|---------------|---------------|--------------|-------------|--------------|
| Record browser | Targets table, not record rows | Points tab with search/filter | Key browser with type/TTL columns | Per-database point list | Memory table: content, agent_id, session_id, tags, created_at |
| Namespace/collection filter | Per-job filter in targets | Per-collection scoping | Per-DB instance | Per-database scoping | Per-agent_id filter (mnemonic's namespace primitive) |
| Semantic search | PromQL expression input | Semantic vector search panel | No semantic search (key-value) | Vector similarity search | Semantic search bar wired to `GET /memories/search` |
| Stats / breakdown | Metrics explorer, series counts | Collection point count, segments | DB memory usage, key count | Per-database statistics component | Agent breakdown: per-agent count + last-active timestamp |
| Operational actions | No in-UI actions | Snapshot create/restore | Bulk delete, SlowLog | Snapshot management | Compaction trigger: dry-run preview + execute |
| Health indicator | Always-on server status badge | Collection status indicator | DB connection status | Server status | Header pill: `{status, backend}` from `GET /health` |
| Auth | Basic auth or none | API key in request header | DB password modal | API key config | Detect 401 → prompt for `mnk_...` key → sessionStorage |
| Embedded vs separate deploy | Embedded in Prometheus binary | Embedded in Qdrant binary | Separate Electron/web app | Separate Node.js server | Embedded in mnemonic binary via rust-embed, `/ui` route |
| Feature gating | UI always on | UI always on | Always separate | Always separate | Cargo feature flag — zero binary cost when off |
| Delete | Not applicable | Point delete in panel | Individual + bulk delete | Not documented | Individual delete with confirmation modal |

---

## New API Endpoint Required

The agent breakdown view (per-agent memory count + last-active timestamp) cannot be efficiently served by the existing `GET /memories` endpoint without either:

1. Fetching all records and grouping client-side (unacceptable at >1K memories)
2. Making N `GET /memories?agent_id=X&limit=1` calls (one per agent — no agent enumeration endpoint exists)

**Recommendation:** Add a lightweight `GET /stats` endpoint as part of v1.6 that returns:

```json
{
  "total_memories": 1247,
  "agents": [
    { "agent_id": "claude-agent", "count": 892, "last_active": "2026-03-22T14:30:00Z" },
    { "agent_id": "gpt-pilot", "count": 355, "last_active": "2026-03-21T09:15:00Z" }
  ],
  "backend": "sqlite"
}
```

This endpoint:
- Requires a single DB query (`SELECT agent_id, COUNT(*), MAX(created_at) FROM memories GROUP BY agent_id`)
- Returns no memory content (no privacy/auth concern beyond existing list auth)
- Should be behind auth middleware like all other `/memories*` routes
- Enables the agent breakdown view without N+1 queries

This is the only new REST endpoint needed for v1.6. All other dashboard features use existing endpoints.

---

## Sources

- [Qdrant Web UI documentation](https://qdrant.tech/documentation/web-ui/) — points tab, collections browser, search panel, snapshot management. MEDIUM confidence.
- [PromLabs — Prometheus 3.0 UI overview](https://promlabs.com/blog/2024/09/11/a-look-at-the-new-prometheus-3-0-ui/) — metrics explorer, tree view, dark mode, embedded React (Mantine) frontend. HIGH confidence.
- [Agent Zero Memory Dashboard (DeepWiki)](https://deepwiki.com/agent0ai/agent-zero/5.6-memory-dashboard) — semantic search, area filters, detail panel, CRUD actions, read-only dashboard pattern. MEDIUM confidence.
- [VectorAdmin GitHub](https://github.com/Mintplex-Labs/vector-admin) — per-namespace statistics component, point counts, activity graphs, multi-backend management. MEDIUM confidence.
- [RedisInsight documentation](https://redis.io/docs/latest/develop/tools/insight/) — key browser with bulk delete, SlowLog, Workbench, real-time metrics. HIGH confidence.
- [rust-embed crates.io](https://crates.io/crates/rust-embed) — compile-time file embedding with feature-gate support, compression (brotli/gzip). HIGH confidence.
- [axum-embed crates.io](https://crates.io/crates/axum-embed) — axum service for rust-embed, SPA fallback routing to index.html. HIGH confidence.
- [Preact vs React bundle size comparison (2025)](https://medium.com/@marketing_96787/preact-vs-react-in-2025-which-javascript-framework-delivers-the-best-performance-f2ded55808a4) — Preact ~3KB gzipped vs React ~40KB; Preact Signals for reactive state. MEDIUM confidence.
- [Axum + rust-embed SPA pattern](https://github.com/tokio-rs/axum/discussions/1309) — serving embedded SPA with fallback to index.html; confirmed pattern for axum 0.7+. MEDIUM confidence.
- [Dashboard UX patterns — Pencil & Paper](https://www.pencilandpaper.io/articles/ux-pattern-analysis-data-dashboards) — operational dashboard design: status-first layout, filter-then-paginate, real-time vs manual refresh tradeoffs. MEDIUM confidence.
- [Data table UX patterns — Pencil & Paper](https://www.pencilandpaper.io/articles/ux-pattern-analysis-enterprise-data-tables) — 25-row default pages, top-level search + filter combination, row expand for detail. MEDIUM confidence.

---
*Feature research for: Mnemonic v1.6 Embedded Web Dashboard*
*Researched: 2026-03-22*
