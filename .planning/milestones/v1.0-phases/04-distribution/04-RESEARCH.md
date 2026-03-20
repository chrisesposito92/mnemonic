# Phase 04: Distribution - Research

**Researched:** 2026-03-19
**Domain:** Rust binary distribution, GitHub Actions CI/CD, README documentation authoring
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**README structure**
- Single README.md file with linked table of contents at the top
- Section order: intro/tagline, quickstart, concepts, configuration, API reference, usage examples, how it works, contributing
- Quickstart is the first thing after the intro — download, run, store a memory in 3 commands or fewer
- API reference is inline with curl request + JSON response examples for every endpoint (no external docs site)
- Dedicated Configuration section with a table listing all MNEMONIC_* env vars, their defaults, and descriptions
- Brief "Concepts" section before API reference explaining agent_id, session_id, and tags

**Binary distribution**
- Two distribution methods: `cargo install mnemonic` for Rust users, prebuilt binaries via GitHub Releases for everyone else
- Prebuilt binaries for: Linux x86_64, macOS x86_64 (Intel), macOS aarch64 (Apple Silicon)
- GitHub Actions CI workflow that builds release binaries on tag push (cross-compile matrix)
- No Docker image in v1 — mention as future work (conflicts with single-binary philosophy)
- Quickstart shows both methods: cargo install and direct binary download

**Agent framework examples**
- Python examples using `requests` library only — no framework dependency, shows REST API directly
- Simple `MnemonicClient` helper class wrapping requests for store, search, list, delete
- Multi-agent example showing agent_id namespacing (two agents sharing one instance)
- One AI tool-use example showing how to define mnemonic as a tool for an LLM agent (conceptual, framework-agnostic)

**Example depth**
- Curl examples use realistic data: agent names like "research-bot", real-looking memory content
- One error response example per endpoint in the API reference
- Brief "How it works" paragraph mentioning SQLite + sqlite-vec + candle without deep architecture detail
- Response examples show full JSON bodies so users know the exact shape

### Claude's Discretion
- Exact GitHub Actions workflow configuration and matrix details
- README formatting choices (badges, shields, etc.)
- Curl example content specifics
- Python client class internal structure
- Whether to include a CHANGELOG.md

### Deferred Ideas (OUT OF SCOPE)
- Docker image distribution — future milestone
- Interactive API playground / Swagger UI — out of scope (conflicts with single-binary)
- CHANGELOG.md — can be added when there are multiple releases
- Homebrew formula — future distribution method
- Windows prebuilt binary — add when there's demand
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DOCS-01 | README includes quickstart guide that gets a user from download to first stored memory in under 3 commands | Binary distribution patterns, `cargo install` metadata requirements, curl one-liner POST /memories |
| DOCS-02 | README includes full API reference with request/response examples for every endpoint | Complete API surface extracted from src/service.rs, src/server.rs, src/error.rs — all types and status codes verified |
| DOCS-03 | README includes usage examples for curl, Python, and at least one agent framework | Python `requests` pattern, framework-agnostic tool-use example pattern |
</phase_requirements>

---

## Summary

This phase produces two deliverables: (1) a polished README.md that serves as the sole documentation artifact, and (2) a GitHub Actions release workflow that cross-compiles prebuilt binaries for Linux x86_64, macOS x86_64, and macOS aarch64 on tag push. The implementation itself is complete — Phase 4 is purely documentation and distribution packaging.

The API surface is fully extracted from source: five endpoints (`POST /memories`, `GET /memories/search`, `GET /memories`, `DELETE /memories/:id`, `GET /health`) with known request shapes, response shapes, status codes, and error format. The Config struct has four fields with the `MNEMONIC_` env var prefix. These are the ground-truth facts the README must document — no inference needed.

The GitHub Actions workflow pattern is well-established: a matrix of three targets (`x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`) built natively (no cross-compilation tool required for these three), binaries staged as `.tar.gz` artifacts, and `softprops/action-gh-release@v2` to publish them on tag push. Before `cargo install mnemonic` works, `Cargo.toml` needs `description`, `license`, `repository`, and `homepage` metadata fields filled in.

**Primary recommendation:** Write README.md directly from the source-verified API surface below; write the GitHub Actions workflow using native cargo builds for all three targets (no `cross` tool needed) with `softprops/action-gh-release@v2`.

---

## Standard Stack

### Core (pre-existing — no new dependencies)

| Library/Tool | Version | Purpose | Why Standard |
|---|---|---|---|
| Rust + cargo | stable | Build toolchain | Required — project is Rust |
| GitHub Actions | — | CI/CD for release builds | Standard for open-source Rust projects |
| softprops/action-gh-release | v2 | Upload binaries to GitHub Releases | Most widely-used release action for Rust projects |
| actions/upload-artifact | v4 | Pass binaries from build jobs to release job | Standard artifact handoff pattern |
| actions/checkout | v4 | Checkout code in CI | Current standard version |
| dtolnay/rust-toolchain | stable | Install Rust in CI | Preferred over `actions-rs/toolchain` (deprecated) |

### Supporting (documentation tools — no installation required)

| Tool | Purpose | Notes |
|---|---|---|
| shields.io | README badges (build status, license, crates.io version) | Optional but conventional for open-source; at Claude's discretion |
| Python `requests` | Example client in README | No install needed — standard library available everywhere |

### No New Rust Dependencies

This phase adds zero new Cargo dependencies. It adds one new file (`README.md` rewrite) and one new directory (`.github/workflows/release.yml`). The only `Cargo.toml` changes are metadata fields in `[package]`.

---

## Architecture Patterns

### Pattern 1: GitHub Actions Release Workflow (Matrix Build)

**What:** A workflow triggered on `push` to tags matching `v*` builds the binary for each target platform in parallel, uploads each binary as an artifact, then a final release job collects all artifacts and publishes them to GitHub Releases.

**When to use:** Any Rust project distributing prebuilt binaries.

**Key insight for this project:** All three required targets (`x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`) can be built natively without the `cross` tool. Linux x86 runs on `ubuntu-latest`. Both macOS targets run on `macos-latest` (Apple Silicon runners support cross-compiling to Intel via `--target x86_64-apple-darwin` after adding the target).

**Example workflow structure (Claude's discretion for exact YAML):**
```yaml
# Source: Cross-Platform Rust CI/CD patterns, GitHub Actions docs
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    name: Build ${{ matrix.name }}
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - name: linux-x86_64
            os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: mnemonic-linux-x86_64
          - name: macos-x86_64
            os: macos-latest
            target: x86_64-apple-darwin
            artifact: mnemonic-macos-x86_64
          - name: macos-aarch64
            os: macos-latest
            target: aarch64-apple-darwin
            artifact: mnemonic-macos-aarch64
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }}
      - name: Stage binary
        run: |
          mkdir -p dist
          cp target/${{ matrix.target }}/release/mnemonic dist/${{ matrix.artifact }}
          tar -czf dist/${{ matrix.artifact }}.tar.gz -C dist ${{ matrix.artifact }}
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact }}
          path: dist/${{ matrix.artifact }}.tar.gz

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
      - uses: softprops/action-gh-release@v2
        with:
          files: '**/*.tar.gz'
```

**Confidence:** MEDIUM — exact YAML syntax verified against documented patterns from multiple 2024-2025 sources; specific action versions confirmed current.

### Pattern 2: README Structure with Table of Contents

**What:** A single-file README with anchored section headers and a ToC linking to each. The ToC anchor format is `#section-name` (lowercase, spaces to hyphens, punctuation stripped).

**Decided section order (locked by CONTEXT.md):**
1. Intro/tagline
2. Quickstart (3 commands)
3. Concepts (agent_id, session_id, tags)
4. Configuration (env vars table)
5. API Reference (inline, per endpoint)
6. Usage Examples (curl, Python, agent tool-use)
7. How It Works
8. Contributing

### Pattern 3: Cargo.toml Metadata for `cargo install`

**What:** For `cargo install mnemonic` to work cleanly (and for eventual crates.io publishing), the `[package]` section needs additional metadata fields beyond the current `name`, `version`, `edition`.

**Required additions to Cargo.toml:**
```toml
[package]
name = "mnemonic"
version = "0.1.0"
edition = "2021"
description = "Framework-agnostic agent memory server — persistent semantic memory via a simple REST API"
license = "MIT"            # or Apache-2.0 — Claude's discretion
repository = "https://github.com/chrisesposito/mnemonic"  # confirm exact URL
homepage = "https://github.com/chrisesposito/mnemonic"
keywords = ["agent", "memory", "embeddings", "semantic-search", "llm"]
categories = ["web-programming::http-server", "database"]
```

`cargo install mnemonic` works without these fields (only `name` is required). These fields are required to publish to crates.io and are conventional for public crates.

**Confidence:** HIGH — verified against The Cargo Book (doc.rust-lang.org).

### Anti-Patterns to Avoid

- **Documenting internal implementation details in the quickstart:** Users need 3 commands, not an explanation of the embedding pipeline. Keep the quickstart minimal and move "How It Works" to the bottom.
- **Using `actions-rs/toolchain`:** This action is unmaintained. Use `dtolnay/rust-toolchain@stable` instead.
- **Using `cross` tool for these targets:** All three targets build natively. `cross` adds complexity and Docker dependency; unnecessary here.
- **Nested `cargo build` in release job:** Build only in the matrix job; the release job only downloads artifacts and publishes.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| GitHub Release creation | Custom API calls via curl | `softprops/action-gh-release@v2` | Handles create-or-update, tag detection, permissions, asset uploads in one step |
| Rust toolchain setup in CI | Manual rustup commands | `dtolnay/rust-toolchain@stable` | Correct caching, target management, minimal config |
| Artifact handoff between CI jobs | Custom S3/artifact storage | `actions/upload-artifact@v4` + `download-artifact@v4` | Built-in GitHub Actions artifact system, free, simple |

---

## Complete API Surface (Source of Truth for Documentation)

All data extracted directly from `src/service.rs`, `src/server.rs`, `src/error.rs`. This is what the README API reference MUST document.

### Endpoint Inventory

| Method | Path | Handler | Success Status | Purpose |
|---|---|---|---|---|
| GET | /health | `health_handler` | 200 | Server readiness check |
| POST | /memories | `create_memory_handler` | 201 | Create and embed a memory |
| GET | /memories/search | `search_memories_handler` | 200 | Semantic KNN search |
| GET | /memories | `list_memories_handler` | 200 | Paginated structured list |
| DELETE | /memories/:id | `delete_memory_handler` | 200 | Delete memory by ID |

### Error Status Codes

| Status | When | Body |
|---|---|---|
| 400 | Validation failure (empty content, missing q) | `{"error": "message"}` |
| 404 | Memory ID not found on DELETE | `{"error": "not found"}` |
| 500 | Internal server error | `{"error": "internal server error"}` |

### POST /memories

**Request body (JSON):**
```json
{
  "content": "string (required, non-empty)",
  "agent_id": "string (optional, default empty string)",
  "session_id": "string (optional, default empty string)",
  "tags": ["string"]
}
```

**Response 201 (Memory object):**
```json
{
  "id": "019xxx-uuid-v7-string",
  "content": "string",
  "agent_id": "string",
  "session_id": "string",
  "tags": ["string"],
  "embedding_model": "all-MiniLM-L6-v2",
  "created_at": "2026-03-19 12:34:56",
  "updated_at": null
}
```

**Error 400:**
```json
{"error": "content must not be empty"}
```

### GET /memories/search

**Query parameters:**
| Param | Type | Required | Default | Description |
|---|---|---|---|---|
| q | string | YES | — | Search query text |
| agent_id | string | no | — | Filter to this agent's memories |
| session_id | string | no | — | Filter to this session's memories |
| tag | string | no | — | Filter by tag (substring match) |
| limit | u32 | no | 10 | Max results (capped at 100) |
| threshold | f32 | no | — | Max distance filter (lower = more similar) |
| after | string | no | — | ISO datetime lower bound for created_at |
| before | string | no | — | ISO datetime upper bound for created_at |

**Response 200:**
```json
{
  "memories": [
    {
      "id": "string",
      "content": "string",
      "agent_id": "string",
      "session_id": "string",
      "tags": ["string"],
      "embedding_model": "string",
      "created_at": "string",
      "updated_at": null,
      "distance": 0.123
    }
  ]
}
```

**Error 400 (missing q):**
```json
{"error": "q parameter is required"}
```

### GET /memories

**Query parameters:**
| Param | Type | Required | Default | Description |
|---|---|---|---|---|
| agent_id | string | no | — | Filter to this agent |
| session_id | string | no | — | Filter to this session |
| tag | string | no | — | Filter by tag (substring match) |
| after | string | no | — | ISO datetime lower bound |
| before | string | no | — | ISO datetime upper bound |
| limit | u32 | no | 20 | Max results (capped at 100) |
| offset | u32 | no | 0 | Pagination offset |

**Response 200:**
```json
{
  "memories": [ /* array of Memory objects (no distance field) */ ],
  "total": 42
}
```

### DELETE /memories/:id

**Path param:** `id` — UUID string

**Response 200:** Full Memory object of the deleted memory (same shape as POST 201 response)

**Error 404:**
```json
{"error": "not found"}
```

### GET /health

**Response 200:**
```json
{"status": "ok"}
```

---

## Configuration Reference (Source of Truth)

Extracted directly from `src/config.rs`. All four fields map to `MNEMONIC_` prefixed env vars. TOML file path is set via `MNEMONIC_CONFIG_PATH`.

| Env Var | Cargo.toml Field | Default | Description |
|---|---|---|---|
| `MNEMONIC_PORT` | `port` | `8080` | TCP port to listen on |
| `MNEMONIC_DB_PATH` | `db_path` | `./mnemonic.db` | Path to SQLite database file |
| `MNEMONIC_EMBEDDING_PROVIDER` | `embedding_provider` | `local` | `local` (bundled model) or `openai` |
| `MNEMONIC_OPENAI_API_KEY` | `openai_api_key` | — | OpenAI API key; switches provider to `openai` when set |
| `MNEMONIC_CONFIG_PATH` | — | `./mnemonic.toml` | Path to optional TOML config file |

**Precedence (highest to lowest):** env vars → TOML file → compiled-in defaults.

---

## Common Pitfalls

### Pitfall 1: Quickstart exceeds 3 commands
**What goes wrong:** Adding env var setup or config explanation in the quickstart blooms it beyond 3 commands, violating DOCS-01.
**Why it happens:** Writers want to be thorough.
**How to avoid:** Quickstart is exactly: (1) download/install binary, (2) start server, (3) `curl POST /memories`. All other context moves to the Concepts and Configuration sections.
**Warning signs:** If you find yourself adding `export MNEMONIC_...` to the quickstart, move it to Configuration.

### Pitfall 2: Missing `updated_at` in documentation
**What goes wrong:** Documentation shows `updated_at` as always `null` and users are confused.
**Why it happens:** The field exists in the schema but Phase 3 has no UPDATE endpoint yet.
**How to avoid:** Document `updated_at` as `null | string` in the API reference, note it is reserved for future use (v2 EAPI-01).

### Pitfall 3: Documenting `distance` as similarity score
**What goes wrong:** Users interpret lower distance as "worse" match.
**Why it happens:** KNN returns L2/cosine distance, not similarity — lower is MORE similar.
**How to avoid:** Explicitly state in the search endpoint docs: "lower distance = more similar; results ordered from most to least similar."

### Pitfall 4: Stale `tag` filter behavior documented incorrectly
**What goes wrong:** User expects exact match, but implementation uses `LIKE '%tag%'` substring match.
**Why it happens:** Documentation says "filter by tag" without specifying match semantics.
**How to avoid:** Document the `tag` query param as: "substring match against the tags array" (verified in `src/service.rs` lines 193, 254).

### Pitfall 5: `cargo install` without Cargo.toml metadata
**What goes wrong:** `cargo install mnemonic` fails because the package isn't published to crates.io, or publishes with missing required registry fields.
**Why it happens:** Current Cargo.toml only has `name`, `version`, `edition`.
**How to avoid:** Add `description`, `license`, `repository` to Cargo.toml before claiming `cargo install mnemonic` works in the README. Until the crate is published, document `cargo install --git https://github.com/...` as the alternative.

### Pitfall 6: GitHub Actions macOS Intel build fails on ARM runner
**What goes wrong:** `cargo build --release` on `macos-latest` for `x86_64-apple-darwin` fails without adding the target.
**Why it happens:** The default toolchain on macOS runners only includes the host target.
**How to avoid:** Use `dtolnay/rust-toolchain@stable` with `targets: ${{ matrix.target }}` to ensure the cross-target is installed before building.

---

## Code Examples

### Quickstart — 3-Command Flow
```bash
# Command 1: Install
curl -L https://github.com/USER/mnemonic/releases/latest/download/mnemonic-linux-x86_64.tar.gz | tar xz
# OR: cargo install mnemonic --git https://github.com/USER/mnemonic

# Command 2: Start
./mnemonic

# Command 3: Store a memory
curl -s -X POST http://localhost:8080/memories \
  -H "Content-Type: application/json" \
  -d '{"content": "The user prefers dark mode", "agent_id": "my-agent"}'
```

### Python MnemonicClient Pattern
```python
# Source: CONTEXT.md decision — requests-only, no framework dependency
import requests

class MnemonicClient:
    def __init__(self, base_url="http://localhost:8080"):
        self.base_url = base_url

    def store(self, content, agent_id=None, session_id=None, tags=None):
        payload = {"content": content}
        if agent_id:    payload["agent_id"] = agent_id
        if session_id:  payload["session_id"] = session_id
        if tags:        payload["tags"] = tags
        r = requests.post(f"{self.base_url}/memories", json=payload)
        r.raise_for_status()
        return r.json()

    def search(self, query, agent_id=None, limit=10):
        params = {"q": query, "limit": limit}
        if agent_id: params["agent_id"] = agent_id
        r = requests.get(f"{self.base_url}/memories/search", params=params)
        r.raise_for_status()
        return r.json()["memories"]

    def list(self, agent_id=None, limit=20, offset=0):
        params = {"limit": limit, "offset": offset}
        if agent_id: params["agent_id"] = agent_id
        r = requests.get(f"{self.base_url}/memories", params=params)
        r.raise_for_status()
        return r.json()

    def delete(self, memory_id):
        r = requests.delete(f"{self.base_url}/memories/{memory_id}")
        r.raise_for_status()
        return r.json()
```

### Multi-Agent Example (two agents, one instance)
```python
# Source: CONTEXT.md decision — agent_id namespacing pattern
client = MnemonicClient()

# Each agent uses its own agent_id namespace
research_bot = lambda content: client.store(content, agent_id="research-bot")
summarizer   = lambda content: client.store(content, agent_id="summarizer")

research_bot("The Eiffel Tower is 330 meters tall")
summarizer("Previous summary covered Paris landmarks")

# Search is scoped to each agent's memories
results = client.search("tall structures", agent_id="research-bot")
# → returns only research-bot memories
```

### Framework-Agnostic Tool-Use Definition
```python
# Source: CONTEXT.md decision — conceptual, framework-agnostic
# Define mnemonic operations as LLM tool schemas

MNEMONIC_TOOLS = [
    {
        "name": "store_memory",
        "description": "Store a piece of information for later retrieval",
        "parameters": {
            "type": "object",
            "properties": {
                "content": {"type": "string", "description": "The information to remember"},
                "tags": {"type": "array", "items": {"type": "string"}, "description": "Optional labels"}
            },
            "required": ["content"]
        }
    },
    {
        "name": "search_memory",
        "description": "Search stored memories by semantic similarity",
        "parameters": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "What to search for"}
            },
            "required": ["query"]
        }
    }
]

# Tool execution dispatch
def handle_tool_call(tool_name, args, agent_id):
    if tool_name == "store_memory":
        return client.store(args["content"], agent_id=agent_id, tags=args.get("tags"))
    elif tool_name == "search_memory":
        return client.search(args["query"], agent_id=agent_id)
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| `actions-rs/toolchain` | `dtolnay/rust-toolchain@stable` | ~2023 | `actions-rs` is unmaintained; dtolnay is the recommended replacement |
| `actions/upload-release-asset` | `softprops/action-gh-release@v2` | ~2022 | Simpler API, handles create-or-update, better glob support |
| `actions/upload-artifact@v3` | `actions/upload-artifact@v4` | 2024 | v3 deprecated March 2025; v4 is current |

**Deprecated/outdated:**
- `actions-rs/toolchain`: unmaintained, superseded by `dtolnay/rust-toolchain`
- `actions/upload-artifact@v3`: GitHub deprecated March 2025

---

## Validation Architecture

`nyquist_validation` is enabled in `.planning/config.json`.

### Test Framework

| Property | Value |
|---|---|
| Framework | Rust built-in test harness (`cargo test`) |
| Config file | None — standard `#[tokio::test]` annotations |
| Quick run command | `cargo test --test integration -- --test-threads=1` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| DOCS-01 | Binary starts and responds to POST /memories | smoke | `cargo test --test integration test_health test_post_memory` | ✅ existing |
| DOCS-02 | All endpoints documented match actual behavior | manual | Review README against source types | N/A (documentation) |
| DOCS-03 | curl/Python examples are copy-paste executable | manual | Run examples against live server | N/A (documentation) |

**Note:** DOCS-01, DOCS-02, and DOCS-03 are documentation requirements. The behavior they document is already tested by existing integration tests in `tests/integration.rs` (21 tests, all passing from Phase 3). No new test files are required for this phase. The validation for this phase is human review: does the README quickstart work in 3 commands? Does the API reference match the source types?

### Sampling Rate
- **Per task commit:** `cargo test --test integration -- --test-threads=1` (verifies server still works)
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green + human spot-check of quickstart commands before `/gsd:verify-work`

### Wave 0 Gaps
None — existing test infrastructure covers all phase requirements. This phase adds no new code that requires new tests.

---

## Open Questions

1. **Repository URL for Cargo.toml and README download links**
   - What we know: The repo is at `github.com/chrisesposito/mnemonic` based on project structure
   - What's unclear: The exact GitHub username/org in the public URL (need to verify before writing download curl commands)
   - Recommendation: Implementer should verify with `git remote -v` and use the real URL

2. **License choice**
   - What we know: Cargo.toml currently has no `license` field
   - What's unclear: MIT vs. Apache-2.0 vs. MIT/Apache-2.0 dual license (CONTEXT.md marks this as Claude's discretion)
   - Recommendation: MIT is the most common choice for Rust CLI tools; dual MIT/Apache-2.0 is idiomatic for Rust libraries. For a binary server tool, MIT alone is simplest.

3. **crates.io publishing vs. git install**
   - What we know: `cargo install mnemonic` only works if the crate is published to crates.io
   - What's unclear: Is the intent to publish to crates.io as part of this phase?
   - Recommendation: Document `cargo install --git https://github.com/USER/mnemonic` in v1 quickstart; note that `cargo install mnemonic` will work after the first crates.io publish. This avoids blocking the README on a registry publish step.

---

## Sources

### Primary (HIGH confidence)
- `src/service.rs` — All request/response types extracted verbatim (CreateMemoryRequest, SearchParams, ListParams, Memory, SearchResponse, SearchResultItem, ListResponse)
- `src/server.rs` — Route definitions, status codes, handler signatures
- `src/error.rs` — ApiError with IntoResponse, all error status code mappings
- `src/config.rs` — Config struct, all four fields, defaults, MNEMONIC_ prefix, precedence rules
- `src/main.rs` — Startup log format, embedding provider selection, embedding_model string values
- [The Cargo Book — Manifest Format](https://doc.rust-lang.org/cargo/reference/manifest.html) — Cargo.toml metadata fields for publishing

### Secondary (MEDIUM confidence)
- [Rust Cross-Compilation GitHub Actions (reemus.dev)](https://reemus.dev/tldr/rust-cross-compilation-github-actions) — Matrix strategy pattern for three targets
- [Cross-Platform Rust Pipeline 2025 (ahmedjama.com)](https://ahmedjama.com/blog/2025/12/cross-platform-rust-pipeline-github-actions/) — `softprops/action-gh-release@v2` + `actions/download-artifact@v4` release job pattern
- [softprops/action-gh-release GitHub repo](https://github.com/softprops/action-gh-release) — v2 confirmed current

### Tertiary (LOW confidence)
- None — all claims supported by primary or secondary sources.

---

## Metadata

**Confidence breakdown:**
- API surface documentation: HIGH — extracted directly from source code, no inference
- Configuration reference: HIGH — extracted directly from src/config.rs
- GitHub Actions workflow: MEDIUM — pattern verified against multiple 2024-2025 sources; exact YAML syntax needs CI run to confirm
- Architecture/patterns: HIGH — standard Rust release workflow patterns

**Research date:** 2026-03-19
**Valid until:** 2026-06-19 (GitHub Actions action versions; re-verify if >90 days old)
