---
phase: 31
reviewers: [gemini, codex]
reviewed_at: 2026-03-22T20:00:00Z
plans_reviewed: [31-01-PLAN.md, 31-02-PLAN.md, 31-03-PLAN.md, 31-04-PLAN.md]
---

# Cross-AI Plan Review — Phase 31

## Gemini Review

### 1. Summary
The implementation plan for Phase 31 is well-structured, logically sequenced, and highly aligned with the "Redis of agents" philosophy—prioritizing a lightweight, zero-dependency operational view. By splitting the work into a backend foundation wave (Plan 01) followed by parallelizable frontend modules (Plans 02-04), the strategy ensures that the UI components have stable APIs to consume immediately. The use of a manual hash router and in-memory auth tokens respects the project's "dense and minimal" constraints while maintaining high security standards through CSP and restricted token storage.

### 2. Strengths
- **Logical Wave Separation**: Moving the `StorageBackend::stats()` implementation to Wave 1 prevents frontend blockers and allows integration tests to run before the UI is even built.
- **Security-First Auth**: The decision to keep the `mnk_...` token in component state only (never `localStorage`) and implementing a backend CSP middleware demonstrates a high level of security maturity for an embedded dashboard.
- **Pragmatic Routing**: Using a 20-line manual hash router (D-04) instead of a heavy library like `react-router` is a perfect choice for an embedded single-page tool where bundle size and simplicity are key.
- **Defensive UI Patterns**: The inclusion of `SkeletonRows` and `ErrorMessage` components in the early shell phase ensures a consistent "loading/failure" UX across all tabs from the start.

### 3. Concerns
- **Filter Population Logic (Plan 31-03)**
    - **Severity: HIGH**
    - **Issue**: The plan states "Filter options derived from current response." In a paginated view (e.g., 20 memories per page), the filter dropdowns will only show `agent_id` or `session_id` values present on the *current page*.
    - **Risk**: If a user wants to filter by an agent that appears on page 5, but they are on page 1, that agent will not appear in the dropdown, making the filtering feature nearly useless for discovery.
- **Qdrant Stats Performance (Plan 31-01)**
    - **Severity: MEDIUM**
    - **Issue**: Qdrant lacks a native `GROUP BY` or `COUNT DISTINCT`. The plan proposes "scroll + HashMap aggregation."
    - **Risk**: For deployments with 100k+ memories, fetching every record just to count agents will cause significant latency and high memory usage on the Mnemonic binary, potentially OOM-ing small containers or slowing down the agent API.
- **CSP Header Strictness (Plan 31-01)**
    - **Severity: LOW**
    - **Issue**: Vite-plugin-singlefile often inlines CSS and occasionally small scripts.
    - **Risk**: A standard "strict" CSP might block the embedded CSS/JS if `unsafe-inline` isn't handled or if hashes/nonces aren't synchronized between the build and the Axum middleware.
- **Auth Persistence UX (Plan 31-02)**
    - **Severity: LOW**
    - **Issue**: D-15 prohibits `localStorage`.
    - **Risk**: Users will be logged out on every page refresh. While secure, this can be frustrating during active debugging. (Note: This is a locked decision, so this is a "UX Warning" rather than a request to change the plan).

### 4. Suggestions
- **Improve Filter Discovery**: Instead of deriving filters from the *response*, consider adding a `GET /tags` or `GET /agents` (or repurposing the new `GET /stats`) to populate filter dropdowns with all known values, or change the filter UI to a simple text input that the user types into.
- **Optimize Qdrant Stats**: Since `GET /stats` is primarily for the dashboard, consider implementing a simple LRU cache or a background ticker in `service.rs` that updates agent counts every 60 seconds, rather than performing a full scroll on every dashboard pageload.
- **Add "Clear Filters" Button**: Plan 31-03 mentions resetting offset to 0 on change, but a dedicated "Clear All" button is essential for UX when multiple filters (agent, session, tag) are active.
- **Refine Distance Bar Logic**: Ensure the `DistanceBar` component handles cases where the storage backend might return distances > 1.0 (some vector DBs use squared Euclidean distance which isn't bound to [0,1]). Ensure it clamps to 0-100%.

### 5. Risk Assessment
**Risk Level: MEDIUM**

The overall plan is technically sound, but the **Filter Population Logic** and **Qdrant Stats Aggregation** represent functional and performance risks. If the filters only show what's currently on screen, the "Browse" requirement (BROWSE-02) is technically met but practically flawed. If these two points are addressed (e.g., by using the stats endpoint to feed filters and adding basic caching for Qdrant counts), the risk drops to **LOW**. The execution steps are otherwise surgical and well-tailored to the project's architecture.

---

## Codex Review

### Overall
The plan set is close, but it does not yet fully guarantee the phase goal. The biggest blockers are the auth contract mismatch around `GET /health`, undefined scoped-key behavior for `GET /stats`, and verification that is too thin for feature-gated backends and browser-state behavior. Plans 31-03 and 31-04 are directionally good, but they inherit those unresolved contracts.

### Plan 31-01
**Summary**
Good backend-first sequencing, but this plan is under-specified where security and cross-backend contracts matter most.

**Strengths**
- Backend foundation first is the right dependency shape for the UI work.
- Putting CSP at the `/ui/` boundary and `/stats` behind auth is directionally correct.

**Concerns**
- `HIGH`: `/stats` scope behavior is undefined for scoped API keys. "Protected" is not enough; scoped keys still authenticate, so the handler must explicitly constrain results or it can leak cross-agent counts.
- `HIGH`: Verification misses optional backend risk. `cargo build --features dashboard` does not prove Qdrant/Postgres still compile after a `StorageBackend` trait change in src/storage/mod.rs.
- `MEDIUM`: Qdrant "scroll + HashMap" can become expensive or incomplete on large datasets unless it explicitly paginates all results and/or caches.
- `MEDIUM`: CSP can easily break the single-file SPA unless it is tested against `/ui/`, fallback routes, and inline assets.

**Suggestions**
- Define `/stats` as scope-aware: wildcard key sees all agents, scoped key sees only its allowed agent.
- Normalize `last_active` at the API boundary so all backends return the same timestamp shape.
- Add a feature-matrix build/test pass for `dashboard + backend-qdrant` and `dashboard + backend-postgres`.
- Test CSP on `/ui/` and a SPA fallback path, not just the root.

**Risk Assessment**: `MEDIUM-HIGH` because the security contract and backend coverage are not finished.

### Plan 31-02
**Summary**
This is the blocker plan. The shell decomposition is sensible, but the auth flow is built on a backend contract that does not match the current product.

**Strengths**
- `api.ts` plus shared error/loading components is a good base for the rest of the UI.
- In-memory token storage matches the phase requirement and avoids obvious persistence mistakes.

**Concerns**
- `HIGH`: The plan assumes `GET /health` becomes a 401 auth probe, but the current contract keeps `/health` public in src/server.rs. As written, auth detection/login validation will be wrong.
- `HIGH`: There is no explicit re-auth path for revoked/invalid tokens. Protected tabs can fail while the header still looks healthy if it only polls public `/health`.
- `MEDIUM`: `tsc` and `vite build` do not verify the actual auth-gated runtime flow.

**Suggestions**
- Keep `/health` public and add either a protected bootstrap/probe endpoint or an `auth_enabled` flag on the health response.
- Make unauthorized responses a first-class app state that clears the token and returns to the login gate.
- Add one browser-level smoke test for open mode, auth mode, bad token, and good token.

**Risk Assessment**: `HIGH` because the current auth approach does not line up with the shipped server behavior.

### Plan 31-03
**Summary**
The browse table work is well-scoped, but the filter strategy and fetch lifecycle need more rigor.

**Strengths**
- Clear coverage of pagination, inline expansion, and the required dense memory table.
- Resetting offset on filter change is the right behavior.

**Concerns**
- `HIGH`: Deriving filter dropdown values from the current paginated response weakens BROWSE-02 in practice; users cannot filter to values that are not on the current page.
- `MEDIUM`: The plan does not mention aborting or ordering in-flight list requests, so fast filter/page changes can apply stale responses.
- `MEDIUM`: State persistence across tab switches is unspecified; filters and expansion state may reset unexpectedly.

**Suggestions**
- Source agent options from `/stats` and preserve selected values even if they are absent from the current page.
- Add request cancellation or request-id guards around list fetches.
- Decide explicitly whether browse state should survive tab changes.

**Risk Assessment**: `MEDIUM` because the UI can ship, but filtering will feel partial unless the data-source issue is fixed.

### Plan 31-04
**Summary**
Reasonable split for Search and Agents, but it depends on unresolved backend/shell contracts and has one important metric-meaning problem.

**Strengths**
- Search-on-submit only is a good restraint for both UX and backend load.
- Separating Agents from Memories keeps the responsibilities clean.

**Concerns**
- `HIGH`: Agents depends on `/stats`, so the scoped-key/security ambiguity from Plan 31-01 carries straight into this plan.
- `MEDIUM`: The distance bar assumes `(1 - distance) * 100%`, but current backends do not all return the same normalized distance metric; SQLite returns raw KNN distance and Postgres uses cosine distance. The bar can mislead unless normalized/clamped.
- `MEDIUM`: Error handling for empty results, 401/403, and repeated submissions is not explicit.

**Suggestions**
- Finalize the `/stats` auth/scope contract before implementing `AgentsTab`.
- Treat the bar as a clamped relative indicator, or normalize per backend before rendering.
- Handle no-results/auth-failure states separately from generic transport errors.

**Risk Assessment**: `MEDIUM-HIGH` because the UI shape is fine, but the underlying contracts are not stable enough yet.

### Bottom Line
If I had to reorder work, I would first fix the backend/app-shell contract: keep `/health` public, define a real auth probe, and make `/stats` scope-aware. After that, the rest of the phase looks implementable with moderate risk.

---

## Consensus Summary

### Agreed Strengths
- **Wave sequencing is correct** — backend foundation (Plan 01) before UI (Plans 02-04) prevents blockers (both reviewers)
- **Security-conscious auth design** — in-memory-only token storage and CSP middleware are well-positioned (both reviewers)
- **Lightweight, pragmatic architecture** — manual hash router, no new dependencies, reusable shared components (both reviewers)

### Agreed Concerns
1. **Filter dropdown population from current page only** — Both reviewers flagged this as HIGH severity. With paginated responses (20/page), filter dropdowns only show values present on the current page, making BROWSE-02 practically flawed for discovery. **Recommendation**: Source agent/session options from `GET /stats` or a dedicated endpoint rather than the current page's response.

2. **Qdrant stats performance at scale** — Both reviewers flagged the scroll + HashMap aggregation as a MEDIUM risk. Full-collection scans for every `/stats` request are expensive at 100k+ memories. **Recommendation**: Add caching or a large-limit guard, and document the scale assumption.

3. **`GET /health` auth contract mismatch** — Codex flagged as HIGH that the plan assumes `/health` returns 401 when auth is active, but the current server keeps `/health` public. This is a critical implementation detail that must be verified before Plan 02 execution. **Recommendation**: Verify actual `/health` behavior with auth middleware; if it stays public, add an `auth_enabled` field to the health response or use a different probe endpoint.

4. **Scoped API key behavior for `/stats`** — Codex flagged as HIGH that scoped keys (restricted to specific agent_ids) could leak cross-agent counts from `/stats`. **Recommendation**: Make the stats handler scope-aware so scoped keys only see their allowed agent's stats.

5. **Distance bar normalization** — Both reviewers noted the distance bar assumes 0.0-1.0 range but backends may return different scales. **Recommendation**: Clamp the bar fill to 0-100% regardless of input value.

### Divergent Views
- **Auth UX (token persistence)**: Gemini noted the "logged out on refresh" UX cost as a LOW concern but acknowledged it's a locked decision. Codex focused more on the missing re-auth path when tokens become invalid mid-session. Both are valid but Codex's concern is actionable — unauthorized responses from protected endpoints should clear the token and return to the login gate.
- **Risk severity**: Gemini rated overall risk as MEDIUM; Codex rated individual plans MEDIUM to HIGH. The divergence is mainly around the `/health` auth contract — if that concern is verified and resolved, both reviews converge toward MEDIUM risk.
- **Verification depth**: Codex specifically called out that `cargo build --features dashboard` doesn't cover `backend-qdrant` and `backend-postgres` feature flags. Gemini didn't flag this. **Recommendation**: Add cross-feature build verification.

---

*Reviewed: 2026-03-22*
*Reviewers: Gemini CLI, Codex CLI*
*Phase plans reviewed: 31-01 through 31-04*
