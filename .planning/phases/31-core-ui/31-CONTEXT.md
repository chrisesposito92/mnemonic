# Phase 31: Core UI - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Auth flow, memory browsing with filtering, semantic search, per-agent breakdown, and GET /stats endpoint — the dashboard becomes a functional tool for exploring memories. No compaction UI (Phase 32).

</domain>

<decisions>
## Implementation Decisions

### Dashboard layout & navigation
- **D-01:** Tab bar layout with three tabs: Memories | Agents | Search. Each tab maps to a hash route (#/memories, #/agents, #/search).
- **D-02:** Persistent header above tabs showing health indicator (green/red dot) and active storage backend name from GET /health.
- **D-03:** Per-tab filters — each tab has its own relevant filter controls. No global filter bar.
- **D-04:** Manual hash router (~20 lines, useState + hashchange listener). No third-party routing library. Switch statement renders the active tab component.

### Memory list presentation
- **D-05:** Table rows layout — dense table with columns: content preview (truncated), agent_id, session_id, tags, created_at. Developer-oriented, monospace font.
- **D-06:** Inline expansion — clicking a row expands it in-place to show full content, id, embedding_model, created_at (full timestamp), updated_at. Click again to collapse.
- **D-07:** Offset pagination using existing `limit` + `offset` params on GET /memories. Shows total count ("Showing 1-20 of 347"). Prev/next buttons with page indicator.
- **D-08:** Memories tab filter controls: agent_id, session_id, and tag dropdowns. Filtering updates without page reload.

### Search experience
- **D-09:** Dedicated Search tab (separate from Memories browse tab). Query input + optional agent_id and tag filters.
- **D-10:** Search results displayed as ranked table with distance scores.
- **D-11:** Distance scores shown as visual bar (filled proportional to similarity) plus raw numeric value for quick-scan and precision.

### Auth prompt flow
- **D-12:** Full-screen login gate — when auth is detected (401 on /health probe), entire dashboard shows centered login screen with token input and "Connect" button. Nothing accessible until authenticated.
- **D-13:** Auth detection via probe to GET /health on mount. 200 = open mode (show dashboard). 401 = auth active (show login screen). Reuses the health check already in the HealthCard pattern.
- **D-14:** Invalid token handling — stay on login screen with inline error message ("Invalid API key"), clear field, let user retry. Never navigate to dashboard with a bad token.
- **D-15:** Token stored in Preact component state only — never localStorage (carried from Phase 30 decision). Token passed to all fetch calls as Authorization header.

### Claude's Discretion
- Exact component file structure and naming
- API client abstraction (if any) vs inline fetch calls
- Content preview truncation length
- Relative time formatting approach (e.g., "2m ago")
- Empty state designs for each tab (zero memories, zero agents, zero search results)
- Loading skeleton patterns for each view (extending the HealthCard skeleton pattern)
- GET /stats endpoint response shape and implementation across storage backends
- CSP header content and placement (AUTH-02)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — BROWSE-01 through BROWSE-05, OPS-01, AUTH-01, AUTH-02
- `.planning/ROADMAP.md` §Phase 31 — Success criteria (5 items)

### Existing dashboard code
- `dashboard/src/App.tsx` — Current app shell (will be replaced with tab layout)
- `dashboard/src/main.tsx` — Entry point, render to #mnemonic-root
- `dashboard/src/index.css` — Tailwind v4 theme with CSS variables (--color-bg, --color-surface, --color-border, --color-text, --color-text-muted, --color-accent, --color-error)
- `dashboard/src/components/HealthCard.tsx` — Existing component with loading/loaded/error state pattern, Row helper

### Backend API
- `src/server.rs` — All REST handlers: health, list_memories, search_memories, create_memory, delete_memory, compact_memories, list_keys
- `src/service.rs` — Memory, ListParams, SearchParams, ListResponse, SearchResponse, SearchResultItem structs
- `src/storage/mod.rs` — StorageBackend trait (7 methods) — GET /stats will need a new method or query across backends

### Phase 30 context
- `.planning/phases/30-dashboard-foundation/30-CONTEXT.md` — Prior decisions D-01 through D-15, build pipeline choices

### Risk noted in STATE.md
- GET /stats with Qdrant backend requires non-SQL aggregation path — inspect `src/storage/qdrant.rs` during implementation

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `HealthCard.tsx` — Loading skeleton pattern (3 gray bars), error state pattern, Row component for label:value display, fetch + AbortSignal.timeout pattern
- `index.css` — 7 CSS variables for consistent theming across all new components
- `ListParams` struct — Supports agent_id, session_id, tag, after, before, limit, offset filters (ready for memory list)
- `SearchParams` struct — Supports q, agent_id, session_id, tag, limit, threshold, after, before (ready for search tab)
- `ListResponse` — Returns `{ memories: Memory[], total: u64 }` (total enables pagination display)

### Established Patterns
- Preact + TypeScript with hooks (useState, useEffect) — no class components
- Inline styles with CSS variable references (not Tailwind utility classes in most places)
- Monospace font applied via inline style on root div
- State machine pattern (`CardState = 'loading' | 'loaded' | 'error'`) for async data

### Integration Points
- `App.tsx` — Will be rewritten to contain the tab bar and hash router
- `GET /health` — Already consumed; will be expanded to power both header indicator and auth detection
- `GET /memories` — Paginated list endpoint ready for Memories tab
- `GET /memories/search` — Semantic search endpoint ready for Search tab
- `GET /stats` — **Does not exist yet** — needs new endpoint + StorageBackend method for agent breakdown (BROWSE-05)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches for the dashboard UI implementation.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 31-core-ui*
*Context gathered: 2026-03-22*
