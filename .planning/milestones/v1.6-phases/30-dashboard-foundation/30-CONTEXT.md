# Phase 30: Dashboard Foundation - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Build pipeline, rust-embed integration, feature gate, and CI wiring — so the `dashboard` Cargo feature compiles, the binary serves the embedded SPA at `/ui`, and CI verifies both the dashboard build and the default binary regression gate. No actual dashboard functionality — that's Phase 31.

</domain>

<decisions>
## Implementation Decisions

### App shell content
- **D-01:** Minimal proof-of-life page — "Mnemonic Dashboard" heading + version + health status from GET /health. Phase 31 replaces it entirely.
- **D-02:** Basic Tailwind styling (dark background, centered card, monospace font) to confirm the full Preact + Tailwind + embed pipeline works.
- **D-03:** Live fetch to GET /health on mount, displaying backend name + status. Validates the full SPA → API round-trip.

### Frontend tooling
- **D-04:** Tailwind v4 with `@tailwindcss/vite` plugin. CSS-first config, no PostCSS or tailwind.config.js needed.
- **D-05:** npm as package manager. Aligns with CI success criteria (`npm ci && npm run build`).
- **D-06:** TypeScript for the Preact frontend. Type safety for API response shapes across all 3 phases.
- **D-07:** Separate Vite dev server for frontend development. `npm run dev` on :5173 with HMR, API proxy to :8080. Build-then-cargo for production.

### Single-file strategy
- **D-08:** Try vite-plugin-singlefile first to produce a single index.html with inlined JS/CSS.
- **D-09:** If vite-plugin-singlefile fails with Preact + Tailwind v4, fall back to multi-file output with axum-embed serving the directory (rust-embed handles the folder, axum-embed serves with correct MIME types).

### CI job structure
- **D-10:** Node.js setup + `npm ci && npm run build` runs as a step within each matrix job (not a separate prerequisite job). Self-contained per platform.
- **D-11:** Separate `regression` CI job runs `cargo build` (default features, no dashboard) + `cargo test` in parallel with dashboard builds. Failure blocks release.
- **D-12:** Release produces both variants per platform — `mnemonic` (slim, no dashboard) and `mnemonic-dashboard` (with embedded UI). Users choose which to download.

### Carried forward from milestone research
- **D-13:** rust-embed 8.11 + axum-embed 0.1 for compile-time asset embedding, both optional deps behind `dashboard` feature.
- **D-14:** Hash routing (`#/path`) over history routing — avoids SPA hard-reload 404s at zero cost.
- **D-15:** Dashboard router merged at top level in build_router() (not inside protected router) to prevent auth middleware blocking asset loads.

### Claude's Discretion
- Exact dashboard/ directory structure and file layout
- Vite configuration details
- Preact project scaffolding approach
- Exact Cargo.toml dependency versions for rust-embed/axum-embed
- Compile-time error implementation for missing dashboard/dist/index.html

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Build infrastructure
- `.planning/REQUIREMENTS.md` — BUILD-01 (rust-embed + axum-embed + SPA fallback), BUILD-02 (feature gate), BUILD-03 (CI wiring)
- `.planning/ROADMAP.md` §Phase 30 — Success criteria (5 items: cargo build, default regression, CI flow, compile-time error)

### Existing server
- `src/server.rs` — `build_router()` function where dashboard routes will merge; protected vs public route pattern
- `src/main.rs` — Feature-gate pattern (`#[cfg(feature = "interface-grpc")]`), server startup flow
- `Cargo.toml` — Existing feature flags pattern (`backend-qdrant`, `backend-postgres`, `interface-grpc`)

### CI
- `.github/workflows/release.yml` — Current release workflow to extend with Node.js build step and regression job

### Prior decisions
- `.planning/STATE.md` §Accumulated Context > Decisions — rust-embed/axum-embed choice, vite-plugin-singlefile risk, hash routing, dashboard router placement

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/server.rs:build_router()` — Router merge pattern (protected + public); dashboard adds a third merge behind `#[cfg(feature = "dashboard")]`
- `Cargo.toml` features section — Established pattern for optional feature flags with `dep:` syntax

### Established Patterns
- Feature gating via `#[cfg(feature = "...")]` — used for `interface-grpc`, `backend-qdrant`, `backend-postgres`
- `src/main.rs` conditional compilation blocks for gRPC — exact pattern to follow for dashboard module
- axum Router composition via `.merge()` with separate protected/public sections

### Integration Points
- `src/server.rs:build_router()` — New dashboard routes merge here, outside the protected router
- `Cargo.toml` — New `dashboard` feature with `dep:rust-embed`, `dep:axum-embed`
- `.github/workflows/release.yml` — Node.js setup step + regression job added

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches for the build pipeline scaffolding.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 30-dashboard-foundation*
*Context gathered: 2026-03-22*
