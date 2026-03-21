---
phase: 14
slug: cli-key-management
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-20
validated: 2026-03-21
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
| **Integration tests** | `cargo test --test cli_integration` |
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
| 14-01-01 | 01 | 1 | CLI-01 | unit | `cargo test --lib cli::tests::test_cmd_create_creates_key` | tests/cli_integration.rs | ✅ green |
| 14-01-02 | 01 | 1 | CLI-02 | unit | `cargo test --lib cli::tests::test_cmd_list_with_keys_does_not_panic` | tests/cli_integration.rs | ✅ green |
| 14-01-03 | 01 | 1 | CLI-03 | unit | `cargo test --lib cli::tests::test_cmd_revoke_by_display_id` | tests/cli_integration.rs | ✅ green |
| 14-01-04 | 01 | 1 | CLI-01/02/03 | integration | `cargo test --test cli_integration` | tests/cli_integration.rs | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `tests/cli_integration.rs` — integration tests for keys create/list/revoke (10 tests, all green)
- [x] Test fixtures for temporary SQLite databases (TempDb struct with cleanup via Drop)

*Existing cargo test infrastructure covers framework needs.*

---

## Integration Test Coverage (tests/cli_integration.rs)

| Test | Requirement | Behavior Verified |
|------|-------------|-------------------|
| `test_keys_create_exits_zero_and_prints_token` | CLI-01 | Binary exits 0, first stdout line starts with `mnk_` and is 68 chars |
| `test_keys_create_prints_metadata` | CLI-01 | Stdout contains ID:, Name:, Scope: fields with key name |
| `test_keys_create_prints_save_warning_to_stderr` | CLI-01 | Stderr contains "Save this key" warning |
| `test_keys_create_scoped_shows_agent_id` | CLI-01 | `--agent-id` scopes the key; agent_id appears in stdout |
| `test_keys_list_empty_state_exits_zero` | CLI-02 | Empty list exits 0 and prints "No API keys found" |
| `test_keys_list_prints_table_with_headers` | CLI-02 | After create, list prints ID/NAME/SCOPE/CREATED/STATUS headers |
| `test_keys_list_shows_active_key_row` | CLI-02 | List shows display_id and "active" status for a live key |
| `test_keys_revoke_by_display_id_exits_zero` | CLI-03 | Revoke by 8-char display_id exits 0 and prints "revoked" |
| `test_keys_revoke_key_appears_revoked_in_list` | CLI-03 | After revoke, list shows "revoked" status for that key |
| `test_keys_revoke_nonexistent_display_id_exits_nonzero` | CLI-03 | Revoke of non-existent key exits non-zero and prints "No key found" |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Startup under 1 second | CLI-01 | Timing varies by machine | Run `time mnemonic keys list` and verify real < 1s |
| "copy now" warning visibility | CLI-01 | UX readability | Run `mnemonic keys create test` and verify warning is prominent |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** 2026-03-21 — Nyquist auditor: all 3 gaps filled, 10 integration tests green
