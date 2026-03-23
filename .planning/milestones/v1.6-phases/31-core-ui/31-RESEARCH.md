# Phase 31: Core UI - Research

**Researched:** 2026-03-22
**Domain:** Preact + TypeScript dashboard UI; Rust/axum backend extension (GET /stats endpoint, CSP header)
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Tab bar layout with three tabs: Memories | Agents | Search. Each tab maps to a hash route (#/memories, #/agents, #/search).
- **D-02:** Persistent header above tabs showing health indicator (green/red dot) and active storage backend name from GET /health.
- **D-03:** Per-tab filters — each tab has its own relevant filter controls. No global filter bar.
- **D-04:** Manual hash router (~20 lines, useState + hashchange listener). No third-party routing library. Switch statement renders the active tab component.
- **D-05:** Table rows layout — dense table with columns: content preview (truncated), agent_id, session_id, tags, created_at. Developer-oriented, monospace font.
- **D-06:** Inline expansion — clicking a row expands it in-place to show full content, id, embedding_model, created_at (full timestamp), updated_at. Click again to collapse.
- **D-07:** Offset pagination using existing `limit` + `offset` params on GET /memories. Shows total count ("Showing 1-20 of 347"). Prev/next buttons with page indicator.
- **D-08:** Memories tab filter controls: agent_id, session_id, and tag dropdowns. Filtering updates without page reload.
- **D-09:** Dedicated Search tab (separate from Memories browse tab). Query input + optional agent_id and tag filters.
- **D-10:** Search results displayed as ranked table with distance scores.
- **D-11:** Distance scores shown as visual bar (filled proportional to similarity) plus raw numeric value for quick-scan and precision.
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

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| BROWSE-01 | User can view a paginated list of memories showing content preview, agent_id, session_id, tags, and created_at | GET /memories already returns ListResponse{memories, total}; ListParams supports limit+offset |
| BROWSE-02 | User can filter memory list by agent_id, session_id, and tag | GET /memories accepts agent_id, session_id, tag query params; no backend changes needed |
| BROWSE-03 | User can perform semantic search from the dashboard and see ranked results with distance scores | GET /memories/search returns SearchResponse{memories[{...memory, distance}]}; distance is lower-is-better f64 |
| BROWSE-04 | User can expand a memory row to see full content and metadata | Memory struct has: id, content, agent_id, session_id, tags, embedding_model, created_at, updated_at — all fields available from list response |
| BROWSE-05 | User can view per-agent memory counts and last-active timestamps via agent breakdown table | Requires new GET /stats endpoint + StorageBackend::stats() method across all three backends |
| OPS-01 | Dashboard header shows health indicator with active storage backend name from GET /health | GET /health already returns {status, backend}; AppState.backend_name already set |
| AUTH-01 | Dashboard detects auth mode via 401 response, prompts for mnk_... bearer token, stores in-memory only | GET /health returns 401 when auth active (confirmed via auth_middleware in server.rs); HealthCard fetch pattern reusable |
| AUTH-02 | All /ui/ responses include Content-Security-Policy header | dashboard.rs router uses axum_embed::ServeEmbed; needs axum layer or middleware to inject header |
</phase_requirements>

---

## Summary

Phase 31 is a full-dashboard buildout on top of Phase 30's foundation. The frontend stack (Preact + TypeScript + Vite + Tailwind v4 as single-file output) is fully established; this phase creates 11 new components and rewrites App.tsx into the tab bar shell. The design system, interaction contracts, and copywriting are fully specified in 31-UI-SPEC.md — implementers should treat that document as ground truth, not derive values from scratch.

The backend has one new capability required: `GET /stats` returning per-agent memory counts and last-active timestamps. This requires adding a `stats()` method to the `StorageBackend` trait, implementing it for all three backends (SQLite, Qdrant, Postgres), adding a new `StatsResponse` struct to `service.rs`, wiring it through `MemoryService`, and adding the route to `server.rs`. The SQLite and Postgres implementations are straightforward GROUP BY queries. The Qdrant implementation has no native GROUP BY — it requires scroll + client-side aggregation (the same pattern already used in the Qdrant list implementation).

AUTH-02 (CSP header) requires adding an axum `Layer` to the dashboard router in `dashboard.rs`. The `axum::middleware::map_response` approach is the idiomatic way to inject a response header into all responses from a nested service.

**Primary recommendation:** Implement in two parallel tracks — (1) backend: stats endpoint + CSP header, and (2) frontend: all 11 components + App.tsx rewrite. The two tracks only intersect at integration.

---

## Standard Stack

### Core (already installed — no new dependencies required)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| preact | ^10.29.0 | UI rendering, hooks | Established in Phase 30 |
| typescript | ^5.9.3 | Type safety | Established in Phase 30 |
| vite | ^8.0.1 | Build tool, dev server | Established in Phase 30 |
| @preact/preset-vite | ^2.10.5 | Preact JSX transform for Vite | Established in Phase 30 |
| tailwindcss | ^4.2.2 | CSS utilities (used sparingly as utility classes) | Established in Phase 30 |
| vite-plugin-singlefile | ^2.3.2 | Single index.html output for rust-embed | Established in Phase 30 |

### No New Frontend Dependencies

The UI-SPEC confirms no third-party component libraries, no icon libraries, no routing libraries. All UI is inline styles + CSS variables. The design system is self-contained.

### Vite Dev Proxy (needs expansion for new endpoints)

Current `vite.config.ts` proxies only `/health`. For local development of the full dashboard, the proxy block must cover the new endpoints:

```typescript
proxy: {
  '/health': 'http://localhost:8080',
  '/memories': 'http://localhost:8080',
  '/stats': 'http://localhost:8080',
},
```

This is a dev-only change — does not affect the production embedded build.

### Installation

No new packages to install. All dependencies already present in `dashboard/package.json`.

---

## Architecture Patterns

### Recommended Component File Structure

```
dashboard/src/
├── App.tsx                    # REWRITE: tab bar shell + hash router + auth gate
├── main.tsx                   # No change
├── index.css                  # No change
├── vite-env.d.ts              # No change (or extend with env types)
├── api.ts                     # NEW: typed fetch wrappers (optional but recommended)
└── components/
    ├── HealthCard.tsx         # No change (kept as reference; Header.tsx replaces its function in the shell)
    ├── Header.tsx             # NEW: persistent top bar with health dot + backend name
    ├── TabBar.tsx             # NEW: three-tab navigation
    ├── LoginScreen.tsx        # NEW: full-screen auth gate
    ├── MemoriesTab.tsx        # NEW: paginated memory table orchestrator
    ├── MemoryRow.tsx          # NEW: single row (collapsed/expanded)
    ├── AgentsTab.tsx          # NEW: per-agent breakdown table
    ├── SearchTab.tsx          # NEW: search input + results
    ├── Pagination.tsx         # NEW: prev/next + "Showing X–Y of Z"
    ├── FilterBar.tsx          # NEW: agent_id/session_id/tag dropdowns
    ├── SkeletonRows.tsx       # NEW: 3-row gray bar skeleton
    ├── DistanceBar.tsx        # NEW: filled bar + numeric score
    └── ErrorMessage.tsx       # NEW: inline error with retry hint
```

### Pattern 1: App.tsx as Auth Gate + Tab Router

App.tsx is the only stateful coordinator. It holds: `token: string | null` (auth state) and `activeTab: 'memories' | 'agents' | 'search'` (routing state). On mount, it probes GET /health — 200 means open mode (token stays null, dashboard renders), 401 means auth mode (show LoginScreen).

```typescript
// Established pattern from HealthCard.tsx — extend this
type AppState =
  | { kind: 'checking' }          // initial mount, health probe in flight
  | { kind: 'login'; error?: string }   // 401 from health probe
  | { kind: 'dashboard'; token: string | null }  // 200 from health probe

export default function App() {
  const [appState, setAppState] = useState<AppState>({ kind: 'checking' })
  const [activeTab, setActiveTab] = useState<Tab>(parseHash())

  useEffect(() => {
    fetch('/health', { signal: AbortSignal.timeout(10_000) })
      .then(res => {
        if (res.status === 401) { setAppState({ kind: 'login' }); return }
        if (!res.ok) throw new Error(`HTTP ${res.status}`)
        return res.json()
      })
      .then(data => { if (data) setAppState({ kind: 'dashboard', token: null }) })
      .catch(() => setAppState({ kind: 'login', error: 'unreachable' }))
  }, [])

  // ... hash router + render switch
}
```

### Pattern 2: Manual Hash Router (~20 lines)

Exactly as specified in D-04. No library needed.

```typescript
type Tab = 'memories' | 'agents' | 'search'

function parseHash(): Tab {
  const hash = window.location.hash
  if (hash === '#/agents') return 'agents'
  if (hash === '#/search') return 'search'
  return 'memories' // default
}

// In App component:
useEffect(() => {
  const handler = () => setActiveTab(parseHash())
  window.addEventListener('hashchange', handler)
  return () => window.removeEventListener('hashchange', handler)
}, [])
```

### Pattern 3: State Machine for Async Data (established)

Every component that fetches uses the three-state discriminated union from HealthCard.tsx:

```typescript
type FetchState<T> =
  | { kind: 'loading' }
  | { kind: 'loaded'; data: T }
  | { kind: 'error'; message: string }
```

### Pattern 4: Token Threading

Token flows downward as a prop. No context needed for a three-tab dashboard. Each fetch call adds the header conditionally:

```typescript
function apiFetch(url: string, token: string | null, init?: RequestInit) {
  const headers: Record<string, string> = { ...(init?.headers as Record<string, string>) }
  if (token) headers['Authorization'] = `Bearer ${token}`
  return fetch(url, { ...init, headers, signal: AbortSignal.timeout(10_000) })
}
```

### Pattern 5: GET /stats Backend Implementation

New method on `StorageBackend` trait:

```rust
/// Returns per-agent memory count and last-active timestamp.
async fn stats(&self) -> Result<Vec<AgentStats>, ApiError>;
```

Where `AgentStats` (in `service.rs`) is:

```rust
#[derive(Debug, serde::Serialize)]
pub struct AgentStats {
    pub agent_id: String,
    pub memory_count: u64,
    pub last_active: String,  // ISO 8601 UTC, max created_at for that agent
}

#[derive(Debug, serde::Serialize)]
pub struct StatsResponse {
    pub agents: Vec<AgentStats>,
}
```

**SQLite implementation:**
```sql
SELECT agent_id,
       COUNT(*) AS memory_count,
       MAX(created_at) AS last_active
FROM memories
GROUP BY agent_id
ORDER BY last_active DESC
```

**Postgres implementation:** Same GROUP BY query via sqlx.

**Qdrant implementation:** No native GROUP BY. Use the existing scroll approach — scroll all points (with_payload=true, with_vectors=false), then aggregate client-side using a `HashMap<String, (u64, String)>`. This is acceptable because (a) typical deployments have small agent counts, and (b) the same pattern is already used in QdrantBackend::list(). Scroll with no filter fetches all points; aggregate by agent_id.

### Pattern 6: CSP Header Injection (AUTH-02)

`dashboard.rs` must wrap the `ServeEmbed` service with an axum layer that injects the CSP header on all responses. Use `axum::middleware::map_response`:

```rust
use axum::middleware;
use axum::response::Response;

async fn add_csp(response: Response) -> Response {
    let (mut parts, body) = response.into_parts();
    parts.headers.insert(
        axum::http::header::CONTENT_SECURITY_POLICY,
        axum::http::HeaderValue::from_static(
            "default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'"
        ),
    );
    Response::from_parts(parts, body)
}

pub fn router() -> Router {
    let serve = ServeEmbed::<DashboardAssets>::with_parameters(...);
    Router::new()
        .nest_service("/ui", serve)
        .layer(middleware::from_fn(add_csp))
}
```

**CSP policy rationale:** `vite-plugin-singlefile` inlines all JS and CSS into `index.html` — there are no external scripts or stylesheets. The policy `default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'` reflects this reality. `'unsafe-inline'` is required for the inlined script and style tags produced by singlefile. No external connections are made from the dashboard (all API calls go to `self`).

### Anti-Patterns to Avoid

- **Using localStorage for the auth token:** D-15 explicitly forbids this. Token lives in Preact component state only.
- **Populating filter dropdowns from a separate API call:** Filter options (agent_id, session_id, tag) are derived from the current page's list response — extract unique values client-side. No extra endpoint needed.
- **On-type search:** Search triggers on Enter keypress or "Search Memories" button only (D-09 / UI-SPEC). Avoids hammering the embedding endpoint on every keystroke.
- **Routing library for three tabs:** D-04 mandates manual hash routing. Adding React Router or similar is out of scope.
- **distance bar using actual CSS width percentage from state**: Distance bar width = `${score * 100}%` where score is the raw distance value (0.0–1.0, lower=better). But the UI shows similarity — a distance of 0.0 means identical (100% bar), distance of 1.0 means least similar (0% bar). The bar fill should be `${(1 - distance) * 100}%`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Relative timestamps ("2m ago") | Custom date math | Simple inline calculation (no library needed) | Only needs seconds/minutes/hours/days; `Date.now() - new Date(ts).getTime()` is sufficient for developer tool |
| Unique filter values | Separate API endpoint | Extract from current list response client-side | ListResponse already has all data needed; avoids N+1 problem |
| Routing | React Router / wouter | 20-line hash router (D-04) | Three tabs, no deep linking, hash routing avoids 404s |
| CSP header | Custom middleware framework | axum `middleware::from_fn` | Standard axum pattern, minimal code |
| Stats aggregation for Qdrant | External aggregation service | Client-side HashMap accumulation after scroll | Matches existing Qdrant list() pagination pattern; acceptable for developer tool scale |

**Key insight:** This is a developer operational tool, not a consumer product. Simplicity and correctness beat polish. The HealthCard pattern already demonstrates the entire component pattern — extend it, don't redesign it.

---

## Common Pitfalls

### Pitfall 1: Distance Bar Direction
**What goes wrong:** Rendering bar fill as `distance * 100%` instead of `(1 - distance) * 100%`. Distance 0.0 = identical (should show FULL bar). Distance 1.0 = dissimilar (should show EMPTY bar).
**Why it happens:** "Distance score" sounds like "higher = more", but the API uses lower-is-better semantics (per StorageBackend trait contract D-02).
**How to avoid:** Comment the formula: `// distance 0.0 = identical → 100% fill; distance 1.0 = dissimilar → 0% fill`
**Warning signs:** Top search results showing near-empty bars.

### Pitfall 2: Filter Dropdown Reset on Page Change
**What goes wrong:** Changing a filter dropdown does not reset the offset to 0, so the user sees page 3 of a filtered result set that has fewer than 3 pages.
**Why it happens:** Pagination offset and filter state are updated independently.
**How to avoid:** Any filter change must set offset back to 0. `handleFilterChange` always calls `setOffset(0)` before updating the filter state.
**Warning signs:** "Showing 41–60 of 12" (impossible pagination).

### Pitfall 3: Auth Token in useEffect Dependency Array
**What goes wrong:** Fetch calls inside useEffect don't re-run when the token changes (e.g., after login) because the token is not in the dependency array.
**Why it happens:** Token is a prop or state value; effects that use it without listing it as a dep will use the stale closure value.
**How to avoid:** Include `token` in every `useEffect` that calls `apiFetch(..., token, ...)`. Alternatively, pass token as a prop from App and only mount tab components after auth is resolved.
**Warning signs:** Tab shows empty data after login even though fetch succeeds.

### Pitfall 4: Vite Dev Proxy Mismatch
**What goes wrong:** New endpoints (/memories, /stats) are not proxied in `vite.config.ts`, so dev server returns 404 on the Vite port.
**Why it happens:** Phase 30 only proxied `/health`.
**How to avoid:** Expand the proxy config before starting development.
**Warning signs:** Network tab shows 404 for /memories on port 5173.

### Pitfall 5: Qdrant Stats — Empty agent_id
**What goes wrong:** Memories stored without an explicit agent_id use `agent_id = ""` (empty string, see `create_memory` in service.rs: `let agent_id = req.agent_id.unwrap_or_default()`). The stats table will show a row with `agent_id: ""`.
**Why it happens:** Empty string is the stored value when agent_id is omitted.
**How to avoid:** This is correct behavior — the frontend should display the empty string as `(none)` or `—` rather than a blank cell.
**Warning signs:** Agents tab shows a blank row that confuses users.

### Pitfall 6: CSP Blocking Inline Scripts
**What goes wrong:** A CSP policy without `'unsafe-inline'` for script-src blocks the inlined JavaScript produced by vite-plugin-singlefile, causing a blank dashboard.
**Why it happens:** Strict CSP (`script-src 'self'`) blocks inline scripts by default.
**How to avoid:** Use `script-src 'unsafe-inline'` in the CSP — this is safe for a self-hosted developer tool that has no user-generated content.
**Warning signs:** Browser console shows "Refused to execute inline script because it violates the following Content Security Policy directive".

### Pitfall 7: StorageBackend Trait Adding stats() Breaks Compilation
**What goes wrong:** Adding `stats()` to the `StorageBackend` trait without implementing it in all three backends (`SqliteBackend`, `QdrantBackend`, `PostgresBackend`) causes a compile error.
**Why it happens:** `StorageBackend` is a fully required trait — no default implementations.
**How to avoid:** Implement `stats()` in all three backends before or in the same commit as the trait change. All three implementations must compile with their feature flags.
**Warning signs:** `cargo build --features backend-qdrant,backend-postgres` fails.

---

## Code Examples

Verified patterns from existing codebase:

### Fetch with timeout (established — HealthCard.tsx)
```typescript
// Source: dashboard/src/components/HealthCard.tsx
fetch('/health', { signal: AbortSignal.timeout(10_000) })
  .then(res => {
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    return res.json()
  })
```

### Skeleton loading bars (established — HealthCard.tsx)
```typescript
// Source: dashboard/src/components/HealthCard.tsx
{[1, 2, 3].map((i) => (
  <div key={i} style={{
    height: '12px',
    background: 'var(--color-border)',
    opacity: 0.4,
    borderRadius: '2px',
  }} />
))}
```

### SQLite GROUP BY stats query
```sql
-- Source: derived from existing sqlite.rs list() pattern
SELECT agent_id,
       COUNT(*) AS memory_count,
       MAX(created_at) AS last_active
FROM memories
GROUP BY agent_id
ORDER BY last_active DESC
```

### axum CSP middleware
```rust
// Source: derived from axum middleware pattern
async fn add_csp(response: Response) -> Response {
    let (mut parts, body) = response.into_parts();
    parts.headers.insert(
        axum::http::header::CONTENT_SECURITY_POLICY,
        axum::http::HeaderValue::from_static(
            "default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'"
        ),
    );
    Response::from_parts(parts, body)
}
```

### Relative timestamp (no library)
```typescript
// Sufficient for developer tool — no library needed
function relativeTime(iso: string): string {
  const diff = Math.floor((Date.now() - new Date(iso).getTime()) / 1000)
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| React (standard) | Preact (lightweight) | Phase 30 | `preact/hooks` not `react`; `ComponentChildren` not `ReactNode` |
| Tailwind v3 (utility-first everywhere) | Tailwind v4 with @theme CSS variables | Phase 30 | CSS variables exposed via `--color-*`; most components use inline styles not utility classes |
| Multi-file SPA with asset manifest | vite-plugin-singlefile → single index.html | Phase 30 | All JS/CSS inlined; no external asset references; CSP must allow unsafe-inline |
| History-based routing | Hash routing (#/path) | Phase 30 | Avoids SPA 404s; no server config needed |

**Deprecated/outdated:**
- `window.localStorage` for token: explicitly forbidden by D-15. Token is component state only.
- `React.FC` type annotation: Preact uses `FunctionComponent` from `preact` or just infers types from return value.
- `import React from 'react'`: Preact uses `import { h } from 'preact'` (but preset-vite handles this automatically via JSX pragma — no manual import needed).

---

## Open Questions

1. **CSP nonce vs unsafe-inline**
   - What we know: vite-plugin-singlefile inlines all scripts/styles; `unsafe-inline` is required
   - What's unclear: Whether a nonce-based CSP would be feasible (would require the Rust server to generate a nonce per request and inject it into the HTML — impossible with static compile-time embedding)
   - Recommendation: Use `unsafe-inline`. The self-hosted nature of mnemonic means there is no untrusted user content that could execute injected scripts. Document the rationale in code comment.

2. **Qdrant stats scroll limit**
   - What we know: QdrantBackend::list() fetches up to `offset + limit + 1` points; stats needs ALL points
   - What's unclear: Qdrant scroll has a default page size (typically 10); full-collection scan requires cursor-based pagination across multiple scroll calls
   - Recommendation: Use `limit(10_000)` (Qdrant scroll max) for the stats query, or implement cursor-based loop. For typical mnemonic deployments (developer tool, <100K memories), a single large-limit scroll is acceptable. Document the scale assumption.

3. **GET /stats auth scope**
   - What we know: All `/memories*` routes are behind auth_middleware; `/health` is public
   - What's unclear: Should `/stats` be behind auth_middleware (it reads memory metadata) or public (it's aggregate, not content)?
   - Recommendation: Place `/stats` behind auth_middleware (same as `/memories`) — it exposes agent_id enumeration which is auth-sensitive. Add to the protected router in `build_router()`.

4. **Header 30-second polling and token threading**
   - What we know: UI-SPEC specifies Header refreshes every 30s; token must be passed to all fetch calls
   - What's unclear: Header.tsx needs the auth token to pass to /health re-polls (in auth mode, the 30s probe must include Authorization header)
   - Recommendation: Pass `token: string | null` as a prop to Header.tsx; include it in the fetch call.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js | Dashboard build | Check .node-version | See dashboard/.node-version | — |
| Cargo / Rust | Backend changes | Assumed present (existing project) | Existing | — |
| dashboard/dist/ | cargo build --features dashboard | Rebuilt each time | N/A | Run `npm run build` first |

Step 2.6: No new external dependencies. All tools are already present and verified by Phase 30.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test runner (`cargo test`) |
| Config file | Cargo.toml feature flags |
| Quick run command | `cargo test --features dashboard` |
| Full suite command | `cargo test --features dashboard,backend-qdrant,backend-postgres` (requires Qdrant/PG available) or `cargo test --features dashboard` for default SQLite-only |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BROWSE-01 | GET /memories returns paginated list | integration | `cargo test --features dashboard -- integration::` | ✅ tests/integration.rs |
| BROWSE-02 | GET /memories with filter params returns filtered results | integration | `cargo test --features dashboard -- integration::` | ✅ tests/integration.rs |
| BROWSE-03 | GET /memories/search returns ranked results with distance | integration | `cargo test --features dashboard -- integration::` | ✅ tests/integration.rs |
| BROWSE-04 | Memory struct includes all expandable fields | unit (type) | Compile-time verification via Memory struct | ✅ src/service.rs |
| BROWSE-05 | GET /stats returns per-agent counts and last_active | integration | `cargo test --features dashboard -- stats_` | ❌ Wave 0 gap |
| OPS-01 | GET /health returns backend name | integration | `cargo test --features dashboard -- health_endpoint` | ✅ tests/dashboard_integration.rs |
| AUTH-01 | GET /health returns 401 when auth active; dashboard detects it | integration | `cargo test --features dashboard -- auth_` | ✅ tests/integration.rs (auth middleware tests) |
| AUTH-02 | GET /ui/ response includes Content-Security-Policy header | integration | `cargo test --features dashboard -- csp_` | ❌ Wave 0 gap |

### Sampling Rate
- **Per task commit:** `cargo test --features dashboard`
- **Per wave merge:** `cargo test --features dashboard`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/dashboard_integration.rs` — add `stats_endpoint_returns_agent_breakdown` test covering BROWSE-05
- [ ] `tests/dashboard_integration.rs` — add `dashboard_ui_includes_csp_header` test covering AUTH-02

*(Existing test infrastructure covers all other requirements. Two new test functions in the existing file, no new file needed.)*

---

## Project Constraints (from CLAUDE.md)

No project-level CLAUDE.md found at `/Users/chrisesposito/Documents/github/mnemonic/CLAUDE.md`. Global CLAUDE.md applies:

- Keep README.md, AGENTS.md, and ROADMAP.md up to date (update if phase changes affect documented behavior)
- User prefers agent teams when it makes sense — this phase has clearly separable backend (Rust) and frontend (Preact) tracks that can be parallelized

---

## Sources

### Primary (HIGH confidence)
- Direct code inspection: `src/server.rs`, `src/service.rs`, `src/storage/mod.rs`, `src/storage/sqlite.rs`, `src/storage/qdrant.rs`, `src/storage/postgres.rs`, `src/dashboard.rs` — full understanding of existing API surface and extension points
- Direct code inspection: `dashboard/src/App.tsx`, `dashboard/src/components/HealthCard.tsx`, `dashboard/src/index.css`, `dashboard/vite.config.ts`, `dashboard/package.json` — complete frontend stack inventory
- Direct read: `.planning/phases/31-core-ui/31-CONTEXT.md` — all 15 locked decisions
- Direct read: `.planning/phases/31-core-ui/31-UI-SPEC.md` — complete design system, component inventory, interaction contracts, copywriting contract
- Direct read: `.planning/REQUIREMENTS.md` — BROWSE-01 through AUTH-02
- Direct read: `tests/dashboard_integration.rs` — existing test patterns to extend

### Secondary (MEDIUM confidence)
- axum `middleware::from_fn` pattern for response header injection — verified against axum 0.8 API used in project
- Qdrant scroll cursor pagination with large limit — derived from existing QdrantBackend::list() implementation pattern

### Tertiary (LOW confidence)
- CSP `unsafe-inline` sufficiency for vite-plugin-singlefile output — requires manual browser verification; flag for testing in Wave 0 gap

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — entire stack already in place; no new dependencies
- Architecture: HIGH — patterns directly derived from existing HealthCard and storage backend code
- Backend stats implementation: HIGH for SQLite/Postgres (standard GROUP BY), MEDIUM for Qdrant (scroll-based aggregation is correct but large-collection scale not tested)
- CSP header: MEDIUM — approach is correct; exact policy string requires browser smoke test
- Pitfalls: HIGH — derived from direct code inspection, not speculation

**Research date:** 2026-03-22
**Valid until:** 2026-05-22 (stable stack — preact, vite, axum are stable; no fast-moving dependencies)
