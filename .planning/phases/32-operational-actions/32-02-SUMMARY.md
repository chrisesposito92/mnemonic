---
phase: 32-operational-actions
plan: "02"
subsystem: dashboard-frontend
tags: [frontend, preact, typescript, compaction, ui]
dependency_graph:
  requires: [32-01]
  provides: [compact-tab-ui, cluster-preview, compaction-api-types]
  affects: [dashboard/src/api.ts, dashboard/src/App.tsx, dashboard/src/components/TabBar.tsx]
tech_stack:
  added: []
  patterns:
    - 7-state machine (idle/loading-dry-run/preview/loading-execute/success/empty/error)
    - Preview invalidation via useEffect watching input changes
    - Chunked Promise.allSettled for graceful partial-failure handling
    - __none__ sentinel pattern for blank agent_id disambiguation
key_files:
  created:
    - dashboard/src/components/CompactTab.tsx
    - dashboard/src/components/ClusterPreview.tsx
  modified:
    - dashboard/src/api.ts
    - dashboard/src/components/TabBar.tsx
    - dashboard/src/App.tsx
decisions:
  - "Preview invalidation on input change: useEffect watching selectedAgent/threshold auto-discards preview state, ensuring users cannot confirm compaction computed with stale params (addresses Codex HIGH)"
  - "__none__ sentinel for blank agent_id: empty string '' means unselected placeholder; '__none__' represents agents with agent_id='' from the API, translated back to '' before API calls (addresses Codex HIGH)"
  - "Promise.allSettled with CHUNK_SIZE=5: individual memory fetch failures degrade to showing raw IDs instead of triggering re-auth or failing the entire preview (addresses Gemini/Codex MEDIUM)"
  - "id_mapping.length for dry-run compacted count: memories_created is 0 in dry-run mode so we derive the count from id_mapping.length (number of clusters = new memories to be created) (addresses Codex MEDIUM)"
  - "API functions added to api.ts in this plan: Plan 01 ran concurrently and handles the Rust backend; this plan adds the TypeScript api.ts wrappers to unblock CompactTab"
metrics:
  duration: "253s"
  completed_date: "2026-03-23T03:26:53Z"
  tasks_completed: 2
  files_changed: 5
---

# Phase 32 Plan 02: Compact Tab UI Summary

Preact CompactTab component with 7-state dry-run flow wired into the dashboard shell as the fourth tab at #/compact.

## What Was Built

**CompactTab.tsx** — Full compaction state machine with agent dropdown populated from GET /stats, threshold input (default 0.85), mandatory dry-run preview before execution, confirm/discard controls, and all 7 states: idle, loading-dry-run, preview, loading-execute, success, empty, error.

**ClusterPreview.tsx** — Presentational component for a single cluster showing source memories with Unicode tree-drawing prefixes (U+251C tee, U+2514 corner), content truncated at 80 chars, graceful fallback to raw memory ID if content fetch fails.

**api.ts additions** — CompactParams, ClusterMapping, CompactResponse interfaces plus compactMemories() and fetchMemoryById() typed wrappers (added here to unblock this plan since 32-01 ran concurrently handling the Rust backend).

**TabBar.tsx** — Tab type union extended to include 'compact', fourth TABS entry added.

**App.tsx** — CompactTab import, #/compact hash route in parseHash(), CompactTab render block.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create ClusterPreview and CompactTab components | e98d7c6 | dashboard/src/api.ts, dashboard/src/components/ClusterPreview.tsx, dashboard/src/components/CompactTab.tsx |
| 2 | Wire CompactTab into TabBar and App.tsx | 36ecdc8 | dashboard/src/components/TabBar.tsx, dashboard/src/App.tsx |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added API types and functions to api.ts in this plan**
- **Found during:** Task 1
- **Issue:** Plan 32-01 (which adds compactMemories, fetchMemoryById, CompactParams, ClusterMapping, CompactResponse to api.ts) ran concurrently and its SUMMARY.md was not present. The CompactTab component requires these types/functions to compile.
- **Fix:** Added CompactParams, ClusterMapping, CompactResponse interfaces and compactMemories()/fetchMemoryById() functions to api.ts as part of this plan's execution.
- **Files modified:** dashboard/src/api.ts
- **Commit:** e98d7c6
- **Note:** If 32-01 also adds these same exports, there will be a duplicate-definition conflict at merge time. The orchestrator will need to resolve any merge conflicts in api.ts between the two parallel plans.

**2. [Rule 3 - Blocking] Ran npm install before TypeScript check**
- **Found during:** Task 1 verification
- **Issue:** dashboard/node_modules did not exist in the worktree, causing tsc to fail with "Cannot find module 'preact/hooks'" across all files.
- **Fix:** Ran npm install to install dependencies before running tsc --noEmit.
- **Files modified:** none (node_modules not committed)

## Verification Results

- `npx tsc --noEmit` — PASSED (0 errors)
- `npx vite build` — PASSED (dist/index.html 48.29 kB gzip: 14.13 kB)
- `cargo build --features dashboard` — PASSED (2 pre-existing dead_code warnings, not from this plan)

## Success Criteria Verification

1. CompactTab implements all 7 states: idle, loading-dry-run, preview, loading-execute, success, empty, error — DONE
2. ClusterPreview renders source memories with tree-drawing prefixes (U+251C/U+2514) and 80-char truncation — DONE
3. Dry-run mandatory before execution (D-04) — DONE (no execute path without going through preview state)
4. Agent dropdown populated from GET /stats, threshold defaults to 0.85 — DONE
5. max_candidates NOT exposed in UI (D-03) — DONE (CompactParams has no max_candidates)
6. Dry-run summary uses id_mapping.length not memories_created — DONE
7. Cluster table with tree prefixes — DONE via ClusterPreview
8. Confirm Compact and Discard Preview buttons — DONE
9. Changing agent/threshold auto-discards preview — DONE via useEffect watching [selectedAgent, threshold]
10. Blank agent_id uses __none__ sentinel — DONE
11. Promise.allSettled in chunks of 5 with graceful degradation — DONE
12. Truncation warning when result.truncated === true — DONE
13. Threshold validated as finite number in [0,1] — DONE via isValidThreshold()
14. Individual fetchMemoryById failures do not trigger onUnauthorized — DONE (Promise.allSettled silently skips rejected entries)
15. Empty state for no compactable clusters with threshold hint — DONE
16. Empty state for no agents — DONE
17. Success state with "Run another compaction" link — DONE
18. Error state with ErrorMessage component inline — DONE
19. All async operations use AbortController with cleanup — DONE
20. Threshold parsed as float before API call — DONE
21. All existing tabs verified to have empty states — DONE (MemoriesTab, AgentsTab, SearchTab all have kind: 'empty')
22. Full build chain: tsc + vite + cargo — DONE

## Known Stubs

None. All states are fully implemented. The CompactTab calls real API endpoints (compactMemories, fetchStats, fetchMemoryById). Note: the Rust backend GET /memories/{id} endpoint is added in Plan 32-01 — if 32-01 did not complete before this dashboard is deployed, fetchMemoryById will return 404s which degrade gracefully to showing raw memory IDs in ClusterPreview (no crash, no re-auth trigger).

## Self-Check: PASSED

Files exist:
- dashboard/src/components/CompactTab.tsx — FOUND
- dashboard/src/components/ClusterPreview.tsx — FOUND
- dashboard/src/api.ts (modified) — FOUND

Commits exist:
- e98d7c6 (Task 1) — FOUND
- 36ecdc8 (Task 2) — FOUND
