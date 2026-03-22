# Phase 30: Dashboard Foundation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-22
**Phase:** 30-dashboard-foundation
**Areas discussed:** App shell content, Frontend tooling, Single-file fallback, CI job structure

---

## App Shell Content

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal proof-of-life | "Mnemonic Dashboard" heading + version + health status from GET /health. Phase 31 replaces it entirely. | ✓ |
| Nav skeleton | Header + sidebar nav with placeholder links. Gives Phase 31 a layout to fill in. | |
| You decide | Claude picks whatever makes the most sense. | |

**User's choice:** Minimal proof-of-life
**Notes:** Just enough to prove embedding works — Phase 31 replaces the whole page.

| Option | Description | Selected |
|--------|-------------|----------|
| Basic Tailwind styling | Dark background, centered card, monospace font — confirms Tailwind is bundled and working. | ✓ |
| Raw unstyled HTML | Plain HTML, no CSS. Tailwind not verified until Phase 31. | |
| You decide | Claude picks. | |

**User's choice:** Basic Tailwind styling
**Notes:** Validates the full pipeline (Preact + Tailwind + vite-plugin-singlefile → rust-embed).

| Option | Description | Selected |
|--------|-------------|----------|
| Live health fetch | Fetch GET /health on mount, display backend name + status. Proves SPA → API round-trip. | ✓ |
| Static text only | Just "Dashboard loaded successfully." No API calls. | |
| You decide | Claude picks. | |

**User's choice:** Live health fetch
**Notes:** Proves the full round-trip: embedded SPA → API → response rendered.

---

## Frontend Tooling

| Option | Description | Selected |
|--------|-------------|----------|
| Tailwind v4 | CSS-first config, @tailwindcss/vite plugin. No PostCSS or tailwind.config.js. | ✓ |
| Tailwind v3 | Class config, PostCSS. Battle-tested but more boilerplate. | |
| You decide | Claude picks. | |

**User's choice:** Tailwind v4
**Notes:** Modern, lighter setup, Preact compatibility is solid.

| Option | Description | Selected |
|--------|-------------|----------|
| npm | CI already uses npm patterns. npm ci is fast and deterministic. | ✓ |
| pnpm | Faster installs, stricter deps. Requires CI change. | |
| You decide | Claude picks. | |

**User's choice:** npm
**Notes:** Matches "npm ci && npm run build" in success criteria.

| Option | Description | Selected |
|--------|-------------|----------|
| TypeScript | Type safety for API response shapes. Preact has excellent TS support. | ✓ |
| Plain JavaScript | Zero config overhead. Faster to scaffold. | |
| You decide | Claude picks. | |

**User's choice:** TypeScript
**Notes:** Pays off across 3 phases.

| Option | Description | Selected |
|--------|-------------|----------|
| Separate Vite dev server | npm run dev on :5173 with HMR, API proxy to :8080. Fast iteration. | ✓ |
| Build-then-cargo only | Always build frontend then cargo build. No HMR. | |
| You decide | Claude picks. | |

**User's choice:** Separate Vite dev server
**Notes:** Fast iteration for Phase 31-32 UI work.

---

## Single-File Fallback

| Option | Description | Selected |
|--------|-------------|----------|
| Multi-file with axum-embed | Drop vite-plugin-singlefile. Normal Vite dist/ output. axum-embed serves all files. | ✓ |
| Manual inline script | Custom plugin or post-build script to inline JS/CSS. Brittle. | |
| You decide | Claude picks. | |

**User's choice:** Multi-file with axum-embed
**Notes:** Safe path — rust-embed handles directories, axum-embed serves correct MIME types.

| Option | Description | Selected |
|--------|-------------|----------|
| Try single-file first | Attempt vite-plugin-singlefile. If it works, simpler embed. If not, switch to multi-file. | ✓ |
| Go straight to multi-file | Skip single-file experiment. Guaranteed to work. | |
| You decide | Claude picks. | |

**User's choice:** Try single-file first
**Notes:** Quick verification. Fall back to multi-file if it doesn't inline cleanly.

---

## CI Job Structure

| Option | Description | Selected |
|--------|-------------|----------|
| Step within each matrix job | Each platform job: setup Node → npm build → setup Rust → cargo build --features dashboard. | ✓ |
| Separate prerequisite job | One "build-frontend" job, matrix jobs download dist/. Avoids redundant npm builds. | |
| You decide | Claude picks. | |

**User's choice:** Step within each matrix job
**Notes:** Self-contained, simpler YAML.

| Option | Description | Selected |
|--------|-------------|----------|
| Separate CI job | New "regression" job: cargo build (default) + cargo test. Runs in parallel. Failure blocks release. | ✓ |
| Extra step in one matrix job | Add default-features build to linux-x86_64 job only. | |
| You decide | Claude picks. | |

**User's choice:** Separate CI job
**Notes:** Clean separation, runs in parallel with dashboard builds.

| Option | Description | Selected |
|--------|-------------|----------|
| Dashboard included by default | Release binaries always include dashboard. Users ignore /ui if unwanted. | |
| Ship both variants | Two binaries per platform: mnemonic (no dashboard) and mnemonic-dashboard. | ✓ |
| You decide | Claude picks. | |

**User's choice:** Ship both variants
**Notes:** Users choose which to download — slim or with embedded UI.

---

## Claude's Discretion

- Exact dashboard/ directory structure and file layout
- Vite configuration details
- Preact project scaffolding approach
- Exact Cargo.toml dependency versions for rust-embed/axum-embed
- Compile-time error implementation for missing dashboard/dist/index.html

## Deferred Ideas

None — discussion stayed within phase scope.
