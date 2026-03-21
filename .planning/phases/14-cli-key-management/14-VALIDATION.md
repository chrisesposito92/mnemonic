---
phase: 14
slug: cli-key-management
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 14 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 14-01-01 | 01 | 1 | CLI-01 | unit | `cargo test cli` | ❌ W0 | ⬜ pending |
| 14-01-02 | 01 | 1 | CLI-02 | unit | `cargo test cli` | ❌ W0 | ⬜ pending |
| 14-01-03 | 01 | 1 | CLI-03 | unit | `cargo test cli` | ❌ W0 | ⬜ pending |
| 14-01-04 | 01 | 1 | CLI-01 | integration | `cargo test --test cli_integration` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/cli_integration.rs` — integration tests for keys create/list/revoke
- [ ] Test fixtures for temporary SQLite databases

*Existing cargo test infrastructure covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Startup under 1 second | CLI-01 | Timing varies by machine | Run `time mnemonic keys list` and verify real < 1s |
| "copy now" warning visibility | CLI-01 | UX readability | Run `mnemonic keys create --name test --scope read` and verify warning is prominent |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
