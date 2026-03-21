# Requirements: Mnemonic

**Defined:** 2026-03-21
**Core Value:** Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run

## v1.3 Requirements

Requirements for CLI subcommands milestone. Each maps to roadmap phases.

### CLI Scaffolding

- [ ] **CLI-01**: `mnemonic serve` starts the HTTP server (same behavior as current bare `mnemonic`)
- [ ] **CLI-02**: Bare `mnemonic` with no subcommand continues to start the server (backward compat)

### Remember

- [ ] **REM-01**: `mnemonic remember <content>` stores a memory with embedded content
- [ ] **REM-02**: `mnemonic remember` reads content from stdin when piped (no positional arg)
- [ ] **REM-03**: `mnemonic remember` accepts `--agent-id` and `--session-id` flags
- [ ] **REM-04**: `mnemonic remember` accepts `--tags` flag for tagging memories

### Recall

- [ ] **RCL-01**: `mnemonic recall` lists recent memories (DB-only, no model load)
- [ ] **RCL-02**: `mnemonic recall --id <uuid>` retrieves a specific memory
- [ ] **RCL-03**: `mnemonic recall` accepts `--agent-id`, `--session-id`, `--limit` filters

### Search

- [ ] **SRC-01**: `mnemonic search <query>` performs semantic search and displays results
- [ ] **SRC-02**: `mnemonic search` accepts `--limit`, `--threshold`, `--agent-id`, `--session-id` flags

### Compact

- [ ] **CMP-01**: `mnemonic compact` triggers memory compaction from CLI
- [ ] **CMP-02**: `mnemonic compact --dry-run` previews compaction without mutating data
- [ ] **CMP-03**: `mnemonic compact` accepts `--agent-id` and `--threshold` flags

### Output

- [ ] **OUT-01**: All subcommands default to human-readable formatted text output
- [ ] **OUT-02**: All subcommands support `--json` flag for machine-readable JSON output
- [ ] **OUT-03**: All subcommands use exit code 0 on success, 1 on error
- [ ] **OUT-04**: All subcommands send data to stdout and errors/warnings to stderr

## Future Requirements

### CLI Enhancements (v1.4+)

- **FAST-01**: CLI commands POST to running server to skip model load (fast-path HTTP)
- **CLR-01**: Color-coded search results with similarity scores (owo-colors)
- **DEL-01**: `mnemonic delete <id>` with `--yes` confirmation flag
- **IMP-01**: `mnemonic import <file>` batch import from JSON

## Out of Scope

| Feature | Reason |
|---------|--------|
| Interactive REPL mode | Model cold start makes REPL startup same as individual invocations; server already IS the persistent process |
| Background daemon mode (`--daemon`) | Platform-specific complexity; systemd/launchd handle this better |
| Multi-format output (`--format csv/table/json`) | `--json` + jq covers all machine formats; two modes (human/JSON) not three |
| Automatic model download | Model is bundled in binary; clear error better than silent download |
| Server URL config file | v1.3 CLI operates directly on local DB; HTTP fast-path deferred to v1.4 |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CLI-01 | Phase 15 | Pending |
| CLI-02 | Phase 15 | Pending |
| REM-01 | Phase 17 | Pending |
| REM-02 | Phase 17 | Pending |
| REM-03 | Phase 17 | Pending |
| REM-04 | Phase 17 | Pending |
| RCL-01 | Phase 16 | Pending |
| RCL-02 | Phase 16 | Pending |
| RCL-03 | Phase 16 | Pending |
| SRC-01 | Phase 18 | Pending |
| SRC-02 | Phase 18 | Pending |
| CMP-01 | Phase 19 | Pending |
| CMP-02 | Phase 19 | Pending |
| CMP-03 | Phase 19 | Pending |
| OUT-01 | Phase 20 | Pending |
| OUT-02 | Phase 20 | Pending |
| OUT-03 | Phase 20 | Pending |
| OUT-04 | Phase 20 | Pending |

**Coverage:**
- v1.3 requirements: 18 total
- Mapped to phases: 18
- Unmapped: 0

---
*Requirements defined: 2026-03-21*
*Last updated: 2026-03-21 after roadmap creation (v1.3)*
