---
phase: 30
slug: dashboard-foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-22
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
| **Estimated runtime** | ~30 seconds (full), ~0.05s (quick) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 30-01-01 | 01 | 1 | BUILD-01 | smoke/integration | `cargo build --features dashboard && cargo test --features dashboard -- dashboard` | No — Wave 0 | pending |
| 30-01-02 | 01 | 1 | BUILD-01 | integration | `cargo test --features dashboard -- ui_serves_html` | No — Wave 0 | pending |
| 30-02-01 | 02 | 1 | BUILD-02 | regression | `cargo test` (292 existing tests) | Yes | pending |
| 30-02-02 | 02 | 1 | BUILD-02 | compile-check | `cargo build 2>&1 \| grep -v dashboard` | n/a | pending |
| 30-03-01 | 03 | 2 | BUILD-03 | CI gate | verified in CI only | CI-only | pending |

*Status: pending · green · red · flaky*

---

## Wave 0 Requirements

- [ ] `tests/dashboard_integration.rs` — stubs for BUILD-01: `GET /ui/` returns 200 with `text/html` when built with `--features dashboard`
- [ ] `src/dashboard.rs` — module must exist before integration test can compile

*Existing infrastructure covers BUILD-02 (292 passing tests) and BUILD-03 (CI regression job).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| vite-plugin-singlefile CSS inlining | BUILD-01 | Empirical verification of plugin combination | Run `npm run build` in `dashboard/`, inspect `dist/index.html` for no external `<link>` or `<script src>` tags |
| `nest_service("/ui")` trailing-slash behavior | BUILD-01 | Edge case not covered by unit tests | Test `GET /ui`, `GET /ui/`, `GET /ui/nonexistent` — all should return 200 with HTML |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
