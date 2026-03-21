# Phase 15: serve subcommand + CLI scaffolding - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Expand the `Commands` enum to include a `Serve` variant. Wire `mnemonic serve` as an explicit subcommand that starts the HTTP server. Preserve bare `mnemonic` (no subcommand) as backward-compatible server start. All existing behavior and tests must remain unchanged.

</domain>

<decisions>
## Implementation Decisions

### Commands enum expansion
- **D-01:** Add `Serve` variant to `Commands` enum in `cli.rs` — no args struct needed (serve takes no CLI-specific flags beyond the existing global `--db`)
- **D-02:** `Serve` variant has no subcommand-specific flags — port/host/config are already handled by `config::load_config()` (env vars + TOML), matching the established config pattern
- **D-03:** Help text for `Serve`: `"Start the HTTP server"` — concise, matches existing `Keys` style (`"Manage API keys"`)

### Backward compatibility dispatch
- **D-04:** `None` (no subcommand) routes to the same server init path as `Serve` — both hit identical code
- **D-05:** No deprecation warning on bare `mnemonic` — it's the primary deployment invocation and should remain first-class indefinitely
- **D-06:** Implementation: match on `Some(Commands::Serve) | None` in a single arm in `main.rs`

### main.rs refactoring scope
- **D-07:** Do NOT extract shared init helpers in this phase — the server init code stays inline in main.rs
- **D-08:** Phases 16-19 will extract helpers (DB-only init, DB+embedding init, full init) as each subcommand needs them — premature extraction now would guess at the interface
- **D-09:** The only structural change to main.rs is moving the `if let Some(Commands::Keys(...))` block to a proper `match` on `cli_args.command`, with `Serve | None` as another arm

### Help output
- **D-10:** `mnemonic --help` lists both `serve` and `keys` in the subcommands section — clap generates this automatically from the enum
- **D-11:** The `about` string stays `"Agent memory server"` — no change needed

### Claude's Discretion
- Match arm ordering in main.rs (Keys first vs Serve first)
- Whether to add a brief comment above the Serve arm
- Test structure and naming

</decisions>

<specifics>
## Specific Ideas

- The dispatch pattern should feel like a natural expansion of the existing `if let Some(Commands::Keys(...))` → `match` conversion — not a rewrite
- STATE.md already notes "v1.3 adds 5 new branches + shared init helper" — Phase 15 adds the first branch (Serve) but defers the shared init helper to later phases

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### CLI structure
- `src/cli.rs` — Current `Commands` enum (line 22), `Cli` struct (line 11), `KeysArgs` pattern (line 29)
- `src/main.rs` — Current dispatch logic (lines 21-49 for Keys, lines 51-187 for server init)

### Requirements
- `.planning/REQUIREMENTS.md` §CLI Scaffolding — CLI-01 (`mnemonic serve` starts server), CLI-02 (bare `mnemonic` backward compat)

### Prior patterns
- `.planning/PROJECT.md` §Key Decisions — `route_layer()` auth middleware, per-request COUNT for open mode

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cli::Cli` struct with `Option<Commands>` — already handles None case, just needs another variant
- `cli::Commands` enum — add `Serve` variant, clap derives help automatically
- `config::load_config()` + `config::validate_config()` — server init already uses these, no changes needed

### Established Patterns
- `if let Some(Commands::Keys(keys_args))` early return pattern in main.rs — will become a proper `match`
- Global `--db` flag on `Cli` struct — applies to all subcommands including `serve`
- Fast-path pattern: Keys does minimal init (DB only); Serve does full init (DB + embedding + LLM + server)

### Integration Points
- `main.rs` lines 21-49: Keys dispatch (will become one match arm)
- `main.rs` lines 51-187: Server init (will become another match arm, code stays inline)
- `cli.rs` line 22: `Commands` enum (add `Serve` variant)

</code_context>

<deferred>
## Deferred Ideas

- `--port` flag on `serve` subcommand — config already handles this; CLI override could be added in v1.4 if requested
- Shared init helpers (DB-only, DB+embedding, full) — Phases 16-19 will extract as needed
- `--daemon` / background mode — explicitly out of scope per REQUIREMENTS.md

</deferred>

---

*Phase: 15-serve-subcommand*
*Context gathered: 2026-03-21*
