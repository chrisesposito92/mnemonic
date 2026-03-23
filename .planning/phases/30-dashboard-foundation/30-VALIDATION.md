---
phase: 30
slug: dashboard-foundation
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-22
audited: 2026-03-23
---

# Phase 30 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (`cargo test`) |
| **Config file** | none — Rust inline `#[cfg(test)]` |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Dashboard suite** | `cargo test --features dashboard --test dashboard_integration` |
| **Estimated runtime** | ~10s (dashboard), ~0.05s (quick), ~8s (full) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Test File | Status |
|---------|------|------|-------------|-----------|-------------------|-----------|--------|
| 30-01-01 | 01 | 1 | BUILD-01 | smoke/integration | `cargo build --features dashboard && cargo test --features dashboard --test dashboard_integration` | `tests/dashboard_integration.rs` | green |
| 30-01-02 | 01 | 1 | BUILD-01 | integration | `cargo test --features dashboard --test dashboard_integration -- dashboard_ui_slash_returns_200_html` | `tests/dashboard_integration.rs` | green |
| 30-02-01 | 02 | 1 | BUILD-02 | regression | `cargo test` (87 lib + 58 integration = 145 tests) | `tests/integration.rs`, `src/**/*.rs` | green |
| 30-02-02 | 02 | 1 | BUILD-02 | compile-check | `cargo build` (default features, zero dashboard deps) | n/a — compile gate | green |
| 30-03-01 | 03 | 2 | BUILD-03 | CI gate | `.github/workflows/release.yml` regression job | `.github/workflows/release.yml` | green |

*Status: pending · green · red · flaky*

---

## Wave 0 Requirements

- [x] `tests/dashboard_integration.rs` — 9 integration tests for BUILD-01: `GET /ui/` returns 200 with `text/html`, mnemonic-root mount point, trailing-slash contract, SPA fallback, asset request safety, CSP header
- [x] `src/dashboard.rs` — module exists with `RustEmbed` + `axum-embed` serving at `/ui`

*Existing infrastructure covers BUILD-02 (145 passing tests) and BUILD-03 (CI regression job).*

---

## Test Coverage Detail

| Test Function | Requirement | What It Proves |
|--------------|-------------|----------------|
| `dashboard_ui_slash_returns_200_html` | BUILD-01 | GET /ui/ through `build_router()` returns 200 text/html |
| `dashboard_ui_contains_mnemonic_root` | BUILD-01 | Response body contains stable `mnemonic-root` mount point |
| `dashboard_ui_no_trailing_slash_returns_200_or_redirect` | BUILD-01 | GET /ui (no slash) returns 200 or redirect to /ui/ |
| `dashboard_spa_fallback_returns_index_html` | BUILD-01 | GET /ui/nonexistent returns 200 text/html (SPA fallback) |
| `dashboard_asset_request_returns_valid_response` | BUILD-01 | Asset requests return 200 or 404, never 500 |
| `dashboard_ui_includes_csp_header` | BUILD-01 | CSP header present on /ui/ and SPA fallback paths |
| `health_endpoint_still_works_with_dashboard` | BUILD-02 | GET /health returns 200 with dashboard feature enabled |
| `health_endpoint_includes_auth_enabled_field` | BUILD-02 | Health response includes auth_enabled boolean |
| `stats_endpoint_returns_agent_breakdown` | BUILD-01 | GET /stats returns per-agent breakdown (bonus) |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| vite-plugin-singlefile CSS inlining | BUILD-01 | Empirical verification of plugin combination | Run `npm run build` in `dashboard/`, inspect `dist/index.html` for no external `<link>` or `<script src>` tags |

*Note: `nest_service("/ui")` trailing-slash behavior was previously manual-only but is now covered by `dashboard_ui_no_trailing_slash_returns_200_or_redirect`.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** passed

---

## Validation Audit 2026-03-23

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |
| Tests found | 9 (dashboard integration) + 87 (lib) + 58 (default integration) |
| Requirements covered | BUILD-01, BUILD-02, BUILD-03 |
| Manual-only items | 1 (vite-plugin-singlefile inlining) |

All three requirements have automated verification through `tests/dashboard_integration.rs` (9 tests, all green) and the existing test suite (145 tests, all green). CI release workflow verified via grep to contain Node.js build, dual artifacts, and regression gate.
