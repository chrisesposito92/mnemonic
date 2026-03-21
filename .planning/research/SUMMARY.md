# Project Research Summary

**Project:** Mnemonic — Rust agent memory server
**Domain:** Rust single-binary CLI tool with embedded vector search and local ML inference (v1.3 — CLI subcommands)
**Researched:** 2026-03-21
**Confidence:** HIGH (all four research files sourced from direct codebase inspection, official docs, and verified benchmarks)

## Executive Summary

Mnemonic v1.3 is a targeted, well-scoped milestone: turn an existing server binary with a `keys` CLI into a full CLI tool where every REST operation — `serve`, `remember`, `recall`, `search`, `compact` — has a matching subcommand. The research is exceptionally high-confidence because this is an extension of shipped code, not greenfield work. The architectural pattern is already established by the v1.2 `keys` subcommand: CLI handlers call service layer methods directly (bypassing HTTP), and the dispatch tier matches initialization cost to subcommand requirements. The only new patterns introduced in v1.3 are a medium-init tier (DB + embedding model, ~2-3s cold start) and five new handler functions in `cli.rs`.

The recommended approach is strictly additive. No new modules, no new files beyond what already exist, and no new Cargo.toml dependencies. The five new subcommands slot into the existing `Commands` enum and dispatch pattern in `main.rs`, the existing `MemoryService` and `CompactionService` methods are called directly without modification, and the existing `cli.rs` module gains the handler implementations. The entire v1.3 delta is concentrated in two files: `cli.rs` (major additions) and `main.rs` (modified dispatch). SQLite WAL mode, already enabled, handles concurrent CLI + server access safely without any application-level locking.

The key risk is cold-start UX: `remember`, `search`, and `compact` all load the local embedding model (~2-3s). This is a known, accepted cost documented in the research — the mitigation for interactive use (HTTP fast path to a running server) is explicitly scoped to v1.4. The v1.3 approach is correct: build the direct-DB path first, ship it, then optimize based on user feedback. The `recall` subcommand is the one fast-path operation (DB only, ~50ms) and should be implemented before the embedding-dependent commands to validate the dispatch infrastructure at low cost.

## Key Findings

### Recommended Stack

The v1.3 stack is identical to v1.2. Zero new dependencies are required. The existing clap 4.6 (derive), tokio-rusqlite 0.7, candle/hf-hub (for local embedding), serde_json, and sqlite-vec 0.1.7 cover all v1.3 needs. Stdin TTY detection for `mnemonic remember` pipe support uses `std::io::IsTerminal` — available in Rust 1.70+ stdlib with no external crate needed.

**Core technologies (locked from v1.2):**
- **clap 4.6 (derive):** Subcommand parsing — derive macros generate all boilerplate; already used for `keys` CLI
- **tokio-rusqlite 0.7:** Async SQLite access — all CLI handlers use `conn.call()` pattern, same as server
- **candle-core + hf-hub:** Local ML inference — medium-init branches load the embedding model on demand via `tokio::task::spawn_blocking`
- **serde_json:** Structured output — `--json` flag on all subcommands requires zero new work; types already derive `Serialize`
- **sqlite-vec 0.1.7:** KNN vector search — `search` subcommand uses the same `search_memories()` method as the HTTP handler

**No new Cargo.toml entries required for v1.3.**

### Expected Features

Research sourced from redis-cli, HTTPie, ripgrep, heroku-cli, clap docs, and direct inspection of the existing `mnemonic keys` CLI conventions.

**Must have (table stakes — P1 for v1.3):**
- `mnemonic serve` — explicit subcommand with backward-compat default (no args = server still starts)
- `mnemonic remember <content>` — store memory with `--agent-id`, `--session-id`, `--tags` flags; stdin pipe support
- `mnemonic recall <id>` — retrieve by ID or structured filter (DB-only, no model load, fast path ~50ms)
- `mnemonic search <query>` — semantic search with `--agent-id`, `--limit`, `--threshold` flags
- `mnemonic compact` — trigger compaction with `--agent-id`, `--dry-run` flags
- `--json` flag on all subcommands — newline-delimited JSON for composability with `jq`
- Exit code 0/1, errors to stderr, data to stdout — Unix convention, already established in `keys` CLI
- `--agent-id` and `--session-id` on all data commands — multi-agent namespacing is Mnemonic's core value

**Should have (differentiators — P1/P2):**
- Stdin pipe support on `remember` — `echo "learned X" | mnemonic remember` follows Unix convention; zero additional dependencies via `std::io::IsTerminal`
- `--dry-run` on `compact` — already supported by REST API; surfaces it to CLI operators without new code
- `--threshold` on `search` — advanced users tuning search quality; maps directly to existing `SearchParams`

**Defer to v1.4+:**
- HTTP fast path: CLI subcommands POST to running server to skip model cold-start (~2s)
- `mnemonic delete <id>` with explicit `--yes` confirmation flag
- Color-coded search output with similarity score indicators (TTY-only)

**Confirmed out of scope:**
- Interactive REPL mode — anti-feature; `mnemonic serve` IS the persistent process
- Background daemon mode — use systemd/launchd; daemonization is platform-specific complexity
- `--format csv/table/json` multi-format — `--json` + jq covers all machine formats
- `mnemonic import <file>` — shell loop + `mnemonic remember` covers it

### Architecture Approach

The architecture is an extension of the proven v1.2 pattern: a tiered initialization dispatch in `main.rs` that selects the minimum startup cost for each subcommand, with thin CLI handler functions in `cli.rs` that call service layer methods directly. No new module structure is introduced; the delta is approximately 200-300 lines across two existing files.

**Major components (all existing, roles unchanged):**
1. **`main.rs` dispatch** — routes to the correct init tier (minimal, medium, or full server) based on the `Commands` variant; gains 5 new branches and a shared `init_db_and_embedding()` helper
2. **`cli.rs` handlers** — thin functions that parse CLI args, call service methods, and format output; gains `run_remember`, `run_recall`, `run_search`, `run_compact`
3. **`MemoryService` / `CompactionService`** — unchanged; CLI callers use the same methods as HTTP handlers with no duplication of business logic
4. **SQLite + WAL mode** — unchanged; concurrent CLI + server access is safe by design

**Init tiers (critical architectural pattern):**
- Minimal (DB only, ~50ms): `recall`, `keys` — structured filter queries; no embedding needed
- Medium (DB + embedding, ~2-3s): `remember`, `search`, `compact` — all require the local ML model
- Full (DB + embedding + LLM + server bind): `serve` — existing behavior, unchanged

**Key patterns:**
- Direct service calls, not HTTP — CLI never calls the running server; works whether server is up or not
- Output design: ID on line 1 (pipeable), tabular for lists, errors to stderr, `--json` for machine output
- `serve` as named subcommand with `None` fallback — `mnemonic` (no args) still starts the server; backward compat preserved

### Critical Pitfalls

The PITFALLS.md covers the full v1.0 through v1.2 history. The most relevant for v1.3 implementation are:

1. **Loading embedding model for `recall`** — `recall` is a structured filter query; loading the model adds ~2s to a command that should be <100ms. Use minimal init (DB only) for `recall`. This is the most likely mistake when copy-pasting from the medium-init branches.

2. **CLI commands going through HTTP** — implementing `mnemonic remember` as an HTTP client that calls `POST /memories` requires the server to be running, adds network stack overhead, and complicates auth. Call service methods directly; SQLite WAL handles concurrent access transparently.

3. **Blocking the tokio runtime during model load** — `LocalEngine::new()` is blocking (HF Hub I/O, model weight parsing). Must use `tokio::task::spawn_blocking(|| LocalEngine::new()).await??` — the same pattern already used in the server path.

4. **Duplicating init logic across dispatch branches** — three medium-init branches (`remember`, `search`, `compact`) that each copy-paste the DB + embedding setup block become a maintenance hazard. Extract `init_db_and_embedding(config: &Config)` as a private helper in `main.rs`.

5. **Reimplementing business logic in CLI handlers** — `run_search` should call `service.search_memories()`, not reconstruct the SQL query. CLI handlers are thin wrappers: parse args, call service, format output.

6. **Breaking backward compat on the no-subcommand default** — existing deployments run `mnemonic` with no args and expect the server to start. The `None` arm in dispatch must continue starting the server. `Commands::Serve` is additive; the default behavior is unchanged.

## Implications for Roadmap

Based on the combined research, the natural build order is defined by two constraints: (1) dependencies between subcommands on the `init_db_and_embedding` helper, and (2) the principle of validating new dispatch infrastructure at lowest cost before adding expensive initialization paths.

### Phase A: `serve` Subcommand + Commands Enum Expansion

**Rationale:** Zero-risk rename of existing behavior. Validates that the `Commands` enum expansion compiles and all existing integration tests pass without change. Establishes the `run_server()` helper that both the `None` and `Commands::Serve` arms call.
**Delivers:** `mnemonic serve` works explicitly; `mnemonic` (no args) still starts the server; backward compat confirmed.
**Addresses:** Table stakes — explicit `serve` subcommand, backward-compat default.
**Avoids:** Breaking existing deployments (Pitfall 6 — `None` arm preserved).
**Research flag:** Standard patterns — no additional phase research needed; pattern is documented in ARCHITECTURE.md with code examples.

### Phase B: `recall` Subcommand (Minimal Init)

**Rationale:** First data subcommand uses the cheapest init path (DB only, no embedding). Validates the new dispatch branches and table output formatting without touching the model-load complexity. This is the fastest subcommand to implement and tests the infrastructure that `remember`, `search`, and `compact` build on.
**Delivers:** `mnemonic recall --id <uuid>` and `mnemonic recall --agent-id <id>` work and return fast (<100ms).
**Addresses:** `recall` table stakes; `--agent-id`, `--limit` flags; exit codes; stderr/stdout discipline.
**Avoids:** Anti-pattern of loading embedding model for a DB-only operation (Pitfall 1).
**Research flag:** Standard patterns — mirrors existing `keys` CLI exactly.

### Phase C: `remember` Subcommand (Medium Init + Helper Extraction)

**Rationale:** Introduces the medium-init tier for the first time. Requires extracting `init_db_and_embedding()` as a private helper in `main.rs` — this helper is then reused by Phases D and E without duplication. `remember` is the write path and confirms the full embed + insert round-trip from CLI. Stdin pipe support adds zero dependencies using `std::io::IsTerminal`.
**Delivers:** `mnemonic remember "text" --agent-id <id>` stores a memory; prints ID on line 1 of stdout (pipeable); `echo "text" | mnemonic remember` works.
**Addresses:** `remember` table stakes; stdin pipe differentiator; `--agent-id`, `--session-id`, `--tags` flags.
**Avoids:** Duplicating init logic (Pitfall 4 — extract helper here); blocking tokio runtime on model load (Pitfall 3 — spawn_blocking).
**Research flag:** Standard patterns — medium-init pattern is described with code examples in ARCHITECTURE.md.

### Phase D: `search` Subcommand (Medium Init, Reuses Helper)

**Rationale:** Same medium-init pattern as Phase C; reuses the `init_db_and_embedding()` helper established in Phase C. Validates the semantic search path from CLI including all flag mapping to `SearchParams`.
**Delivers:** `mnemonic search "query" --agent-id <id> --limit 5 --threshold 0.8` returns ranked results in tabular format.
**Addresses:** `search` table stakes; `--limit`, `--threshold`, `--agent-id`, `--session-id` flags; tabular output with similarity scores.
**Avoids:** Reimplementing search logic (Pitfall 5 — calls `service.search_memories()` directly).
**Research flag:** Standard patterns — identical medium-init pattern to Phase C.

### Phase E: `compact` Subcommand (Medium Init, CompactionService)

**Rationale:** Most complex CLI handler — `CompactionService` construction is more involved than `MemoryService` (requires optional LLM engine). Implemented last because it depends on the medium-init helper from Phase C and the output formatting conventions established by Phases B-D. The `--dry-run` flag is highest-value test case and should be validated first before any data-mutating path.
**Delivers:** `mnemonic compact --agent-id <id> --dry-run` previews compaction; without `--dry-run` executes it; summary output with cluster counts.
**Addresses:** `compact` table stakes; `--dry-run` differentiator; compaction feedback output.
**Avoids:** Reimplementing compaction logic (Pitfall 5 — calls `compaction.compact()` directly); blocking tokio on similarity clustering (spawn_blocking for CPU-bound work).
**Research flag:** May benefit from a quick inspection of `compaction.rs` initialization sequence before implementation — the `CompactionService` constructor with optional LLM engine has more moving parts than `MemoryService`.

### Phase F: `--json` Flag + Output Polish

**Rationale:** The `--json` flag works across all subcommands using the same `Serialize` derives already on the response types. Implementing it after all subcommands exist avoids retrofitting output code across multiple in-progress handlers. Cold-start stderr message (for medium-init commands) is also handled here.
**Delivers:** `mnemonic search "query" --json | jq '.'` produces newline-delimited JSON; all subcommands support `--json`; stderr "loading embedding model..." message for medium-init commands (suppressed in `--json` mode).
**Addresses:** `--json` table stakes; newline-delimited JSON for `search` (ripgrep pattern); cold-start UX.
**Research flag:** Confirm newline-delimited JSON (ripgrep pattern) vs. JSON array for `search` output matches what agent consumers expect before shipping.

### Phase Ordering Rationale

- `serve` first because it touches zero new code paths — validates enum expansion does not break the build.
- `recall` second because it uses minimal init and produces the dispatch infrastructure that medium-init phases depend on.
- `remember` third because it introduces `init_db_and_embedding()` — the reusable helper that `search` and `compact` share.
- `search` fourth because it follows an identical pattern to `remember` with a different service call.
- `compact` last because `CompactionService` is the most complex to construct; the `--dry-run` path is critical to test before any data mutation.
- `--json` last because it is pure output formatting and does not gate any other capability.

### Research Flags

Phases likely needing a research-phase check during planning:
- **Phase E (compact):** `CompactionService` construction from a CLI context (with optional LLM engine) has more moving parts than the other subcommands. Worth inspecting the current `compaction.rs` initialization sequence before implementation begins to confirm the correct assembly order.
- **Phase F (--json output format):** The newline-delimited JSON pattern for `search` is a deliberate UX choice sourced from ripgrep. Worth confirming this matches what agent consumers actually expect before shipping.

Phases with standard patterns (skip research-phase):
- **Phase A (serve):** Trivial clap enum extension; the pattern is established and documented in ARCHITECTURE.md.
- **Phase B (recall):** DB-only path mirrors existing `keys` CLI exactly. No unknowns.
- **Phase C (remember):** Medium-init pattern is described with code examples in ARCHITECTURE.md.
- **Phase D (search):** Identical medium-init pattern; no new architecture concerns.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Zero new dependencies; all technologies are locked from v1.2; verified via direct Cargo.toml inspection |
| Features | HIGH | Sourced from redis-cli, HTTPie, ripgrep, heroku-cli patterns + direct inspection of existing `cli.rs` and `service.rs` |
| Architecture | HIGH | Direct inspection of shipped v1.2 source code (5,925 lines across 12 files); patterns are proven in production; ARCHITECTURE.md includes explicit code examples |
| Pitfalls | HIGH | Critical pitfalls verified via official docs and known issues; v1.3-specific pitfalls derived from direct code analysis |

**Overall confidence:** HIGH

### Gaps to Address

- **Cold-start UX validation:** The ~2-3s embedding model load is accepted for v1.3, but the actual user experience has not been validated against real workflows. The stderr "loading embedding model..." message mitigates this — confirm the message is suppressed correctly in `--json` mode.
- **`CompactionService` CLI construction:** ARCHITECTURE.md notes that `compact` requires the most complex init path (optional LLM engine). The exact initialization sequence for a CLI context should be confirmed against `compaction.rs` before Phase E begins.
- **`MemoryService::get_memory(id)` method:** ARCHITECTURE.md notes that `recall --id <uuid>` may need a new `get_memory()` method on `MemoryService` if one does not currently exist. Confirm before Phase B begins — this is a small addition but affects the plan.
- **Stdin detection MSRV:** `std::io::IsTerminal` requires Rust 1.70+. Confirm the project's MSRV in `Cargo.toml` supports this before using it for stdin pipe detection in `remember`.

## Sources

### Primary (HIGH confidence)
- Mnemonic `src/cli.rs` — existing keys CLI conventions (dispatch pattern, stdout/stderr split, table format, fast path)
- Mnemonic `src/main.rs` — existing Commands enum, dispatch, model init sequence, fast path for keys
- Mnemonic `src/service.rs` — `MemoryService`, `SearchParams`, `ListParams`, `Memory`, `SearchResultItem` structs
- Mnemonic `src/compaction.rs` — `CompactionService`, `CompactRequest`, compaction logic
- clap 4.6 documentation — subcommand derive patterns (https://docs.rs/clap/latest/clap/_derive/_tutorial/)
- SQLite WAL mode documentation — concurrent access guarantees (https://www.sqlite.org/wal.html)

### Secondary (MEDIUM confidence)
- [Heroku CLI Style Guide](https://devcenter.heroku.com/articles/cli-style-guide) — stdout/stderr split, `--json` flag, table format
- [HTTPie Redirected Output docs](https://httpie.io/docs/cli/redirected-output) — TTY detection for auto pretty/plain mode
- [ripgrep `--json` output docs](https://learnbyexample.github.io/learn_gnugrep_ripgrep/ripgrep.html) — newline-delimited JSON for streaming
- [Rust CLI patterns 2026](https://dasroot.net/posts/2026/02/rust-cli-patterns-clap-cargo-configuration/) — clap derive subcommand patterns
- [Cloudflare workers-sdk discussion](https://github.com/cloudflare/workers-sdk/discussions/2940) — rationale for reserving stdout for machine-readable output

### Tertiary (LOW confidence)
- [14 tips for amazing CLIs](https://dev.to/wesen/14-great-tips-to-make-amazing-cli-applications-3gp3) — input/output design patterns (general, not Rust-specific)

---
*Research completed: 2026-03-21*
*Ready for roadmap: yes*
