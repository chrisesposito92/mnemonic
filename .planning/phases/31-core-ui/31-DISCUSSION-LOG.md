# Phase 31: Core UI - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-22
**Phase:** 31-core-ui
**Areas discussed:** Dashboard layout & navigation, Memory list presentation, Search experience, Auth prompt flow

---

## Dashboard Layout & Navigation

### View organization

| Option | Description | Selected |
|--------|-------------|----------|
| Tab bar | Horizontal tabs (Memories, Agents, Search) with hash routes, health in persistent header | ✓ |
| Single page with sections | All views stacked vertically, search as filter bar | |
| Sidebar navigation | Vertical sidebar on left, content on right | |

**User's choice:** Tab bar
**Notes:** Clean separation per view, each tab is a hash route

### Filter placement

| Option | Description | Selected |
|--------|-------------|----------|
| Per-tab filters | Each tab has its own relevant filters | ✓ |
| Global filter bar | Persistent filter bar below tabs applies globally | |
| You decide | Claude picks | |

**User's choice:** Per-tab filters
**Notes:** Keeps each view focused

### Router implementation

| Option | Description | Selected |
|--------|-------------|----------|
| Manual hash router | useState + hashchange listener, ~20 lines, zero deps | ✓ |
| preact-router | Third-party routing library | |
| You decide | Claude picks | |

**User's choice:** Manual hash router
**Notes:** Matches lightweight Preact spirit, only 3 static routes

---

## Memory List Presentation

### Display style

| Option | Description | Selected |
|--------|-------------|----------|
| Table rows | Dense table with columns, click to expand | ✓ |
| Cards | Spacious cards with content preview, metadata below | |

**User's choice:** Table rows
**Notes:** Fits operational/developer tool aesthetic

### Pagination

| Option | Description | Selected |
|--------|-------------|----------|
| Offset pagination | Page numbers or prev/next, uses existing limit+offset params | ✓ |
| Load more button | Append on click, no random page access | |
| You decide | Claude picks | |

**User's choice:** Offset pagination
**Notes:** Simple, matches the API

### Expanded detail

| Option | Description | Selected |
|--------|-------------|----------|
| Inline expansion | Row expands in-place showing full content and metadata | ✓ |
| Side panel | Slide-out panel on right | |
| Modal dialog | Centered modal overlay | |

**User's choice:** Inline expansion
**Notes:** No modal or side panel needed

---

## Search Experience

### Search surfacing

| Option | Description | Selected |
|--------|-------------|----------|
| Dedicated Search tab | Own tab with query input, filters, ranked results | ✓ |
| Unified search + browse | Single view switching between browse and search modes | |

**User's choice:** Dedicated Search tab
**Notes:** Separate from Memories browse tab

### Distance score display

| Option | Description | Selected |
|--------|-------------|----------|
| Raw distance value | Numeric distance as-is (0.12, 0.45) | |
| Percentage similarity | Convert to percentage (88% match) | |
| Visual bar + number | Horizontal bar proportional to similarity plus raw number | ✓ |
| You decide | Claude picks | |

**User's choice:** Visual bar + number
**Notes:** Quick-scan visual plus precision

---

## Auth Prompt Flow

### Auth gate UX

| Option | Description | Selected |
|--------|-------------|----------|
| Full-screen login | Centered login screen, nothing accessible until authenticated | ✓ |
| Inline banner | Warning banner at top with token input | |
| Modal overlay | Modal dialog over dimmed dashboard | |

**User's choice:** Full-screen login
**Notes:** Clean gate, token input with Connect button

### Auth detection

| Option | Description | Selected |
|--------|-------------|----------|
| Probe /health on load | Fetch GET /health on mount, 200=open, 401=auth needed | ✓ |
| Try first real request | Attempt first API call, switch to login on 401 | |
| You decide | Claude picks | |

**User's choice:** Probe /health on load
**Notes:** Fast, reuses existing health check pattern

### Invalid token handling

| Option | Description | Selected |
|--------|-------------|----------|
| Stay on login with error | Inline error, clear field, retry | ✓ |
| Navigate then redirect | Accept token, try data, bounce on 401 | |
| You decide | Claude picks | |

**User's choice:** Stay on login screen with error
**Notes:** Never navigate to dashboard with bad token

---

## Claude's Discretion

- Component file structure and naming
- API client abstraction approach
- Content preview truncation length
- Relative time formatting
- Empty state designs
- Loading skeleton patterns
- GET /stats endpoint shape
- CSP header implementation

## Deferred Ideas

None — discussion stayed within phase scope.
