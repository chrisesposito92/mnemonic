# Stack Research

**Domain:** Rust agent memory server (embedded vector search, local ML inference, REST API)
**Researched:** 2026-03-19
**Confidence:** HIGH (all versions verified against official sources)

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| tokio | 1.50.0 | Async runtime | The de facto async runtime for Rust. axum, tokio-rusqlite, and the entire ecosystem assume tokio. No real alternative in this stack. |
| axum | 0.8.8 | HTTP REST API layer | Modern, ergonomic, Tower-native. Thinner than actix-web and better DX than warp. tokio-rs maintains it alongside tokio — deep integration, active 0.9 in progress. The ecosystem default for new Rust HTTP services. |
| rusqlite | 0.38.0 | SQLite access (sync layer) | The canonical Rust SQLite binding. The `bundled` feature compiles SQLite 3.51.1 directly into the binary — zero system dependency. Required by tokio-rusqlite under the hood. |
| tokio-rusqlite | 0.7.0 | Async SQLite wrapper | Prevents rusqlite's blocking calls from starving the tokio threadpool. Spawns a dedicated thread per connection, proxies calls via mpsc/oneshot channels. Required for correct async behavior under concurrent agent requests. |
| sqlite-vec | 0.1.7 | SQLite vector extension | Actively maintained (Mozilla Builders project, last release March 17 2026). sqlite-vss, the predecessor, is archived. KNN search, `vec0` virtual tables, compact BLOB storage. Zero external dependencies — pure C, loads as a SQLite extension at runtime. |
| candle-core + candle-nn + candle-transformers | 0.9.2 | Local ML inference | Pure Rust inference. No ONNX Runtime, no Python, no system libraries. The only way to bundle a real neural network model into a single Rust binary. Maintained by HuggingFace. inference-first, not training. |
| tokenizers | 0.22.2 | HuggingFace tokenization | Official HuggingFace Rust tokenizer library. Required alongside candle for sentence-transformer models. Handles WordPiece, BPE, tokenization pipeline including special tokens. |
| hf-hub | latest (0.x) | Model download / caching | Official HuggingFace Rust client. Downloads safetensors weights from HF Hub on first run, caches at `~/.cache/huggingface/`. Enables zero-bundle binary — model weights loaded at startup, not compiled in. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde | 1.0.228 | Serialization / deserialization | Always. Drives axum JSON extractors and responders, config deserialization, everything. Use `derive` feature. |
| serde_json | 1.0.134 | JSON support for serde | Always alongside serde. axum's `Json<T>` extractor/responder uses it internally. |
| tower-http | 0.6.8 | HTTP middleware | Add for request tracing (`TraceLayer`), CORS (`CorsLayer`), and compression. axum is Tower-native so tower-http middleware composes with zero friction. |
| tracing | 0.1.x | Structured async logging | The correct logging approach in async Rust — spans correlate events across concurrent tasks. Use `tracing-subscriber` for stdout formatting. Never use the `log` crate directly. |
| tracing-subscriber | 0.3.x | Tracing output formatter | Pairs with `tracing`. Format logs as JSON for production or human-readable for dev via `EnvFilter`. |
| config | 0.15.22 | Layered configuration | Handles TOML file + env var merging in one pass. Supports prefix-scoped env vars (e.g., `MNEMONIC_HOST`). Cleaner than hand-rolling env + file parsing. |
| uuid | 1.22.0 | Unique ID generation | Memory IDs, session IDs. Use `v4` feature for random IDs. Consider `v7` if you want sortable, time-prefixed IDs for the primary key (better SQLite index locality). |
| thiserror | 2.x | Ergonomic error types | Define domain errors with `#[derive(Error)]`. Each error variant maps to an HTTP status code in axum's `IntoResponse` impl. Avoids `anyhow` which loses structured error info for API responses. |
| reqwest | 0.12.x | HTTP client (OpenAI fallback) | Only needed for the optional OpenAI embeddings API path. Use `rustls-tls` feature to avoid OpenSSL dependency. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| cargo (Rust stable) | Build system | Use stable toolchain. No nightly features required by this stack. |
| cargo-watch | Live reload during dev | `cargo watch -x run` for fast iteration on API and model changes. |
| cargo-dist | Single-binary release distribution | Generates GitHub release artifacts for macOS (arm64, x86_64), Linux (musl), and Windows. The right tool for distributing a single binary. |
| x86_64-unknown-linux-musl target | Static Linux binary | Build with `RUSTFLAGS="-C target-feature=+crt-static"` for a fully static Linux binary. Required for Docker scratch images and broad Linux distribution. |
| sqlx-cli (optional) | Migration tooling | Only if you choose sqlx over rusqlite later. Not needed for this stack since schema is applied programmatically. |

## Installation

```toml
# Cargo.toml

[dependencies]
# Async runtime + HTTP
tokio = { version = "1", features = ["full"] }
axum = "0.8"
tower-http = { version = "0.6", features = ["trace", "cors"] }

# Database
rusqlite = { version = "0.38", features = ["bundled"] }
tokio-rusqlite = "0.7"

# Vector extension (loaded at runtime via rusqlite extension API)
sqlite-vec = "0.1"

# ML inference
candle-core = "0.9"
candle-nn = "0.9"
candle-transformers = "0.9"
tokenizers = "0.22"
hf-hub = "0.3"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Config
config = "0.15"

# Utilities
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "2"

# OpenAI fallback (optional)
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| axum | actix-web | If you need the absolute highest raw throughput benchmark numbers. actix-web has slightly higher ceiling but much worse ergonomics, a different middleware model (actix-style actors), and more churn historically. Not worth it here. |
| axum | warp | Never. warp's type-level filter composition is hard to maintain at scale and the compiler errors are painful. axum has strictly better DX. |
| candle | ort (ONNX Runtime) | If you need broader model format support and accept a system dependency. ort requires the ONNX Runtime C library (~100MB) — incompatible with the single-binary constraint. Do not use for this project. |
| candle | burn | burn is a newer Rust ML framework with better training support. For inference-only with HuggingFace models, candle has more model implementations and better HF Hub integration today. Revisit in 12–18 months. |
| rusqlite + tokio-rusqlite | sqlx | sqlx provides async-native SQLite and compile-time query checking. However, sqlx's SQLite support does not expose the C extension API needed to load sqlite-vec. rusqlite exposes `load_extension` which is required. |
| sqlite-vec | Qdrant | Qdrant is a purpose-built vector DB — excellent if you need an external service. sqlite-vec satisfies the zero-external-dependency constraint. Switch to Qdrant as a pluggable backend in a future milestone if needed. |
| sqlite-vec | pgvector | pgvector requires PostgreSQL. Outside scope. Future milestone. |
| hf-hub | bundle weights in binary | Bundling 22MB+ of model weights in the binary via `include_bytes!` bloats compile times, git history, and binary size without benefit. hf-hub caches on first run and is the correct approach. |
| thiserror | anyhow | anyhow is excellent for applications where you just want `?` propagation. For a REST API, you need structured errors that map to HTTP status codes — thiserror preserves that structure. Use anyhow only in CLI tooling or integration tests. |
| uuid v4 | uuid v7 | Use v7 UUIDs if you want time-ordered primary keys. v7 UUIDs sort chronologically which improves B-tree index locality in SQLite for large tables. Either works; v4 is simpler to start. |
| config | dotenvy | dotenvy is env-var only. config handles both TOML file and env vars in one layered system. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| sqlite-vss | Archived. No longer maintained. The successor is sqlite-vec. | sqlite-vec 0.1.7+ |
| ort / ONNX Runtime | Requires a C shared library (~100MB) at runtime. Breaks single-binary distribution. | candle (pure Rust) |
| fastembed-rs (default features) | Default backend is ONNX Runtime, not candle. Adds system dependency unless you only use the handful of candle-feature-flagged models. The candle coverage is narrow compared to native candle. | Use candle directly with hf-hub + tokenizers |
| diesel | Diesel is excellent for relational workloads but does not expose the rusqlite C extension API. You cannot load sqlite-vec through diesel. | rusqlite with tokio-rusqlite |
| sqlx for this project | sqlx's SQLite driver does not expose `load_extension`. sqlite-vec must be registered via the extension API. | rusqlite |
| log crate | log is synchronous and loses async span context. In an async tokio application, tracing spans let you correlate log events across concurrent tasks. | tracing + tracing-subscriber |
| actix-rt or async-std | The entire stack (axum, tower, tokio-rusqlite) is built for tokio. Mixing runtimes causes pain. | tokio |

## Stack Patterns by Variant

**If deploying as a Docker container:**
- Build with `x86_64-unknown-linux-musl` target and `RUSTFLAGS="-C target-feature=+crt-static -C link-self-contained=yes"`
- Produces a fully static binary deployable in a scratch container
- Be aware: musl's allocator is slower than glibc under heavy multithreaded load — consider jemalloc via `tikv-jemallocator` if benchmarks show contention

**If enabling GPU acceleration (future):**
- candle supports CUDA via the `cuda` feature flag
- Requires CUDA toolkit at build time, adds system dependency
- Keep CPU-only as the default, document GPU builds as an opt-in compilation variant

**If adding OpenAI embedding fallback:**
- Use reqwest with `rustls-tls` (not `native-tls`) to avoid system OpenSSL
- Feature-flag behind `openai` feature to keep the default binary clean
- Gate at config initialization, not at request time

**If embedding model needs to change:**
- all-MiniLM-L6-v2 is aging (2019 architecture, 512 token limit, low MTEB scores in 2025 benchmarks)
- nomic-embed-text-v1.5 is a strong upgrade: longer context (8192 tokens), better MTEB, still candle-compatible
- BGE-small-en-v1.5 is a viable alternative: similar size, better MTEB, candle-compatible
- Keep the model identifier in config so users can swap without recompilation

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| axum 0.8.x | tower-http 0.6.x | axum 0.7 required tower-http 0.5. axum 0.8 moved to tower-http 0.6. Keep them aligned. |
| candle-core 0.9.2 | candle-nn 0.9.2, candle-transformers 0.9.2 | All candle subcrates must be the exact same version. Version mismatch causes linker errors. |
| tokenizers 0.22.x | candle 0.9.x | Both are HuggingFace projects. Use latest tokenizers with latest candle. |
| rusqlite 0.38 | tokio-rusqlite 0.7 | tokio-rusqlite 0.7 lists rusqlite 0.38 as a compatible dependency. Verify if upgrading rusqlite. |
| rusqlite 0.38 | sqlite-vec 0.1.x | sqlite-vec loads via rusqlite's `load_extension` / `sqlite3_auto_extension` APIs. No version coupling beyond rusqlite 0.x API compatibility. |
| serde 1.x | serde_json 1.x | Stable. serde 1.0 has been backward-compatible since 2018. |

## Sources

- [axum docs.rs 0.8.8](https://docs.rs/axum/latest/axum/) — version confirmed
- [tokio GitHub releases](https://github.com/tokio-rs/tokio/releases) — v1.50.0 confirmed
- [sqlite-vec GitHub releases](https://github.com/asg017/sqlite-vec/releases) — v0.1.7 confirmed (March 17 2026)
- [candle Cargo.toml](https://github.com/huggingface/candle/blob/main/Cargo.toml) — v0.9.2 confirmed
- [tokenizers docs.rs](https://docs.rs/tokenizers/latest/tokenizers/) — v0.22.2 confirmed
- [tokio-rusqlite docs.rs](https://docs.rs/tokio-rusqlite/latest/tokio_rusqlite/) — v0.7.0 confirmed
- [rusqlite docs.rs](https://docs.rs/crate/rusqlite/latest) — v0.38.0 confirmed, bundles SQLite 3.51.1
- [tower-http docs.rs](https://docs.rs/tower-http/latest/tower_http/) — v0.6.8 confirmed
- [config docs.rs](https://docs.rs/config/latest/config/) — v0.15.22 confirmed
- [uuid crates.io](https://crates.io/crates/uuid) — v1.22.0 confirmed (MEDIUM confidence, crates.io requires JS)
- [fastembed-rs README](https://github.com/Anush008/fastembed-rs/blob/main/README.md) — confirmed default backend is ONNX Runtime, not candle (why it's excluded)
- [HN: Don't use all-MiniLM-L6-v2](https://news.ycombinator.com/item?id=46081800) — model aging context (MEDIUM confidence, community discussion)
- [sqlite-vec Rust docs](https://alexgarcia.xyz/sqlite-vec/rust.html) — Rust integration pattern confirmed

---
*Stack research for: Rust agent memory server (mnemonic)*
*Researched: 2026-03-19*
