# Roadmap: Mnemonic

## Milestones

- ✅ **v1.0 MVP** — Phases 1-5 (shipped 2026-03-20)
- ✅ **v1.1 Memory Compaction** — Phases 6-9 (shipped 2026-03-20)
- ✅ **v1.2 Authentication / API Keys** — Phases 10-14 (shipped 2026-03-21)
- 🚧 **v1.3 CLI** — Phases 15-20 (in progress)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-5) — SHIPPED 2026-03-20</summary>

- [x] Phase 1: Foundation (3/3 plans) — completed 2026-03-19
- [x] Phase 2: Embedding (2/2 plans) — completed 2026-03-19
- [x] Phase 3: Service and API (3/3 plans) — completed 2026-03-19
- [x] Phase 4: Distribution (2/2 plans) — completed 2026-03-19
- [x] Phase 5: Config & Embedding Provider Cleanup (1/1 plan) — completed 2026-03-20

</details>

<details>
<summary>✅ v1.1 Memory Compaction (Phases 6-9) — SHIPPED 2026-03-20</summary>

- [x] Phase 6: Foundation (2/2 plans) — completed 2026-03-20
- [x] Phase 7: Summarization Engine (1/1 plan) — completed 2026-03-20
- [x] Phase 8: Compaction Core (2/2 plans) — completed 2026-03-20
- [x] Phase 9: HTTP Integration (1/1 plan) — completed 2026-03-20

</details>

<details>
<summary>✅ v1.2 Authentication / API Keys (Phases 10-14) — SHIPPED 2026-03-21</summary>

- [x] Phase 10: Auth Schema Foundation (2/2 plans) — completed 2026-03-20
- [x] Phase 11: KeyService Core (1/1 plan) — completed 2026-03-21
- [x] Phase 12: Auth Middleware (1/1 plan) — completed 2026-03-21
- [x] Phase 13: HTTP Wiring and REST Key Endpoints (2/2 plans) — completed 2026-03-21
- [x] Phase 14: CLI Key Management (2/2 plans) — completed 2026-03-21

</details>

### 🚧 v1.3 CLI (In Progress)

**Milestone Goal:** Turn the single binary into a full CLI tool with subcommands for every operation — serve the API, store/recall/search memories, compact, and manage keys, all from the terminal.

- [ ] **Phase 15: serve subcommand + CLI scaffolding** — Expand Commands enum; wire `mnemonic serve` as an explicit subcommand while preserving bare `mnemonic` backward compat
- [ ] **Phase 16: recall subcommand** — Fast DB-only path for listing and retrieving memories with structured filters; no embedding model load
- [ ] **Phase 17: remember subcommand** — Medium-init path (DB + embedding); store memories from CLI with stdin pipe support and full flag set
- [ ] **Phase 18: search subcommand** — Medium-init path; semantic search from CLI reusing the embedding init helper established in Phase 17
- [ ] **Phase 19: compact subcommand** — Most complex init (CompactionService with optional LLM engine); trigger compaction and dry-run preview from CLI
- [ ] **Phase 20: output polish** — Enforce `--json`, exit codes, and stderr/stdout consistency across all subcommands; eliminate all output inconsistencies

## Phase Details

### Phase 15: serve subcommand + CLI scaffolding
**Goal**: Users can explicitly invoke `mnemonic serve` to start the HTTP server, and existing bare `mnemonic` invocations continue working unchanged
**Depends on**: Phase 14 (v1.2 CLI foundation)
**Requirements**: CLI-01, CLI-02
**Success Criteria** (what must be TRUE):
  1. `mnemonic serve` starts the HTTP server and the server accepts requests exactly as before
  2. `mnemonic` (no args) still starts the HTTP server — no behavior change for existing deployments
  3. `mnemonic --help` shows `serve` in the subcommands list alongside `keys`
  4. All existing integration tests pass without modification after the Commands enum expansion
**Plans**: 1 plan
Plans:
- [ ] 15-01-PLAN.md — Add Serve variant, convert dispatch to match, add help-text integration tests

### Phase 16: recall subcommand
**Goal**: Users can retrieve and list memories from the terminal in under 100ms without loading the embedding model
**Depends on**: Phase 15
**Requirements**: RCL-01, RCL-02, RCL-03
**Success Criteria** (what must be TRUE):
  1. `mnemonic recall` lists recent memories in human-readable tabular format
  2. `mnemonic recall --id <uuid>` retrieves a single specific memory by ID
  3. `mnemonic recall --agent-id <id> --session-id <id> --limit 10` returns filtered results
  4. Command completes in under 100ms (DB-only path, no embedding model loaded)
**Plans**: TBD

### Phase 17: remember subcommand
**Goal**: Users can store memories directly from the terminal with a positional argument or piped stdin, with full agent/session/tag metadata
**Depends on**: Phase 16
**Requirements**: REM-01, REM-02, REM-03, REM-04
**Success Criteria** (what must be TRUE):
  1. `mnemonic remember "content"` embeds and stores a memory, printing the new memory ID to stdout
  2. `echo "content" | mnemonic remember` works identically when stdin is piped (no positional arg required)
  3. `mnemonic remember "content" --agent-id <id> --session-id <id> --tags tag1,tag2` stores with full metadata
  4. The embedding model loads via spawn_blocking without blocking the tokio runtime
**Plans**: TBD

### Phase 18: search subcommand
**Goal**: Users can perform semantic search from the terminal with result ranking and filtering flags
**Depends on**: Phase 17
**Requirements**: SRC-01, SRC-02
**Success Criteria** (what must be TRUE):
  1. `mnemonic search "query"` returns ranked semantic search results in tabular format with similarity scores
  2. `mnemonic search "query" --limit 5 --threshold 0.8 --agent-id <id> --session-id <id>` applies all filters correctly
  3. Command calls `MemoryService::search_memories()` directly without reimplementing search logic
**Plans**: TBD

### Phase 19: compact subcommand
**Goal**: Users can trigger and preview memory compaction from the terminal with agent scoping and threshold control
**Depends on**: Phase 18
**Requirements**: CMP-01, CMP-02, CMP-03
**Success Criteria** (what must be TRUE):
  1. `mnemonic compact` triggers compaction and prints a summary of clusters merged and memories removed
  2. `mnemonic compact --dry-run` previews what would be compacted without mutating any data
  3. `mnemonic compact --agent-id <id> --threshold 0.85` scopes compaction to one agent with custom similarity threshold
  4. CompactionService constructs correctly in a CLI context with optional LLM engine
**Plans**: TBD

### Phase 20: output polish
**Goal**: All subcommands produce consistent, machine-composable output — `--json` flag works everywhere, exit codes are correct, and data/errors are split across stdout/stderr
**Depends on**: Phase 19
**Requirements**: OUT-01, OUT-02, OUT-03, OUT-04
**Success Criteria** (what must be TRUE):
  1. All subcommands default to human-readable formatted text output when no flags are passed
  2. `mnemonic <any-subcommand> --json` produces valid JSON on stdout for every subcommand
  3. All subcommands exit with code 0 on success and code 1 on any error
  4. All error messages and warnings appear on stderr; all data output appears on stdout
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation | v1.0 | 3/3 | Complete | 2026-03-19 |
| 2. Embedding | v1.0 | 2/2 | Complete | 2026-03-19 |
| 3. Service and API | v1.0 | 3/3 | Complete | 2026-03-19 |
| 4. Distribution | v1.0 | 2/2 | Complete | 2026-03-19 |
| 5. Config Cleanup | v1.0 | 1/1 | Complete | 2026-03-20 |
| 6. Foundation | v1.1 | 2/2 | Complete | 2026-03-20 |
| 7. Summarization Engine | v1.1 | 1/1 | Complete | 2026-03-20 |
| 8. Compaction Core | v1.1 | 2/2 | Complete | 2026-03-20 |
| 9. HTTP Integration | v1.1 | 1/1 | Complete | 2026-03-20 |
| 10. Auth Schema Foundation | v1.2 | 2/2 | Complete | 2026-03-20 |
| 11. KeyService Core | v1.2 | 1/1 | Complete | 2026-03-21 |
| 12. Auth Middleware | v1.2 | 1/1 | Complete | 2026-03-21 |
| 13. HTTP Wiring and REST Key Endpoints | v1.2 | 2/2 | Complete | 2026-03-21 |
| 14. CLI Key Management | v1.2 | 2/2 | Complete | 2026-03-21 |
| 15. serve subcommand + CLI scaffolding | v1.3 | 0/1 | Planned | - |
| 16. recall subcommand | v1.3 | 0/TBD | Not started | - |
| 17. remember subcommand | v1.3 | 0/TBD | Not started | - |
| 18. search subcommand | v1.3 | 0/TBD | Not started | - |
| 19. compact subcommand | v1.3 | 0/TBD | Not started | - |
| 20. output polish | v1.3 | 0/TBD | Not started | - |
