# Phase 32: Operational Actions - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-22
**Phase:** 32-operational-actions
**Areas discussed:** Compaction UX flow, Diff preview display, UI polish scope

---

## Compaction UX Flow

### Where should compaction live?

| Option | Description | Selected |
|--------|-------------|----------|
| New "Compact" tab | Fourth tab alongside Memories/Agents/Search. Dedicated space, clean hash route. | ✓ |
| Panel inside Memories tab | Controls below memory table. Consolidated but mixes read/write. | |
| Modal from header | Button in header opens overlay. Accessible from any tab. | |

**User's choice:** New "Compact" tab
**Notes:** User initially questioned whether the dashboard should be read-only. After confirming OPS-02 is in scope (compaction is the one exception), chose dedicated tab for clean separation.

### Post-compaction refresh behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-refresh on tab switch | Tabs re-fetch fresh data when navigated to after compaction. | ✓ |
| Manual refresh only | Tabs keep existing data until explicit reload. | |

**User's choice:** Auto-refresh on tab switch (Recommended)

### Parameter exposure

| Option | Description | Selected |
|--------|-------------|----------|
| Threshold only | Expose similarity threshold (default 0.85). max_candidates stays at default. | ✓ |
| Both configurable | Expose threshold + max_candidates. More control, more complexity. | |
| Defaults only | No configuration — just agent scope + dry-run button. | |

**User's choice:** Threshold only (Recommended)

---

## Diff Preview Display

### How to display dry-run results

| Option | Description | Selected |
|--------|-------------|----------|
| Summary + cluster table | Summary line + table listing each cluster with source memory content previews. | ✓ |
| Summary only | Just numbers: clusters, memories merged. No breakdown. | |
| Collapsible clusters | Same as cluster table but collapsible per cluster. | |

**User's choice:** Summary + cluster table (Recommended)

### Fetching source memory content

| Option | Description | Selected |
|--------|-------------|----------|
| Fetch source memories | After dry-run, fetch each source memory by ID for content previews. | ✓ |
| Show IDs only | Display source memory IDs without content. | |

**User's choice:** Fetch source memories (Recommended)

---

## UI Polish Scope

### Polish thoroughness

| Option | Description | Selected |
|--------|-------------|----------|
| Audit + fill gaps | Review all tabs for missing states, fix gaps, apply to Compact tab. | ✓ |
| Compact tab only | Only ensure new tab has proper states. | |
| Full redesign | Redesign all states across every tab with custom visuals. | |

**User's choice:** Audit + fill gaps (Recommended)

### Error boundary strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Per-tab (existing pattern) | Each tab catches own errors, shows ErrorMessage inline. | ✓ |
| Global + per-tab | Add top-level error boundary as last-resort, keep per-tab handling. | |

**User's choice:** Per-tab (existing pattern) (Recommended)

---

## Claude's Discretion

- Compact tab component file structure and naming
- Threshold input control (slider vs text input vs both)
- How to fetch source memories for preview (individual GET or batch)
- Empty state message for Compact tab
- Loading state during dry-run execution
- Post-compaction success message design

## Deferred Ideas

None — discussion stayed within phase scope.
