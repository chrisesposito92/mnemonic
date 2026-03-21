# Feature Research

**Domain:** CLI subcommands for a Rust memory-server binary (v1.3 milestone)
**Researched:** 2026-03-21
**Confidence:** HIGH (patterns sourced from redis-cli, HTTPie, ripgrep, jq, heroku-cli, clap crate docs, existing mnemonic cli.rs)

---

## Scope Note

This document covers **only the new features for v1.3**: turning `mnemonic` from a server-with-keys-CLI into a full CLI tool where every REST operation has a matching subcommand. The v1.0/v1.1/v1.2 baseline (REST API, embeddings, compaction, auth, `mnemonic keys`) is already shipped and is a dependency, not a feature.

The central question: what does a well-designed `serve / remember / recall / search / compact` CLI look like, given patterns from proven tools in the space?

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features any CLI for a data tool provides. Missing these makes the CLI feel like a half-finished wrapper.

| Feature | Why Expected | Complexity | Dependencies on Existing Architecture |
|---------|--------------|------------|---------------------------------------|
| `mnemonic serve` subcommand that starts the HTTP server | Current default behavior (no subcommand = server) must become explicit. Every multi-command binary uses `serve` or `start` — Docker, uvicorn, gunicorn. Without this, adding subcommands breaks backward compat for existing users who run `mnemonic` raw. | LOW | Refactor `main.rs`: `None` command branch becomes `Commands::Serve`. Existing server init path is unchanged — just gated behind a variant. |
| `mnemonic remember <content>` stores a memory | Mirrors the REST `POST /memories`. Any key-value or document store CLI has a store/set/put subcommand. Redis has `SET`, sqlite3 can `INSERT`. A memory tool without a CLI store command is incomplete. | MEDIUM | Requires embedding model load (same as server). Must call `MemoryService::create_memory`. Fast path: if server is running, can POST via HTTP instead of direct DB. See "server vs. direct" decision below. |
| `mnemonic recall <id>` retrieves a memory by ID | Mirrors `GET /memories/:id`. Every data tool has a get-by-key command. Redis has `GET`, sqlite3 has `SELECT`. Without ID lookup, users cannot inspect a specific memory. | LOW | DB-only path — no embedding load needed. Wraps `MemoryService` or direct DB query for the `SELECT ... WHERE id = ?` case. |
| `mnemonic search <query>` performs semantic search | Mirrors `GET /memories/search?q=`. This is Mnemonic's core value. A semantic memory tool with no CLI search is broken — it is the primary operation agents and developers would want to test interactively. | MEDIUM | Requires embedding model load to embed the query. Calls `MemoryService::search_memories`. Slow cold start (~1-2s for local model) is expected and documented. |
| `mnemonic compact` triggers memory compaction | Mirrors `POST /memories/compact`. Compaction is already a CLI-oriented workflow (operators trigger it deliberately). Without a CLI trigger, developers must use curl, which is higher friction than the tool warrants. | LOW | Calls `CompactionService::compact`. Requires embedding model (for deduplication similarity). Dry-run flag (`--dry-run`) already exists in the REST layer and must be surfaced. |
| Human-readable output by default | Tools like redis-cli, sqlite3, and heroku-cli all default to human-readable tabular or plain text output. Developers interacting directly expect readable output. JSON-by-default is hostile for interactive use. | LOW | Print formatted text to stdout. Use `eprintln!` for warnings (same pattern as existing `keys` CLI). |
| `--json` flag for machine-readable output | Heroku CLI: `heroku releases --json`. ripgrep: `--json`. HTTPie: auto-detects TTY and adjusts. Scripts and agent orchestration tools need structured output. Without `--json`, piping to `jq` is impossible and shell automation breaks. | LOW | Serialize the same structs already used in HTTP responses via `serde_json::to_string`. The types (`Memory`, `SearchResultItem`, `ListResponse`) already derive `Serialize`. |
| Exit codes: 0 on success, 1 on error | POSIX standard. Every tool from grep to redis-cli uses exit code 0/1. Shell scripts (`&&`, `||`) depend on exit codes. Without this, automation breaks silently. | LOW | Already implemented in `keys` CLI via `std::process::exit(1)`. Extend same pattern to all subcommands. |
| Errors go to stderr, data goes to stdout | Core Unix convention (used by redis-cli, ripgrep, jq, HTTPie). "Data to stdout, messages to stderr." Allows `mnemonic search foo > results.json 2>/dev/null` without mixing error text into data stream. | LOW | Already established in `keys` CLI (`println!` for data, `eprintln!` for errors). Codify as project convention. |
| `--agent-id` flag on remember/recall/search/compact | Multi-agent namespacing is Mnemonic's core feature. All memory operations are already scoped by `agent_id`. A CLI that cannot specify which agent's memories to operate on is unusable for the primary use case. | LOW | Map `--agent-id` flag to the `agent_id` field in the existing request structs. Already done for `keys create --agent-id`. |
| `--limit` flag on recall and search | Mirrors `limit` query param on REST endpoints. Data tools always let users constrain result count (redis-cli `SCAN COUNT`, sqlite3 `LIMIT`). Without it, returning 10 results vs 100 requires API knowledge. | LOW | Map `--limit N` to `SearchParams.limit` / `ListParams.limit`. Default 10 (same as REST). |

### Differentiators (Competitive Advantage)

Features that are not universally expected but align with Mnemonic's positioning as a zero-friction, agent-aware tool.

| Feature | Value Proposition | Complexity | Dependencies on Existing Architecture |
|---------|-------------------|------------|---------------------------------------|
| Fast path: CLI memory commands skip model load when server is already running | The local embedding model takes ~1-2 seconds to load. If `mnemonic serve` is already running, `mnemonic remember` can POST to it via HTTP instead of loading the model again. This would make the CLI feel instant for interactive use when the server is up. | HIGH | Requires: (1) configurable `--server` URL flag, (2) HTTP client for CLI subcommands, (3) logic to try HTTP first, fall back to direct if server not available. Adds reqwest as a CLI dependency. Complex enough to defer to v1.4 — document as future work. |
| `mnemonic remember` accepts content from stdin | Unix pipe convention: `echo "learned X" \| mnemonic remember`. `cat notes.txt \| mnemonic remember`. HTTPie uses this pattern — stdin without positional arg = pipe input. Enables shell-scripting memory storage from any tool. | LOW | Check if content arg is absent AND stdin is not a TTY (`atty` crate or `std::io::stdin().is_terminal()`). Read stdin if so, error if neither present. |
| `--dry-run` flag on compact | Already supported by the REST endpoint. Exposing it in the CLI allows developers to preview what compaction would do before mutating data. Ripgrep does this with `--stats` for non-mutating analysis. Unique to Mnemonic — no other memory CLI tool offers this. | LOW | Map to existing `CompactionRequest.dry_run: true`. Print "would merge N memories" message. |
| `--threshold` flag on search | Mirrors `SearchParams.threshold` — minimum similarity score. Advanced users tuning search quality need this. jq users know they want to filter results; giving `--threshold 0.8` is more ergonomic than `\| jq '[.[] \| select(.distance < 0.2)]'`. | LOW | Map to `SearchParams.threshold`. Document valid range (0.0-1.0). Default: none (same as REST). |
| `--session-id` flag on remember/recall/search | Session-scoped retrieval is already supported by the REST API. Exposing it in the CLI makes per-conversation memory management possible without crafting HTTP requests. | LOW | Map `--session-id` to `session_id` fields in existing request structs. Same pattern as `--agent-id`. |
| Color-coded output for search results with similarity scores | Search results with similarity distances benefit from visual hierarchy — top results visually distinct from marginal matches. Heroku uses color for status indicators. Acceptable only when output is a TTY (disabled for pipes/JSON). | MEDIUM | Use `termcolor` or ANSI escape codes. Gate on `atty::is(atty::Stream::Stdout)`. Not essential for v1.3 — mark as P2. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Interactive REPL mode (like sqlite3 or redis-cli) | "It would be nice to run commands interactively without relaunching." | Model cold start (1-2s) makes REPL startup cost the same as individual invocations. The REST server already IS the persistent process. Building a separate REPL duplicates the server's purpose and adds a readline/rustyline dependency for a use case that `mnemonic serve` + HTTP client already covers. | Use `mnemonic serve` as the persistent process; interact via HTTPie, curl, or any HTTP client. |
| Background daemon mode (`mnemonic serve --daemon` / `--background`) | "I want `mnemonic serve` to run in the background without occupying a terminal." | Proper daemonization requires double-fork, pid files, signal handling — significant platform-specific complexity. Cross-platform (macOS/Linux) daemonization in Rust is non-trivial and error-prone. Systemd/launchd already provide this for users who need it. | Document systemd service file and launchd plist in README. The binary itself stays foreground. |
| `--format table/json/csv` multi-format output | "Give me CSV for spreadsheets." | Three format renderers for every command multiplies implementation surface. CSV is lossy for nested data (tags arrays, metadata). jq already transforms JSON into any format. | `--json` + jq covers all machine formats. Human output covers the interactive case. Two modes, not three. |
| `mnemonic delete <id>` subcommand | "I need to delete memories from the CLI." | Delete is a destructive operation. A CLI delete that bypasses confirmation adds risk (typos, scripting accidents). REST `DELETE /memories/:id` exists for programmatic deletion. If added, it needs `--confirm` or `--yes` flag — scope creep. | Document using `curl -X DELETE` for now. Add in v1.4 with explicit `--yes` flag if user demand materializes. |
| `mnemonic import <file>` batch import | "I have a JSON file of memories to bulk load." | Batch import is a non-trivial problem: error handling per-record, deduplication behavior, partial failures. The REST API already accepts one memory at a time and can be called in a loop by a shell script. | `jq -c '.[]' memories.json \| while read m; do mnemonic remember "$m"; done` covers it. No special command needed. |
| Automatic model download on first run | "The user shouldn't have to think about the model." | The model is already bundled in the binary for `mnemonic serve`. CLI subcommands share the same binary — no download step needed. If the model is somehow absent, a clear error is better than a silent download during what the user thinks is a fast command. | Error message: "embedding model not found; run mnemonic serve once to verify binary integrity." |
| Server URL config in `~/.config/mnemonic/config.toml` | "I want to configure the server URL once for all CLI invocations." | For v1.3, CLI subcommands operate directly on the local DB (same path as `mnemonic serve`). There is no "server URL" concept yet — the CLI bypasses HTTP entirely. | `--db` global flag (already implemented) selects which DB file to use. Server URL concept is deferred to the fast-path HTTP optimization (v1.4+). |

---

## Feature Dependencies

```
[mnemonic serve]
    └──requires──> [Commands enum refactor in cli.rs] (add Serve variant, make None = show help or default to Serve)

[mnemonic remember <content>]
    └──requires──> [EmbeddingEngine initialization] (model load, ~1-2s cold start)
    └──requires──> [MemoryService::create_memory] (existing — no changes)
    └──requires──> [stdin detection] (for pipe support — new, LOW complexity)
    └──enhances──> [mnemonic serve] (fast path: POST to running server skips model load — v1.4)

[mnemonic recall <id>]
    └──requires──> [DB-only path in main.rs] (no embedding load — same fast-path as keys CLI)
    └──requires──> [MemoryService::get_memory by ID] (may need new method if not exposed)

[mnemonic search <query>]
    └──requires──> [EmbeddingEngine initialization] (model load — unavoidable for semantic search)
    └──requires──> [MemoryService::search_memories] (existing — no changes)
    └──requires──> [--threshold, --limit, --agent-id, --session-id flags] (map to SearchParams)

[mnemonic compact]
    └──requires──> [EmbeddingEngine initialization] (for similarity clustering)
    └──requires──> [CompactionService::compact] (existing — no changes)
    └──requires──> [--dry-run flag] (maps to existing CompactionRequest.dry_run)

[--json flag (all subcommands)]
    └──requires──> [Memory, SearchResultItem, ListResponse already derive Serialize] (existing — no changes)

[stdin pipe support (remember)]
    └──requires──> [TTY detection] (atty crate or std::io::IsTerminal — Rust 1.70+)
    └──conflicts──> [positional <content> arg] (if stdin is a pipe, positional arg is absent)
```

### Dependency Notes

- **`recall` is the only DB-only path:** `remember`, `search`, and `compact` all require the embedding model to be loaded. `recall <id>` is a simple `SELECT WHERE id=?` — it can follow the same fast-path as `mnemonic keys` (no model init, no validate_config). This makes `recall` feel instant while the others have the known cold-start cost.
- **`serve` variant must be the default for backward compat:** Existing users may run `mnemonic` with no subcommand and expect it to start the server. The safest approach: `None` arm in `main.rs` continues to start the server (or shows help with a deprecation notice). `Commands::Serve` explicitly starts it. This avoids breaking existing deployments.
- **`remember` and `search` share the model init path:** Both need the embedding engine. The model init code in `main.rs` can be extracted into a helper `init_embedding_engine(&config)` to avoid duplication across three command handlers.
- **`compact` requires both embedding AND optional LLM engine:** Same as server init. The compact CLI path must mirror the full server init sequence for services (DB + embedding + optional LLM). This is the most expensive CLI cold start.

---

## MVP Definition

### Ship in v1.3

- [ ] `mnemonic serve` — explicit subcommand to start HTTP server (backward-compat default)
- [ ] `mnemonic remember <content>` — store a memory with `--agent-id`, `--session-id`, `--tags` flags
- [ ] `mnemonic remember` reads from stdin when content arg absent (pipe support)
- [ ] `mnemonic recall <id>` — retrieve a single memory by ID (DB-only fast path, no model load)
- [ ] `mnemonic search <query>` — semantic search with `--agent-id`, `--session-id`, `--limit`, `--threshold` flags
- [ ] `mnemonic compact` — trigger compaction with `--agent-id`, `--dry-run` flags
- [ ] `--json` global flag on all subcommands for machine-readable output
- [ ] Human-readable default output (memory ID + content truncated + metadata)
- [ ] Exit code 0/1 on all paths
- [ ] Errors to stderr, data to stdout (all subcommands)

### Add After Validation (v1.4+)

- [ ] Fast path: CLI subcommands POST to running server via HTTP (skip model load) — add when users report cold-start friction
- [ ] `mnemonic delete <id>` with explicit `--yes` confirmation flag
- [ ] Color-coded search output with similarity score indicators (TTY-only)
- [ ] `mnemonic keys rotate <id>` — zero-downtime key rotation helper

### Confirmed Out of Scope (v1.3)

- [ ] Interactive REPL mode — `mnemonic serve` IS the persistent process
- [ ] Background daemon mode — use systemd/launchd
- [ ] `--format csv/table/json` multi-format — `--json` + jq covers it
- [ ] `mnemonic import <file>` — shell loop + `mnemonic remember` covers it
- [ ] Server URL config (`~/.config/mnemonic/`) — CLI operates on local DB directly

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| `mnemonic serve` subcommand + backward compat | HIGH | LOW | P1 |
| `mnemonic remember <content>` | HIGH | MEDIUM | P1 |
| Stdin pipe support for `remember` | HIGH | LOW | P1 |
| `mnemonic recall <id>` (fast path) | HIGH | LOW | P1 |
| `mnemonic search <query>` | HIGH | MEDIUM | P1 |
| `mnemonic compact` with `--dry-run` | MEDIUM | MEDIUM | P1 |
| `--json` flag (all subcommands) | HIGH | LOW | P1 |
| `--agent-id` / `--session-id` on all commands | HIGH | LOW | P1 |
| `--limit` / `--threshold` on search | MEDIUM | LOW | P1 |
| Exit code 0/1 + stderr/stdout discipline | HIGH | LOW | P1 |
| `mnemonic delete <id>` | MEDIUM | LOW | P2 |
| Color-coded search output (TTY-only) | LOW | MEDIUM | P3 |
| Fast path: HTTP to running server | HIGH | HIGH | P2 (v1.4) |

**Priority key:**
- P1: Must have for v1.3 to feel like a complete CLI
- P2: High value, add when possible (v1.3 or v1.4)
- P3: Nice to have, future consideration

---

## CLI UX Reference: Patterns from Established Tools

### Output Format Conventions

**From Heroku CLI (highest relevance — similar developer tool audience):**
- Default output is grep-parseable human text with column headers
- `--json` returns a JSON array, enables `jq` composition
- Stdout for data, stderr for progress/errors
- Tables with fixed-width columns (already established in `mnemonic keys list`)

**From ripgrep:**
- `--json` emits newline-delimited JSON objects (one per match) rather than a JSON array
- Enables streaming: `rg --json | jq 'select(.type=="match")'`
- For memory search results (N matches), newline-delimited JSON per result is more composable than a wrapped array
- Recommendation: `--json` on search emits one JSON object per line (the `SearchResultItem` struct)

**From HTTPie:**
- Auto-detects TTY: pretty-printed colored output for terminals, raw JSON when piped
- This is the ideal UX but requires `atty` crate and adds complexity
- For v1.3: `--json` flag is explicit opt-in; auto-detect is a P2 enhancement

**From redis-cli:**
- No output on success for mutating commands (`SET key val` returns `OK`, not the value)
- Read commands return just the value, no decoration
- Error output: `(error) ERR ...` with parenthetical prefix
- Mnemonic analogy: `mnemonic remember` returns the memory ID on success (one line, pipeable)

**From sqlite3 CLI:**
- `.mode` command switches output format (not applicable to Mnemonic's subcommand model)
- Default output is pipe-separated values — too raw for humans
- Mnemonic should NOT follow sqlite3's default output; use Heroku-style tabular output

### Input Conventions

**Positional argument for primary data:**
```
mnemonic remember "The user prefers dark mode"
mnemonic recall 01920abc-...
mnemonic search "user interface preferences"
```

**Flags for metadata/filters:**
```
mnemonic remember "content" --agent-id claude --session-id sess_001 --tags "ui,preferences"
mnemonic search "dark mode" --agent-id claude --limit 5 --threshold 0.7
mnemonic compact --agent-id claude --dry-run
```

**Stdin for pipe workflows (remember only):**
```
echo "learned from conversation" | mnemonic remember --agent-id claude
cat meeting_notes.txt | mnemonic remember --agent-id assistant --tags "meetings"
```

### Error Handling Conventions

Based on clap + existing `mnemonic keys` implementation:
- Invalid args: clap handles automatically with usage message
- DB errors: `eprintln!("error: {}", e); std::process::exit(1)`
- Not found (recall by ID): `eprintln!("error: memory '{}' not found", id); std::process::exit(1)`
- Model load failure: `eprintln!("error: failed to load embedding model: {}", e); std::process::exit(1)`
- Empty result (search/recall): print "No memories found." to stdout (not an error), exit 0

### Cold Start Warning

The local embedding model (all-MiniLM-L6-v2) takes 1-2 seconds to load. This affects `remember`, `search`, and `compact`. Conventions from tools with known startup costs (Java CLIs, Python ML tools):
- Do NOT print a spinner or progress bar for v1.3 (adds complexity)
- DO print a startup message to stderr if startup exceeds 1 second: `"loading embedding model..."` (stderr only, invisible in pipes)
- For `--json` mode, suppress all startup messages (pure data to stdout, silencing stderr is user's responsibility with `2>/dev/null`)

---

## Competitor / Reference Analysis

| Feature | redis-cli | sqlite3 | ripgrep | HTTPie | Mnemonic v1.3 |
|---------|-----------|---------|---------|--------|----------------|
| Default output format | Human (REPL) | Pipe-separated | Colored matches | Pretty-printed | Human tabular |
| Machine-readable flag | None (REPL mode) | `.mode json` | `--json` | Auto (TTY detect) | `--json` |
| Stdin for data | Via pipe to REPL | Via pipe to REPL | Query via stdin | Request body via pipe | `remember` reads stdin |
| Exit codes | 0/1 | 0/1 | 0/1 (found/not found) | 0/1 | 0/1 |
| Error stream | stdout (REPL) | stdout (REPL) | stderr | stderr | stderr |
| Subcommand model | No (REPL) | No (REPL) | No (single purpose) | No (single purpose) | Yes (`clap` derive) |
| Slow startup warning | N/A | N/A | N/A | N/A | stderr "loading..." |

---

## Sources

- [Heroku CLI Style Guide](https://devcenter.heroku.com/articles/cli-style-guide) — stdout/stderr split, `--json` flag, table format, flag-vs-positional conventions. HIGH confidence.
- [HTTPie Redirected Output docs](https://httpie.io/docs/cli/redirected-output) — TTY detection for auto pretty/plain mode. HIGH confidence (official docs).
- [ripgrep `--json` output docs](https://learnbyexample.github.io/learn_gnugrep_ripgrep/ripgrep.html) — newline-delimited JSON for streaming composability. MEDIUM confidence.
- [CLI best practices (HackMD)](https://hackmd.io/@arturtamborski/cli-best-practices) — stdout/stderr discipline, exit codes. MEDIUM confidence.
- [Rust CLI patterns 2026 (dasroot.net)](https://dasroot.net/posts/2026/02/rust-cli-patterns-clap-cargo-configuration/) — clap derive subcommand patterns. MEDIUM confidence.
- [Cloudflare workers-sdk discussion: stderr for logs](https://github.com/cloudflare/workers-sdk/discussions/2940) — rationale for reserving stdout for machine-readable output. HIGH confidence.
- Mnemonic `src/cli.rs` — existing keys CLI conventions (stdout/stderr split, table format, fast path without model load). HIGH confidence (primary source).
- Mnemonic `src/service.rs` — existing `SearchParams`, `ListParams`, `Memory`, `SearchResultItem` structs. HIGH confidence (primary source).
- Mnemonic `src/main.rs` — existing dispatch pattern, model init sequence, fast path for keys. HIGH confidence (primary source).
- [14 tips for amazing CLIs (DEV Community)](https://dev.to/wesen/14-great-tips-to-make-amazing-cli-applications-3gp3) — input/output design patterns. MEDIUM confidence.

---
*Feature research for: Mnemonic v1.3 CLI subcommands (serve, remember, recall, search, compact)*
*Researched: 2026-03-21*
