# Phase 32: Operational Actions - Research

**Researched:** 2026-03-22
**Domain:** Preact dashboard — compaction UI, state machines, API client extension
**Confidence:** HIGH

## Summary

Phase 32 is the final v1.6 phase. All infrastructure (Preact + Vite + CSS variables, state machine pattern, apiFetch wrapper, SkeletonRows, ErrorMessage, AbortController cleanup) is fully established in Phase 31. This phase adds one new tab (Compact) with a two-step dry-run flow, one new presentational component (ClusterPreview), two new API wrappers (compactMemories, fetchMemoryById), and minor modifications to TabBar and App. It also audits existing tabs for state coverage gaps.

The existing tab audit (captured in UI-SPEC, decision D-10) found **no gaps** in MemoriesTab, AgentsTab, SearchTab, or Header. All four tabs already implement the full `loading | loaded | empty | error` state machine. Phase 32 implementation is therefore entirely additive — no existing component logic needs rework.

One critical backend discovery: **GET /memories/{id} does not exist**. The server.rs router only exposes DELETE /memories/{id}. The UI-SPEC calls for fetchMemoryById to call `GET /memories/{id}` for source preview content — this endpoint must be added to the backend as part of this phase.

**Primary recommendation:** Implement in three waves: (1) backend GET /memories/{id} route + api.ts wrappers, (2) CompactTab state machine and ClusterPreview component, (3) TabBar/App wiring and cross-tab refresh verification.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** New "Compact" tab — fourth tab alongside Memories | Agents | Search. Hash route `#/compact`. Keeps the only write operation separated from read-only browsing.
- **D-02:** Tab contains: agent scope dropdown (populated from GET /stats), similarity threshold input (default 0.85), and "Run Dry Run" button.
- **D-03:** max_candidates stays at API default (100) — not exposed in UI.
- **D-04:** Two-step flow: dry-run first (mandatory), then confirm to execute. No way to skip the preview.
- **D-05:** After compaction completes, other tabs auto-refresh on next navigation. Each tab already fetches on mount — no cross-tab state needed, just ensure tabs re-fetch when activated.
- **D-06:** Summary line at top: "N clusters found, M memories → K compacted".
- **D-07:** Cluster table below summary showing each cluster with source memory content previews grouped together (tree-style: `├` / `└` prefixes).
- **D-08:** After dry-run returns source_ids, fetch each source memory by ID (GET /memories/{id} or batch) to display content previews. Extra API calls but essential for the user to judge clustering quality.
- **D-09:** Confirm Compact + Cancel buttons below the cluster table. Confirm calls POST /memories/compact without dry_run flag.
- **D-10:** Audit existing tabs (Memories, Agents, Search) for missing empty/loading/error states. Fill any gaps using existing SkeletonRows and ErrorMessage components.
- **D-11:** New Compact tab follows the same `loading | loaded | empty | error` state machine pattern established in Phase 31.
- **D-12:** Per-tab error handling (existing pattern). No global error boundary. Each tab catches its own errors and renders ErrorMessage inline.

### Claude's Discretion
- Compact tab component file structure and naming
- Threshold input control (slider vs text input vs both)
- How to fetch source memories for preview (individual GET or batch approach)
- Empty state message for Compact tab when no agents exist
- Loading state during dry-run execution (may take time for large memory sets)
- Post-compaction success message design

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| OPS-02 | User can trigger compaction with dry-run preview showing before/after memory mapping, then confirm to execute | CompactRequest/CompactResponse shapes confirmed in compaction.rs; two-step flow via dry_run flag; POST /memories/compact exists; GET /memories/{id} must be added for source preview content |
</phase_requirements>

---

## Standard Stack

### Core (already installed — no new deps needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| preact | ^10.29.0 | UI framework (signal-based, React-compatible hooks API) | Established in Phase 30 |
| typescript | ^5.9.3 | Type safety | Established in Phase 30 |
| vite | ^8.0.1 | Build tool | Established in Phase 30 |
| tailwindcss | ^4.2.2 | CSS utilities | Established in Phase 30 |
| vite-plugin-singlefile | ^2.3.2 | Single-file output for Rust embedding | Established in Phase 30 |

**No new npm packages required for Phase 32.** All UI needs are covered by the existing CSS variable system + inline styles + Preact hooks.

### Backend — Rust (already established)

| Crate | Purpose | Relevant to Phase 32 |
|-------|---------|----------------------|
| axum | HTTP routing | GET /memories/{id} endpoint must be added |
| serde / serde_json | JSON serialization | CompactResponse already derived; Memory struct already serialized |

**Installation:** None needed.

---

## Architecture Patterns

### Established Project Structure (Phase 31)

```
dashboard/src/
├── App.tsx               # App shell — hash router, auth state machine, tab render
├── api.ts                # Typed API client — apiFetch, all endpoint wrappers
├── index.css             # CSS variables (@theme block) — no changes needed
└── components/
    ├── TabBar.tsx         # Tab bar — ADD 'compact' tab entry
    ├── MemoriesTab.tsx    # Memories tab — no changes (audit: complete)
    ├── AgentsTab.tsx      # Agents tab — no changes (audit: complete)
    ├── SearchTab.tsx      # Search tab — no changes (audit: complete)
    ├── Header.tsx         # Health indicator — no changes (audit: complete)
    ├── SkeletonRows.tsx   # Reusable loading skeleton
    ├── ErrorMessage.tsx   # Reusable inline error display
    ├── FilterBar.tsx      # Reusable filter dropdowns — NOT used in CompactTab
    ├── MemoryRow.tsx      # Memory row — NOT modified
    ├── DistanceBar.tsx    # Distance bar — NOT modified
    ├── Pagination.tsx     # Pagination — NOT modified
    ├── LoginScreen.tsx    # Login — NOT modified
    ├── CompactTab.tsx     # NEW: compaction state machine + controls
    └── ClusterPreview.tsx # NEW: presentational cluster tree display
```

### Pattern 1: Tab State Machine

All tabs use a discriminated union state type. CompactTab extends this with two extra states for the two-step flow.

```typescript
// Source: dashboard/src/components/MemoriesTab.tsx (established pattern)
// Standard 4-state machine (other tabs):
type TabState =
  | { kind: 'loading' }
  | { kind: 'loaded'; data: T }
  | { kind: 'empty' }
  | { kind: 'error'; message: string }

// CompactTab extension (6 states per UI-SPEC):
type CompactState =
  | { kind: 'idle' }
  | { kind: 'loading-dry-run' }
  | { kind: 'preview'; result: CompactResponse; memories: Map<string, Memory> }
  | { kind: 'loading-execute' }
  | { kind: 'success'; result: CompactResponse }
  | { kind: 'empty' }        // clusters_found === 0 after dry-run
  | { kind: 'error'; message: string }
```

### Pattern 2: AbortController Cleanup in useEffect

```typescript
// Source: dashboard/src/components/AgentsTab.tsx (established pattern)
useEffect(() => {
  const controller = new AbortController()
  setState({ kind: 'loading' })

  fetchStats(token, controller.signal)
    .then(data => { /* ... */ })
    .catch(err => {
      if (err instanceof UnauthorizedError) { onUnauthorized(); return }
      if (err.name === 'AbortError') return
      setState({ kind: 'error', message: 'Failed to load data. Reload the page or check server status.' })
    })

  return () => controller.abort()
}, [token, onUnauthorized])
```

### Pattern 3: apiFetch POST with JSON Body

```typescript
// Source: dashboard/src/api.ts (apiFetch wrapper — inferred from existing GET pattern)
export async function compactMemories(
  token: string | null,
  params: CompactParams,
  signal?: AbortSignal | null,
): Promise<CompactResponse> {
  const resp = await apiFetch('/memories/compact', token, signal, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(params),
  })
  return resp.json()
}
```

### Pattern 4: Tab Props Interface

```typescript
// Source: all existing tab components (established pattern)
interface CompactTabProps {
  token: string | null
  onUnauthorized: () => void
}
```

### Pattern 5: Agent Dropdown Populated from /stats

```typescript
// Source: dashboard/src/components/MemoriesTab.tsx (established pattern)
useEffect(() => {
  const controller = new AbortController()
  fetchStats(token, controller.signal)
    .then(data => setAgentOptions(data.agents.map(a => a.agent_id)))
    .catch(err => {
      if (err instanceof UnauthorizedError) { onUnauthorized(); return }
      // Non-critical: dropdown just won't pre-populate
    })
  return () => controller.abort()
}, [token, onUnauthorized])
```

### Pattern 6: Button Style (Primary CTA)

```typescript
// Source: dashboard/src/components/SearchTab.tsx "Search Memories" button
{
  padding: '8px 16px',
  fontSize: '14px',
  fontWeight: 600,
  fontFamily: 'inherit',
  background: 'var(--color-accent)',
  color: 'var(--color-bg)',
  border: 'none',
  borderRadius: '4px',
  cursor: disabled ? 'default' : 'pointer',
  opacity: disabled ? 0.6 : 1,
}
```

### Pattern 7: Empty State

```typescript
// Source: dashboard/src/components/AgentsTab.tsx (established pattern)
// Centered, padding 48px 0, heading 14px/400, body 12px/400/muted, marginTop 8px
<div style={{ textAlign: 'center', padding: '48px 0' }}>
  <div style={{ fontSize: '14px', fontWeight: 400, color: 'var(--color-text)' }}>
    Heading text
  </div>
  <div style={{ fontSize: '12px', fontWeight: 400, color: 'var(--color-text-muted)', marginTop: '8px' }}>
    Detail text
  </div>
</div>
```

### Anti-Patterns to Avoid

- **Global error boundary:** Decision D-12 explicitly requires per-tab inline error handling. Do not add a React/Preact error boundary wrapper around the tab panel.
- **Cross-tab state sync:** Decision D-05 explicitly relies on per-mount fetching. Do not add a global store, context, or signal for tab data.
- **Skipping dry-run:** Decision D-04 — the UI must not expose any path to run compaction without reviewing the dry-run diff first.
- **Exposing max_candidates:** Decision D-03 — keep it at API default (100), not in UI.
- **Token in localStorage:** Prior decision (Phase 31) — token is component state only, never persisted.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Auth header injection | Custom fetch wrapper | `apiFetch` in `api.ts` | Already handles token, timeout, AbortSignal, UnauthorizedError |
| Loading skeleton | Custom shimmer | `SkeletonRows` component | Already exists, consistent style |
| Inline error display | Custom error component | `ErrorMessage` component | Already exists, uses --color-error |
| Abort on unmount | Manual flag | AbortController + useEffect cleanup | Established pattern, prevents stale state updates |
| CSS theming | Hardcoded hex values | CSS variables (--color-*) | 7 variables cover all needed colors |
| Agent dropdown data | Separate stats fetch | Reuse `fetchStats()` | Already used by MemoriesTab and SearchTab |
| Tree-drawing characters | Custom icons | Unicode box-drawing: `\u251C` (├) and `\u2514` (└) | Already specified in UI-SPEC, zero dependency |

**Key insight:** This codebase is explicitly no-library for UI — zero third-party UI components, icon packs, or animation libraries. Every new component follows the inline-styles + CSS-variables + Preact hooks pattern established in Phase 30-31.

---

## Runtime State Inventory

Step 2.5 SKIPPED — Phase 32 is not a rename/refactor/migration phase. It is additive: new frontend tab component and one new backend route.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js | dashboard build | ✓ | v24.13.0 | — |
| npm | package management | ✓ | 11.6.2 | — |
| Rust/cargo | backend route addition | assumed ✓ (project builds) | — | — |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** None.

---

## Common Pitfalls

### Pitfall 1: GET /memories/{id} Does Not Exist

**What goes wrong:** The UI-SPEC and CONTEXT.md D-08 specify fetching source memory content via `GET /memories/{id}` for cluster preview. This endpoint does NOT exist in server.rs — only DELETE /memories/{id} is registered. Calling a non-existent route returns 404, apiFetch throws ApiError, and the cluster preview shows raw IDs instead of content.

**Why it happens:** The LIST endpoint (`GET /memories`) returns paginated content but there is no single-memory GET. The compact endpoint only returns source_ids, not content.

**How to avoid:** Add `GET /memories/{id}` to server.rs and the memory service before implementing the frontend fetch. The handler can reuse the existing `list_memories` service logic filtered by ID, or a new `get_memory(id)` service method.

**Warning signs:** 404 responses on `/memories/{some-uuid}` in browser DevTools Network tab.

### Pitfall 2: Empty agent_id String Sent to POST /memories/compact

**What goes wrong:** The backend validates that `agent_id` is non-empty and returns 400 BadRequest. If the agent dropdown shows `(none)` for memories with an empty agent_id and the user selects it, an empty string gets sent.

**Why it happens:** The agent dropdown stores `""` as the value for the "(none)" option (consistent with other tabs). But the compact endpoint requires a non-empty agent_id.

**How to avoid:** Disable the "Run Dry Run" button when the selected agent value is an empty string. The placeholder "Select agent..." option with value `""` already handles this — just check `!selectedAgent` in the disabled condition. The "(none)" option for empty agent_id should only appear if the backend actually has memories with empty agent_id; those memories are compactable, so the empty string IS a valid agent_id for the compact API (the server does `agent_id.trim().is_empty()` check). Verify this edge case.

**Warning signs:** 400 response from POST /memories/compact with message "agent_id must not be empty".

### Pitfall 3: Parallel fetchMemoryById Calls Race Against AbortController

**What goes wrong:** The preview state loads memory content for potentially many source IDs. If the user clicks "Discard Preview" while fetches are in flight, state updates from completed fetches can overwrite the newly-reset idle state.

**Why it happens:** Individual promises not tied to a single AbortController lifecycle.

**How to avoid:** Use a single AbortController per dry-run fetch sequence. Pass its signal to all fetchMemoryById calls. Abort the controller when the user discards the preview or the component unmounts. Store fetched memories in component state only after checking that the abort signal hasn't fired.

**Warning signs:** Memory content appearing in the UI after clicking "Discard Preview".

### Pitfall 4: threshold Sent as String Instead of Number

**What goes wrong:** `<input type="number">` value is always a string in JavaScript/TypeScript. POST /memories/compact with `threshold: "0.85"` instead of `threshold: 0.85` may fail JSON deserialization in Rust (serde expects f32, gets a string).

**Why it happens:** `(e.target as HTMLInputElement).value` returns a string.

**How to avoid:** Parse with `parseFloat()` before including in the request params. Validate that the result is a finite number in [0, 1] before enabling the button.

**Warning signs:** 400 or 422 response from POST /memories/compact, or serde deserialization error in server logs.

### Pitfall 5: CompactTab Mounts with No Agents in /stats

**What goes wrong:** If no memories have been stored yet, `/stats` returns `{ agents: [] }`. The agent dropdown shows only the placeholder "Select agent..." option. The user cannot run a dry-run. The tab must show an appropriate empty state instead of a useless form.

**Why it happens:** No agents means no agent_id to select; the dry-run button is always disabled.

**How to avoid:** After fetching stats, if `agents.length === 0`, render the "no agents" empty state ("No agents available. Agents will appear here once memories are stored.") instead of the controls form. This is specified in the UI-SPEC copywriting contract.

**Warning signs:** User sees a disabled "Run Dry Run" button with no explanation.

### Pitfall 6: TabBar.tsx Tab Type Mismatch

**What goes wrong:** TabBar exports `type Tab = 'memories' | 'agents' | 'search'`. App.tsx imports this type. Adding 'compact' to TabBar without updating App.tsx parseHash() and the conditional render block causes TypeScript type errors.

**Why it happens:** The Tab type union is used in three places (TabBar definition, App.tsx parseHash, App.tsx render conditions) and all three must be updated atomically.

**How to avoid:** Update all three in a single task: TabBar.tsx Tab type + TABS array, App.tsx parseHash(), App.tsx tab render block.

---

## Code Examples

### API Wrapper: compactMemories

```typescript
// Source: api.ts pattern from fetchMemories/searchMemories + compaction.rs types
export interface CompactParams {
  agent_id: string
  threshold?: number
  dry_run?: boolean
}

export interface ClusterMapping {
  source_ids: string[]
  new_id: string | null
}

export interface CompactResponse {
  run_id: string
  clusters_found: number
  memories_merged: number
  memories_created: number
  id_mapping: ClusterMapping[]
  truncated: boolean
}

export async function compactMemories(
  token: string | null,
  params: CompactParams,
  signal?: AbortSignal | null,
): Promise<CompactResponse> {
  const resp = await apiFetch('/memories/compact', token, signal, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(params),
  })
  return resp.json()
}
```

### API Wrapper: fetchMemoryById

```typescript
// Source: api.ts apiFetch pattern; endpoint must be added to server.rs
export async function fetchMemoryById(
  token: string | null,
  id: string,
  signal?: AbortSignal | null,
): Promise<Memory> {
  const resp = await apiFetch(`/memories/${id}`, token, signal)
  return resp.json()
}
```

### Backend Route Addition (server.rs)

```rust
// Add to protected router in build_router():
.route("/memories/{id}", get(get_memory_handler).delete(delete_memory_handler))
// Or as separate routes if axum requires it for multi-method on same path
```

### TabBar Update

```typescript
// Source: dashboard/src/components/TabBar.tsx
// Change:
export type Tab = 'memories' | 'agents' | 'search' | 'compact'

const TABS: { id: Tab; label: string; href: string }[] = [
  { id: 'memories', label: 'Memories', href: '#/memories' },
  { id: 'agents', label: 'Agents', href: '#/agents' },
  { id: 'search', label: 'Search', href: '#/search' },
  { id: 'compact', label: 'Compact', href: '#/compact' },
]
```

### App.tsx parseHash Update

```typescript
// Source: dashboard/src/App.tsx
function parseHash(): Tab {
  const hash = window.location.hash
  if (hash === '#/agents') return 'agents'
  if (hash === '#/search') return 'search'
  if (hash === '#/compact') return 'compact'
  return 'memories'
}
```

### ClusterPreview Presentational Component (Sketch)

```typescript
// Source: UI-SPEC Dry-Run Diff Display section + established inline style pattern
interface ClusterPreviewProps {
  clusterIndex: number
  sourceIds: string[]
  memories: Map<string, Memory>  // pre-fetched, passed from parent
}

export default function ClusterPreview({ clusterIndex, sourceIds, memories }: ClusterPreviewProps) {
  return (
    <div style={{ padding: '8px 0', borderBottom: '1px solid var(--color-border)' }}>
      <div style={{ fontSize: '12px', color: 'var(--color-text-muted)', marginBottom: '4px' }}>
        Cluster {clusterIndex + 1}
      </div>
      {sourceIds.map((id, i) => {
        const memory = memories.get(id)
        const isLast = i === sourceIds.length - 1
        const prefix = isLast ? '\u2514 ' : '\u251C '  // └ or ├
        const content = memory
          ? memory.content.length > 80 ? memory.content.slice(0, 80) + '...' : memory.content
          : id  // fallback to raw ID if fetch failed
        return (
          <div key={id} style={{ display: 'flex', gap: '4px', fontSize: '14px', color: 'var(--color-text)' }}>
            <span style={{ fontSize: '12px', color: 'var(--color-text-muted)', flexShrink: 0 }}>
              {prefix}
            </span>
            <span>{content}</span>
          </div>
        )
      })}
    </div>
  )
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Global error boundary | Per-tab inline error (D-12) | Phase 31 decision | No Preact ErrorBoundary class needed |
| History routing | Hash routing (D-04, Phase 31) | Phase 30 decision | parseHash() is the only router |
| Multi-file vite output | Single-file (vite-plugin-singlefile) | Phase 30 decision | Dashboard is one index.html embedded in Rust binary |

**Deprecated/outdated:**
- Tailwind v3 class-based approach: project uses Tailwind v4 with `@theme` block in index.css and CSS variables, not utility classes on elements.

---

## Open Questions

1. **GET /memories/{id} — does the MemoryService have a `get_by_id` method or must one be added?**
   - What we know: server.rs only has DELETE /memories/{id}. The service module has `delete_memory(id)` but no confirmed `get_memory(id)`.
   - What's unclear: whether `src/service.rs` already has a `get_memory` or `find_by_id` method that can be wired to a new GET handler with minimal code.
   - Recommendation: Read `src/service.rs` during planning or early Wave 0 to determine if it's a handler-only addition or a service+handler addition.

2. **How should parallel fetchMemoryById calls be structured?**
   - What we know: D-08 says to fetch each source_id individually. Multiple clusters can have multiple source IDs (default max_candidates=100 means up to ~50 clusters of 2 each in worst case).
   - What's unclear: Whether Promise.all (all in parallel) or sequential fetching is better for the UX and server load.
   - Recommendation: Use Promise.all with a single shared AbortController. Show per-cluster "Loading..." placeholders while the batch resolves. This is marked as Claude's Discretion.

3. **Axum route conflict: /memories/{id} with GET + DELETE vs /memories/compact with POST**
   - What we know: axum matches routes in registration order. `GET /memories/{id}` could potentially match `GET /memories/compact` if registered after. However, `/memories/compact` uses POST only, so no conflict for GET.
   - What's unclear: Whether adding `GET /memories/{id}` (a parameterized route) conflicts with any other `/memories/...` GET routes (only `/memories` GET and `/memories/search` GET exist).
   - Recommendation: The existing DELETE /memories/{id} already uses a parameterized route without conflict; adding GET to the same route is safe. The planner should combine into `.route("/memories/{id}", get(get_memory_handler).delete(delete_memory_handler))`.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | None detected (no jest.config.*, vitest.config.*, or test scripts in package.json) |
| Config file | None |
| Quick run command | `cd dashboard && npm run build` (type-check + build as proxy for tests) |
| Full suite command | `cargo test` (backend Rust unit tests) |

**No automated frontend test infrastructure exists.** The dashboard has no test files, no testing framework configured, and no test scripts in package.json. Validation for this phase is build-time (TypeScript compilation) + manual browser verification.

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OPS-02 (dry-run flow) | POST /memories/compact with dry_run:true returns CompactResponse | manual | — | N/A (no frontend test infra) |
| OPS-02 (execute flow) | POST /memories/compact with dry_run:false mutates data | manual | — | N/A |
| OPS-02 (backend GET) | GET /memories/{id} returns single Memory | cargo test (integration if exists) | `cargo test` | ❌ new |
| OPS-02 (type safety) | TypeScript compiles without errors | build | `cd dashboard && npm run build` | ✓ |

### Sampling Rate

- **Per task commit:** `cd dashboard && npm run build` — catches TypeScript errors
- **Per wave merge:** `cargo test && cd dashboard && npm run build` — catches both layers
- **Phase gate:** Full suite green + manual browser test of dry-run → confirm flow before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `src/service.rs` — verify if `get_memory(id)` method exists or needs adding
- [ ] Backend integration test for `GET /memories/{id}` — new route needs basic test coverage matching existing handler test pattern in Rust tests

*(No frontend test infrastructure to create — existing pattern is build-only validation for the dashboard layer)*

---

## Sources

### Primary (HIGH confidence)
- Direct code inspection: `dashboard/src/App.tsx`, `api.ts`, all component files — patterns verified from source
- Direct code inspection: `src/server.rs` — route inventory confirmed; GET /memories/{id} absence confirmed
- Direct code inspection: `src/compaction.rs` — CompactRequest/CompactResponse/ClusterMapping types verified
- Direct inspection: `.planning/phases/32-operational-actions/32-CONTEXT.md` — all decisions D-01 through D-12
- Direct inspection: `.planning/phases/32-operational-actions/32-UI-SPEC.md` — component inventory, state machine, copywriting, API contract
- Direct inspection: `dashboard/package.json` — no test framework present

### Secondary (MEDIUM confidence)
- None required — all findings are from direct source inspection

### Tertiary (LOW confidence)
- None

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified from package.json and existing source files
- Architecture: HIGH — all patterns extracted from existing working components
- Pitfalls: HIGH — pitfalls 1, 4, 6 verified against actual source; pitfalls 2, 3, 5 from established patterns
- Missing endpoint (GET /memories/{id}): HIGH — confirmed absent from server.rs router

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (stable stack; no external dependency changes expected)
