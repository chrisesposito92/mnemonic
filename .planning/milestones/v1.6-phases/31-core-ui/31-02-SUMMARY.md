---
phase: 31-core-ui
plan: "02"
subsystem: dashboard-frontend
tags: [preact, typescript, auth, api-client, hash-router, components]
dependency_graph:
  requires: [31-01]
  provides: [31-03, 31-04]
  affects: [dashboard/src/api.ts, dashboard/src/App.tsx, dashboard/src/components/]
tech_stack:
  added: []
  patterns:
    - AbortSignal.any() for combined caller-abort + timeout
    - Discriminated union state machine (AppState: checking | login | dashboard)
    - Health-endpoint-based auth detection (auth_enabled field, not 401 probe)
    - useRef for stable callback reference across renders
key_files:
  created:
    - dashboard/src/api.ts
    - dashboard/src/components/Header.tsx
    - dashboard/src/components/TabBar.tsx
    - dashboard/src/components/LoginScreen.tsx
    - dashboard/src/components/SkeletonRows.tsx
    - dashboard/src/components/ErrorMessage.tsx
  modified:
    - dashboard/src/App.tsx
decisions:
  - "fetchHealth never sends auth token -- health endpoint is always public (review concern #2)"
  - "apiFetch throws UnauthorizedError on 401/403 so callers can trigger re-auth (review concern #8)"
  - "AbortSignal.any() combines caller signal + timeout so both cancellation paths work (review concern #6)"
  - "handleUnauthorized stored in useRef so Plans 03/04 tab components can receive it as onUnauthorized prop"
  - "LoginScreen validates against /memories?limit=1 not /health (health is public, need real auth test)"
  - "TypeScript noUnusedLocals: handleUnauthorized exposed via useRef pattern to satisfy strict check while keeping callback available"
metrics:
  duration: "334s (~5.5 min)"
  completed_date: "2026-03-23"
  tasks_completed: 2
  files_changed: 7
---

# Phase 31 Plan 02: App Shell (API Client + Auth Gate + Components) Summary

**One-liner:** Typed API client with AbortController+auth+401-interception, auth gate using health endpoint's auth_enabled field (not 401 probe), hash router, 30s health polling header, three-tab navigation, and shared skeleton/error UI primitives.

## What Was Built

### api.ts -- Typed fetch client
- `apiFetch(url, token, signal?, init?)`: auth header injection, AbortSignal.any() combining caller signal + 10s timeout, throws `UnauthorizedError` on 401/403
- `fetchHealth(signal?)`: never sends token (health is public per review concern #2)
- `fetchMemories`, `fetchStats`, `searchMemories`: typed wrappers for all dashboard-needed endpoints
- `ApiError` and `UnauthorizedError` error classes for structured error handling

### App.tsx -- Root coordinator
- Three-state AppState machine: `checking` (health probe in-flight) | `login` (auth required) | `dashboard` (open or authenticated)
- Auth detection via `health.auth_enabled` field (review concern #2) — no 401 probing
- Hash router: `parseHash()` + `hashchange` listener, defaults to `#/memories`
- `handleUnauthorized` callback wired via `useRef` for Plans 03/04 tab components (review concern #8)

### Header.tsx
- Health dot: green (color-accent) when ok, red (color-error) on error, gray (color-border) loading
- Backend name displayed in muted text
- Polls GET /health every 30s using setInterval + AbortController cleanup
- Token prop accepted but never passed to fetchHealth (public endpoint)

### TabBar.tsx
- Three tabs: Memories (#/memories), Agents (#/agents), Search (#/search)
- Active tab: color-text + 2px solid color-accent bottom border
- Inactive: color-text-muted + 2px transparent border (maintains layout stability)

### LoginScreen.tsx
- Full-screen centered login gate
- Validates token against `/memories?limit=1` (protected endpoint, not /health)
- `UnauthorizedError` -> "Invalid API key. Check your key and try again."
- Network errors -> "Could not reach API. Check that the server is running and reload the page."
- Accepts `error` prop for pre-populated message (e.g., session expiry from App.tsx)
- Token cleared from input on failed validation

### SkeletonRows.tsx
- Configurable `rows` prop (default 3)
- 12px height bars, color-border background, opacity 0.4, borderRadius 2px
- Matches HealthCard.tsx established pattern

### ErrorMessage.tsx
- Single-prop component: `message: string`
- 14px/400/color-error inline style

## Verification Results

| Check | Result |
|-------|--------|
| `npx tsc --noEmit` | PASS |
| `npx vite build` | PASS (dist/index.html 25.04 kB) |
| `cargo build --features dashboard` | PASS (2 pre-existing warnings) |
| No localStorage/sessionStorage | PASS (grep returns 0) |
| auth_enabled field check in App.tsx | PASS |
| 30_000 poll interval in Header.tsx | PASS |
| #/memories href in TabBar.tsx | PASS |
| mnk_ placeholder in LoginScreen.tsx | PASS |
| opacity: 0.4 in SkeletonRows.tsx | PASS |
| color-error in ErrorMessage.tsx | PASS |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] node_modules not installed in worktree**
- **Found during:** Task 1 (TypeScript check failed: cannot find module 'preact')
- **Issue:** The worktree was freshly created without running `npm install`. node_modules directory was absent.
- **Fix:** `npm install` in dashboard/ — added 114 packages from existing package-lock.json
- **Files modified:** dashboard/node_modules/ (not tracked in git, covered by .gitignore)
- **Commit:** Pre-task (unblocking step before Task 1 commit)

**2. [Rule 1 - Bug] TypeScript noUnusedLocals: handleUnauthorized callback**
- **Found during:** Task 2 (tsc --noEmit strict mode)
- **Issue:** `handleUnauthorized` defined in App.tsx but not yet consumed (tab components are placeholders). TypeScript's `noUnusedLocals: true` flagged it.
- **Fix:** Stored callback in `useRef` (standard Preact/React pattern for stable callback refs). This satisfies TypeScript (the ref IS used to assign `onUnauthorizedRef.current`) while making the callback available for Plans 03/04 via `onUnauthorizedRef.current`.
- **Files modified:** dashboard/src/App.tsx
- **Commit:** Part of Task 2 commit (bb742ff)

**3. [Rule 1 - Bug] UnauthorizedError imported but unused in App.tsx**
- **Found during:** Task 2 (tsc --noEmit strict mode)
- **Issue:** Plan template included `import { fetchHealth, UnauthorizedError } from './api'` but App.tsx doesn't directly catch UnauthorizedError (LoginScreen does).
- **Fix:** Removed UnauthorizedError from App.tsx import. App.tsx only uses fetchHealth; tab components (Plans 03/04) will import UnauthorizedError directly.
- **Files modified:** dashboard/src/App.tsx
- **Commit:** Part of Task 2 commit (bb742ff)

## Known Stubs

The following placeholder tab content divs exist in App.tsx (lines 110-128) and are **intentional stubs** — documented in the plan as placeholders for Plans 03 and 04:

| Stub | File | Purpose |
|------|------|---------|
| "Memories tab -- implemented in Plan 03" | dashboard/src/App.tsx | Replaced by MemoriesTab component in Plan 03 |
| "Agents tab -- implemented in Plan 04" | dashboard/src/App.tsx | Replaced by AgentsTab component in Plan 04 |
| "Search tab -- implemented in Plan 04" | dashboard/src/App.tsx | Replaced by SearchTab component in Plan 04 |

These stubs do not prevent Plan 02's goal (app shell with auth gate + navigation) from being achieved. Plans 03 and 04 will wire real content.

## Commits

| Hash | Task | Description |
|------|------|-------------|
| cbde537 | Task 1 | feat(31-02): create api.ts typed fetch client with auth, abort, 401 interception |
| bb742ff | Task 2 | feat(31-02): rewrite App.tsx + create Header, TabBar, LoginScreen, SkeletonRows, ErrorMessage |

## Self-Check: PASSED

| Item | Status |
|------|--------|
| dashboard/src/api.ts | FOUND |
| dashboard/src/App.tsx | FOUND |
| dashboard/src/components/Header.tsx | FOUND |
| dashboard/src/components/TabBar.tsx | FOUND |
| dashboard/src/components/LoginScreen.tsx | FOUND |
| dashboard/src/components/SkeletonRows.tsx | FOUND |
| dashboard/src/components/ErrorMessage.tsx | FOUND |
| dashboard/dist/index.html | FOUND |
| Commit cbde537 (api.ts) | FOUND |
| Commit bb742ff (components) | FOUND |
