---
phase: 22
slug: config-extension-backend-factory-and-config-cli
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-21
---

# Phase 22 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test framework + tokio::test |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --lib config::tests storage::tests cli::tests` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib config::tests storage::tests cli::tests`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 22-01-01 | 01 | 1 | CONF-01 | unit | `cargo test --lib config::tests::test_config_defaults_storage_provider` | ✅ | ✅ green |
| 22-01-01 | 01 | 1 | CONF-01 | unit | `cargo test --lib config::tests::test_validate_config_sqlite_ok` | ✅ | ✅ green |
| 22-01-01 | 01 | 1 | CONF-01 | unit | `cargo test --lib config::tests::test_validate_config_qdrant_no_url` | ✅ | ✅ green |
| 22-01-01 | 01 | 1 | CONF-01 | unit | `cargo test --lib config::tests::test_validate_config_qdrant_with_url` | ✅ | ✅ green |
| 22-01-01 | 01 | 1 | CONF-01 | unit | `cargo test --lib config::tests::test_validate_config_postgres_no_url` | ✅ | ✅ green |
| 22-01-01 | 01 | 1 | CONF-01 | unit | `cargo test --lib config::tests::test_validate_config_postgres_with_url` | ✅ | ✅ green |
| 22-01-01 | 01 | 1 | CONF-01 | unit | `cargo test --lib config::tests::test_validate_config_unknown_storage_provider` | ✅ | ✅ green |
| 22-01-01 | 01 | 1 | CONF-01 | unit | `cargo test --lib config::tests::test_storage_provider_env_override` | ✅ | ✅ green |
| 22-01-01 | 01 | 1 | CONF-01 | unit | `cargo test --lib config::tests::test_storage_provider_toml_override` | ✅ | ✅ green |
| 22-01-02 | 01 | 1 | CONF-02 | unit | `cargo test --lib storage::tests::test_create_backend_sqlite` | ✅ | ✅ green |
| 22-01-02 | 01 | 1 | CONF-02 | unit | `cargo test --lib storage::tests::test_create_backend_qdrant_no_feature` | ✅ | ✅ green |
| 22-01-02 | 01 | 1 | CONF-02 | unit | `cargo test --lib storage::tests::test_create_backend_postgres_no_feature` | ✅ | ✅ green |
| 22-01-02 | 01 | 1 | CONF-02 | unit | `cargo test --lib storage::tests::test_create_backend_unknown_provider` | ✅ | ✅ green |
| 22-02-01 | 02 | 2 | CONF-03 | unit | `cargo test --lib cli::tests::test_redact_option_some_returns_stars` | ✅ | ✅ green |
| 22-02-01 | 02 | 2 | CONF-03 | unit | `cargo test --lib cli::tests::test_redact_option_none_returns_null` | ✅ | ✅ green |
| 22-02-01 | 02 | 2 | CONF-03 | unit | `cargo test --lib cli::tests::test_redact_option_some_hides_actual_value` | ✅ | ✅ green |
| 22-02-01 | 02 | 2 | CONF-03 | CLI | `cargo test --test cli_integration test_config_show_exits_zero_and_prints_sections` | ✅ | ✅ green |
| 22-02-01 | 02 | 2 | CONF-03 | CLI | `cargo test --test cli_integration test_config_show_displays_expected_fields` | ✅ | ✅ green |
| 22-02-01 | 02 | 2 | CONF-03 | CLI | `cargo test --test cli_integration test_config_show_redacts_secrets` | ✅ | ✅ green |
| 22-02-01 | 02 | 2 | CONF-03 | CLI | `cargo test --test cli_integration test_config_show_json_outputs_valid_json_with_expected_keys` | ✅ | ✅ green |
| 22-02-01 | 02 | 2 | CONF-03 | CLI | `cargo test --test cli_integration test_config_show_json_redacts_secrets` | ✅ | ✅ green |
| 22-02-02 | 02 | 2 | CONF-04 | integration | `cargo test --test integration test_health` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Config show terminal formatting | CONF-03 | Visual readability | `cargo run -- config show` — check grouped output |
| Config show JSON formatting | CONF-03 | Visual inspection | `cargo run -- config show --json` — check valid JSON |
| Health endpoint live server | CONF-04 | Requires running server | Start server, `curl localhost:8080/health` |

---

## Validation Audit 2026-03-21

| Metric | Count |
|--------|-------|
| Gaps found | 2 |
| Resolved | 2 |
| Escalated | 0 |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 15s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-03-21
