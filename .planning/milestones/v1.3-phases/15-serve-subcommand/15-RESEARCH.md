# Phase 15: serve subcommand + CLI scaffolding - Research

**Researched:** 2026-03-21
**Domain:** Rust / clap v4 enum expansion + main.rs dispatch refactor
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Add `Serve` variant to `Commands` enum in `cli.rs` — no args struct needed (serve takes no CLI-specific flags beyond the existing global `--db`)
- **D-02:** `Serve` variant has no subcommand-specific flags — port/host/config are already handled by `config::load_config()` (env vars + TOML), matching the established config pattern
- **D-03:** Help text for `Serve`: `"Start the HTTP server"` — concise, matches existing `Keys` style (`"Manage API keys"`)
- **D-04:** `None` (no subcommand) routes to the same server init path as `Serve` — both hit identical code
- **D-05:** No deprecation warning on bare `mnemonic` — it's the primary deployment invocation and should remain first-class indefinitely
- **D-06:** Implementation: match on `Some(Commands::Serve) | None` in a single arm in `main.rs`
- **D-07:** Do NOT extract shared init helpers in this phase — the server init code stays inline in main.rs
- **D-08:** Phases 16-19 will extract helpers (DB-only init, DB+embedding init, full init) as each subcommand needs them — premature extraction now would guess at the interface
- **D-09:** The only structural change to main.rs is moving the `if let Some(Commands::Keys(...))` block to a proper `match` on `cli_args.command`, with `Serve | None` as another arm

- **D-10:** `mnemonic --help` lists both `serve` and `keys` in the subcommands section — clap generates this automatically from the enum
- **D-11:** The `about` string stays `"Agent memory server"` — no change needed

### Claude's Discretion

- Match arm ordering in main.rs (Keys first vs Serve first)
- Whether to add a brief comment above the Serve arm
- Test structure and naming

### Deferred Ideas (OUT OF SCOPE)

- `--port` flag on `serve` subcommand — config already handles this; CLI override could be added in v1.4 if requested
- Shared init helpers (DB-only, DB+embedding, full) — Phases 16-19 will extract as needed
- `--daemon` / background mode — explicitly out of scope per REQUIREMENTS.md
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-01 | `mnemonic serve` starts the HTTP server (same behavior as current bare `mnemonic`) | Adding `Serve` variant to `Commands` and routing `Some(Commands::Serve)` to existing server init path (main.rs lines 51-187) |
| CLI-02 | Bare `mnemonic` with no subcommand continues to start the server (backward compat) | `Some(Commands::Serve) \| None` in a single match arm; `Cli.command` is already `Option<Commands>`, so `None` arm is the natural extension |
</phase_requirements>

## Summary

Phase 15 is a minimal, surgical Rust refactoring. The existing `Commands` enum in `cli.rs` has a single `Keys(KeysArgs)` variant. The task is to add a `Serve` variant (with no args struct) and convert the `if let Some(Commands::Keys(...))` dispatch in `main.rs` into a proper `match` that routes `Some(Commands::Serve) | None` to the already-existing server init block.

All dispatch logic already exists and is correct. No new libraries are required. The clap `#[derive(Subcommand)]` macro handles help text generation automatically — adding a doc-comment to the `Serve` variant is sufficient to populate `mnemonic --help`. The `Cli.command` field is `Option<Commands>`, which already represents the no-subcommand case; `None` routing to server startup is the current behavior and continues unchanged.

The only integration test risk is that existing tests in `tests/cli_integration.rs` rely on `std::process::Command` invoking the compiled binary. These tests exercise `mnemonic keys ...` and do not start the server, so they will pass without modification. The `tests/integration.rs` tests call library functions directly and are unaffected by the dispatch change.

**Primary recommendation:** Add one enum variant in `cli.rs`, replace one `if let` with a `match` in `main.rs`. Total diff is approximately 15 lines.

## Standard Stack

### Core (already in Cargo.toml — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.x (derive feature) | CLI parsing, subcommand dispatch, help generation | Already used; `#[derive(Subcommand)]` on `Commands` enum is the established project pattern |

### Supporting (unchanged)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| anyhow | 1.x | Error propagation in main() | Already used; no change |
| tokio | 1.x | Async runtime | Already used; no change |

### Alternatives Considered

None applicable — all decisions are locked. The existing clap derive-based approach is the only pattern in this codebase.

**Installation:** No changes to Cargo.toml needed.

## Architecture Patterns

### Recommended Project Structure (unchanged)

```
src/
├── cli.rs          # Commands enum — add Serve variant here
├── main.rs         # Dispatch — convert if let to match here
└── ...             # Everything else: untouched
```

### Pattern 1: clap Subcommand Enum Expansion

**What:** Adding a unit variant to an existing `#[derive(Subcommand)]` enum. A unit variant (no fields, no Args struct) requires only a doc-comment for help text.

**When to use:** When the subcommand takes no additional flags (all configuration comes from global flags or environment).

**Example (before — current state):**
```rust
// src/cli.rs line 22
#[derive(Subcommand)]
pub enum Commands {
    /// Manage API keys
    Keys(KeysArgs),
}
```

**Example (after — Phase 15 target):**
```rust
// src/cli.rs
#[derive(Subcommand)]
pub enum Commands {
    /// Start the HTTP server
    Serve,
    /// Manage API keys
    Keys(KeysArgs),
}
```

No `Args` struct, no `#[command(...)]` attribute needed on `Serve`. The doc-comment `/// Start the HTTP server` is the help text clap will display.

### Pattern 2: match on Option<Commands> with or-pattern

**What:** Replacing `if let Some(Commands::Keys(...))` with a `match` that handles all variants. The `None` arm (bare invocation) and `Some(Commands::Serve)` arm are merged with an or-pattern.

**When to use:** When two distinct inputs (bare invocation and explicit subcommand) must route to the same code path.

**Example (before — current state in main.rs lines 21-49):**
```rust
if let Some(cli::Commands::Keys(keys_args)) = cli_args.command {
    // ... minimal DB-only init
    cli::run_keys(keys_args.subcommand, key_service).await;
    return Ok(());
}

// Server path falls through below
```

**Example (after — Phase 15 target):**
```rust
match cli_args.command {
    Some(cli::Commands::Keys(keys_args)) => {
        // ... identical minimal DB-only init (unchanged)
        cli::run_keys(keys_args.subcommand, key_service).await;
        return Ok(());
    }
    Some(cli::Commands::Serve) | None => {
        // server path — fall through or inline
    }
}
```

The or-pattern `Some(cli::Commands::Serve) | None` is valid Rust syntax and is the most explicit encoding of D-06.

Alternatively, the server path can simply continue after the `match` block with an early return in the `Keys` arm. Both approaches compile. The inline `match` with explicit arms is preferred for readability as the number of subcommands grows in Phases 16-19.

### Anti-Patterns to Avoid

- **Extracting server init into a function in this phase:** D-07 and D-08 explicitly forbid this. The interface for shared init helpers is not yet defined; premature extraction will be wrong and require rework in Phases 16-19.
- **Adding a deprecation warning for bare `mnemonic`:** D-05 forbids this. Bare invocation is first-class and must print no extra output.
- **Adding `--port`, `--host`, or similar flags to `Serve`:** D-02 defers all config overrides to v1.4.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Help text for subcommands | Custom `--help` handler | clap doc-comment on enum variant | clap generates correct, well-formatted help automatically |
| Arg parsing for unit variant | Custom match on `std::env::args()` | `#[derive(Subcommand)]` | Already the project pattern; consistent with `Keys` |

**Key insight:** clap v4 with `#[derive(Subcommand)]` handles everything. A unit variant with a doc-comment is 2 lines of code.

## Common Pitfalls

### Pitfall 1: Forgetting the or-pattern syntax for `None`

**What goes wrong:** Writing two separate arms — `Some(Commands::Serve) => { ... }` and `None => { ... }` — with duplicated server init code in each.

**Why it happens:** Not realizing Rust or-patterns work across `Some`/`None`.

**How to avoid:** Use `Some(cli::Commands::Serve) | None => { ... }` as a single arm per D-06.

**Warning signs:** Any duplication of the server init block is wrong.

### Pitfall 2: match exhaustiveness compiler error

**What goes wrong:** After adding `Serve` to `Commands`, the existing `if let Some(Commands::Keys(keys_args))` falls through to the server path for any command including `Serve`. But once converted to `match`, the compiler requires all variants to be handled.

**Why it happens:** Forgetting that `match cli_args.command` over `Option<Commands>` requires arms for `Some(Commands::Keys(...))`, `Some(Commands::Serve)`, and `None`.

**How to avoid:** The or-pattern `Some(cli::Commands::Serve) | None => { ... }` satisfies the compiler while correctly grouping both server-start inputs.

**Warning signs:** `non-exhaustive patterns` compiler error.

### Pitfall 3: --db override not applied in the Serve arm

**What goes wrong:** The server init path (currently falling through after the `if let`) implicitly has access to `cli_args`. After refactoring to `match`, the `cli_args.db` override must still be applied if present.

**Why it happens:** The current server init block on lines 51-187 never applies `cli_args.db` — it uses `config::load_config()` which reads env vars and TOML only.

**How to avoid:** Inspect whether the current server init block applies the `--db` override. If not, no change needed. If the Keys path applies it but the server path doesn't, add the same `if let Some(db_override) = cli_args.db { config.db_path = db_override; }` pattern to the server arm. This is a correctness check, not a structural concern.

**Warning signs:** Integration test that passes `--db /tmp/test.db serve` and then checks which database was used.

### Pitfall 4: Help output regression

**What goes wrong:** Adding `Serve` variant without a doc-comment causes clap to display an unhelpful `serve` entry with no description in `mnemonic --help`.

**Why it happens:** Omitting the `/// Start the HTTP server` doc-comment.

**How to avoid:** Always add the doc-comment. Per D-03, the exact text is `"Start the HTTP server"`.

## Code Examples

### Adding unit variant to Commands enum

```rust
// src/cli.rs
/// Top-level subcommands.
#[derive(Subcommand)]
pub enum Commands {
    /// Start the HTTP server
    Serve,
    /// Manage API keys
    Keys(KeysArgs),
}
```

Source: Verified against clap v4 docs — unit variants in `#[derive(Subcommand)]` enums are fully supported. Doc-comment becomes the help string displayed by `--help`.

### Converting if let to match with or-pattern

```rust
// src/main.rs — replaces lines 21-49 if let block
match cli_args.command {
    Some(cli::Commands::Keys(keys_args)) => {
        // 1. Register sqlite-vec
        db::register_sqlite_vec();

        // 2. Load config for db_path only
        let mut config = config::load_config()
            .map_err(|e| anyhow::anyhow!(e))?;

        // 3. Apply --db override if provided
        if let Some(db_override) = cli_args.db {
            config.db_path = db_override;
        }

        // 4. Open DB and apply schema
        let conn = db::open(&config).await
            .map_err(|e| anyhow::anyhow!(e))?;
        let conn_arc = std::sync::Arc::new(conn);

        // 5. Construct KeyService
        let key_service = auth::KeyService::new(conn_arc);

        // 6. Run keys subcommand and exit
        cli::run_keys(keys_args.subcommand, key_service).await;
        return Ok(());
    }
    Some(cli::Commands::Serve) | None => {
        // Server path continues below (or fall-through if match ends here)
    }
}

// Server init continues from here (lines 51-187, unchanged)
```

Source: Verified against current `src/main.rs`. The `if let` pattern is lines 21-49; server init is lines 51-187.

### CLI integration test pattern for serve subcommand

```rust
// tests/cli_integration.rs — new test for Phase 15
/// CLI-01: `mnemonic serve` appears in --help output.
#[test]
fn test_serve_appears_in_help() {
    let bin = binary();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "--help must exit 0");
    assert!(
        stdout.contains("serve"),
        "--help must list 'serve' subcommand; got:\n{}",
        stdout
    );
}
```

Source: Modeled on existing `tests/cli_integration.rs` patterns (TempDb helper, `binary()` fn, `std::process::Command`).

Note: Testing that `mnemonic serve` actually starts the server requires a long-running process and is not practical in a synchronous integration test. The help-text test verifies the command is registered. Behavioral correctness (server accepts requests) is verified by the existing `tests/integration.rs` and `tests/error_types.rs` which test the server directly at the library level.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `if let Some(Commands::Keys(...))` early return | `match cli_args.command { ... }` with all arms | Phase 15 | More variants can be added cleanly in Phases 16-19 |

**No deprecated patterns applicable to this phase.**

## Open Questions

1. **Does the server init block apply the `--db` override?**
   - What we know: The Keys arm (lines 28-34 of main.rs) applies `cli_args.db` override via `config.db_path = db_override`. The server init block (lines 59-61) calls `config::load_config()` but never reads `cli_args.db`.
   - What's unclear: Whether `--db /path/to/db mnemonic serve` should be a supported usage pattern. If yes, the server arm must apply the override. If the override is intentionally keys-only, no action needed.
   - Recommendation: Apply the override in the server arm for consistency — `--db` is a global flag (`global = true` on line 13 of cli.rs) and users will reasonably expect it to work with `serve`. This is a one-line addition. The implementer should confirm this interpretation before coding.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness + cargo |
| Config file | none (Cargo.toml `[dev-dependencies]`) |
| Quick run command | `cargo test --test cli_integration 2>&1 \| tail -20` |
| Full suite command | `cargo test 2>&1 \| tail -30` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-01 | `mnemonic serve` starts HTTP server | smoke (binary invocation) | `cargo test --test cli_integration test_serve_appears_in_help` | No — Wave 0 gap |
| CLI-01 | `mnemonic serve` listed in `--help` | integration | `cargo test --test cli_integration test_serve_appears_in_help` | No — Wave 0 gap |
| CLI-02 | Bare `mnemonic` (no args) still dispatches to server | compile-time + manual | n/a — existing server integration tests cover server behavior | Yes (integration.rs) |
| CLI-02 | Existing `mnemonic keys ...` tests pass unchanged | regression | `cargo test --test cli_integration` | Yes (cli_integration.rs) |

### Sampling Rate

- **Per task commit:** `cargo test --test cli_integration 2>&1 | tail -20`
- **Per wave merge:** `cargo test 2>&1 | tail -30`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] Add `test_serve_appears_in_help` to `tests/cli_integration.rs` — covers CLI-01 (verifies `serve` is registered as a subcommand and appears in `--help`)

*(No new test files or framework config needed — existing `tests/cli_integration.rs` infrastructure is reusable)*

## Sources

### Primary (HIGH confidence)

- `src/cli.rs` (read directly) — exact current `Commands` enum, `Cli` struct, `KeysArgs` pattern
- `src/main.rs` (read directly) — exact current dispatch logic, server init block line ranges
- `tests/cli_integration.rs` (read directly) — exact test patterns, `binary()` helper, `TempDb` struct
- `Cargo.toml` (read directly) — clap 4.x with derive feature confirmed, zero new deps needed
- `.planning/phases/15-serve-subcommand/15-CONTEXT.md` (read directly) — all implementation decisions locked

### Secondary (MEDIUM confidence)

- clap v4 documentation (knowledge) — `#[derive(Subcommand)]` unit variants and doc-comment help text behavior; verified consistent with existing `Keys(KeysArgs)` pattern in the codebase

### Tertiary (LOW confidence)

- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new dependencies; everything sourced from existing Cargo.toml
- Architecture: HIGH — exact current code read; pattern is a direct extension of existing `Keys` variant
- Pitfalls: HIGH — identified from reading the actual dispatch code and test infrastructure
- Test map: HIGH — test patterns read directly from `tests/cli_integration.rs`

**Research date:** 2026-03-21
**Valid until:** Stable — Rust/clap 4.x patterns are stable; expires only if clap major version changes
