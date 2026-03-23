# Phase 32: Operational Actions - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Compaction panel with dry-run diff preview and execute confirmation, plus audit and gap-fill of empty/loading/error states across all dashboard tabs. This is the final phase of v1.6 — after this, the embedded dashboard is feature-complete.

</domain>

<decisions>
## Implementation Decisions

### Compaction UI placement
- **D-01:** New "Compact" tab — fourth tab alongside Memories | Agents | Search. Hash route `#/compact`. Keeps the only write operation separated from read-only browsing.
- **D-02:** Tab contains: agent scope dropdown (populated from GET /stats), similarity threshold input (default 0.85), and "Run Dry Run" button.
- **D-03:** max_candidates stays at API default (100) — not exposed in UI. Too technical for most users.

### Compaction flow
- **D-04:** Two-step flow: dry-run first (mandatory), then confirm to execute. No way to skip the preview.
- **D-05:** After compaction completes, other tabs auto-refresh on next navigation. Each tab already fetches on mount — no cross-tab state needed, just ensure tabs re-fetch when activated.

### Dry-run diff display
- **D-06:** Summary line at top: "N clusters found, M memories → K compacted".
- **D-07:** Cluster table below summary showing each cluster with source memory content previews grouped together (tree-style: `├` / `└` prefixes).
- **D-08:** After dry-run returns source_ids, fetch each source memory by ID (GET /memories/{id} or batch) to display content previews. Extra API calls but essential for the user to judge clustering quality.
- **D-09:** Confirm Compact + Cancel buttons below the cluster table. Confirm calls POST /memories/compact without dry_run flag.

### UI polish
- **D-10:** Audit existing tabs (Memories, Agents, Search) for missing empty/loading/error states. Fill any gaps using existing `SkeletonRows` and `ErrorMessage` components.
- **D-11:** New Compact tab follows the same `loading | loaded | empty | error` state machine pattern established in Phase 31.
- **D-12:** Per-tab error handling (existing pattern). No global error boundary. Each tab catches its own errors and renders `ErrorMessage` inline.

### Claude's Discretion
- Compact tab component file structure and naming
- Threshold input control (slider vs text input vs both)
- How to fetch source memories for preview (individual GET or batch approach)
- Empty state message for Compact tab when no agents exist
- Loading state during dry-run execution (may take time for large memory sets)
- Post-compaction success message design

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — OPS-02 (compaction with dry-run preview and confirm)
- `.planning/ROADMAP.md` §Phase 32 — Success criteria (4 items: dry-run preview, confirm execute, empty states, loading/error states)

### Existing dashboard code
- `dashboard/src/App.tsx` — App shell with hash router, auth state machine, tab rendering, handleUnauthorized pattern
- `dashboard/src/api.ts` — Typed API client: apiFetch wrapper, UnauthorizedError, fetchMemories, fetchStats, searchMemories, fetchHealth
- `dashboard/src/components/MemoriesTab.tsx` — Reference implementation of `loading | loaded | empty | error` state machine with SkeletonRows, ErrorMessage, filters, pagination
- `dashboard/src/components/SearchTab.tsx` — Search tab with distance bars, agent/tag filters
- `dashboard/src/components/AgentsTab.tsx` — Agent breakdown table from GET /stats
- `dashboard/src/components/SkeletonRows.tsx` — Reusable loading skeleton component
- `dashboard/src/components/ErrorMessage.tsx` — Reusable error display component
- `dashboard/src/components/FilterBar.tsx` — Filter controls pattern (agent, session, tag dropdowns)
- `dashboard/src/components/TabBar.tsx` — Tab bar component (needs Compact tab added)
- `dashboard/src/index.css` — CSS variables for theming (--color-bg, --color-surface, --color-border, --color-text, etc.)

### Backend compaction API
- `src/compaction.rs` — CompactRequest (agent_id, threshold, max_candidates, dry_run), CompactResponse (run_id, clusters_found, memories_merged, memories_created, id_mapping with ClusterMapping)
- `src/server.rs` — compact_memories_handler at POST /memories/compact with auth scope enforcement

### Phase 31 context
- `.planning/phases/31-core-ui/31-CONTEXT.md` — Prior decisions D-01 through D-15 (tab layout, auth flow, state machine patterns)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SkeletonRows` component — loading skeleton with configurable row count
- `ErrorMessage` component — inline error display
- `FilterBar` component — dropdown filter controls (agent, session, tag)
- `apiFetch` wrapper — auth header injection, timeout, abort signal, UnauthorizedError on 401/403
- `fetchStats` — already fetches agent list for dropdown population
- CSS variables — 7 theme variables for consistent styling

### Established Patterns
- State machine: `type TabState = { kind: 'loading' } | { kind: 'loaded'; ... } | { kind: 'empty' } | { kind: 'error'; message: string }`
- AbortController cleanup in useEffect for preventing stale responses
- `onUnauthorized` prop passed to each tab from App.tsx
- Token passed as prop from App.tsx to all tabs
- Inline styles with CSS variable references

### Integration Points
- `App.tsx` — Add 'compact' to Tab type, add `#/compact` to parseHash(), render CompactTab
- `TabBar.tsx` — Add Compact tab button
- `api.ts` — Add `compactMemories(token, params)` and `fetchMemoryById(token, id)` wrappers
- POST /memories/compact — Existing endpoint, supports dry_run, agent_id, threshold

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches within the established dashboard patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 32-operational-actions*
*Context gathered: 2026-03-22*
