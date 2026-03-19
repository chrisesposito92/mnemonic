# Architecture Research

**Domain:** Rust single-binary agent memory server (embedded vector DB + local ML inference)
**Researched:** 2026-03-19
**Confidence:** HIGH

## Standard Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        HTTP Layer (axum)                         │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────────┐    │
│  │ POST /memories│  │ GET /memories │  │ DELETE /memories  │    │
│  │   (store)     │  │  (search)     │  │    (delete)       │    │
│  └───────┬───────┘  └───────┬───────┘  └────────┬──────────┘    │
│          │                  │                   │               │
├──────────┴──────────────────┴───────────────────┴───────────────┤
│                      Service Layer                               │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    MemoryService                          │   │
│  │  store_memory()  search_memories()  delete_memory()      │   │
│  └──────┬─────────────────────┬────────────────────────────┘   │
│         │                     │                                  │
├─────────┴──────────┬──────────┴──────────────────────────────── ┤
│     Embedding Layer│           Storage Layer                     │
│  ┌─────────────────┴──┐   ┌───────────────────────────────────┐ │
│  │  EmbeddingEngine   │   │        MemoryRepository            │ │
│  │  (candle BERT)     │   │      (tokio-rusqlite + sqlite-vec) │ │
│  │                    │   │                                    │ │
│  │  embed(text)       │   │  insert()  knn_search()  delete()  │ │
│  │  → Vec<f32> [384]  │   │  filter(agent_id, session_id)      │ │
│  └────────────────────┘   └───────────────────────────────────┘ │
│                                      │                           │
├──────────────────────────────────────┴───────────────────────────┤
│                        Storage Layer                              │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │                   mnemonic.db (SQLite file)                 │  │
│  │  ┌───────────────────┐   ┌────────────────────────────┐    │  │
│  │  │  memories (table) │   │  vec_memories (vec0 table) │    │  │
│  │  │  id, agent_id,    │   │  id, embedding float[384]  │    │  │
│  │  │  session_id,      │   │                            │    │  │
│  │  │  content, meta,   │   │  KNN via MATCH + ORDER BY  │    │  │
│  │  │  created_at       │   │  distance                  │    │  │
│  │  └───────────────────┘   └────────────────────────────┘    │  │
│  └────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Implementation |
|-----------|----------------|----------------|
| HTTP Layer | Parse requests, route to handlers, serialize responses, return errors | axum Router + handlers, JSON via serde_json |
| AppState | Share long-lived resources across handlers safely | `Arc<AppState>` with `MemoryService` + config fields |
| MemoryService | Orchestrate store/search/delete flows; choose embedding provider | Plain struct; calls EmbeddingEngine then MemoryRepository |
| EmbeddingEngine | Generate 384-dim float vectors from text; provider abstraction | Trait with `LocalEngine` (candle BERT) + `OpenAiEngine` fallback |
| MemoryRepository | All SQLite I/O: insert memories, run vector KNN, filter, delete | tokio-rusqlite `Connection` handle; raw SQL with sqlite-vec |
| Config | Load env vars / TOML at startup; provide typed config to AppState | `config` or `figment` crate; accessed via `Arc<Config>` |

---

## Recommended Project Structure

```
src/
├── main.rs              # Entry point: load config, init DB, build router, bind port
├── config.rs            # Config struct: db_path, model_path, openai_key, host, port
├── state.rs             # AppState struct: Arc<MemoryService>, Arc<Config>
├── error.rs             # AppError enum: implements axum IntoResponse; maps to HTTP codes
│
├── api/                 # HTTP layer — thin handlers only, no business logic
│   ├── mod.rs           # build_router() → axum::Router with .with_state()
│   ├── memories.rs      # handlers: store_memory, search_memories, delete_memory, health
│   └── types.rs         # Request/response DTOs: StoreRequest, SearchRequest, MemoryResponse
│
├── service/             # Orchestration — business logic, no HTTP or SQL
│   ├── mod.rs
│   └── memory.rs        # MemoryService: store(), search(), delete_by_id(), delete_by_agent()
│
├── embedding/           # Embedding provider abstraction
│   ├── mod.rs           # EmbeddingEngine trait: async fn embed(&self, text: &str) -> Vec<f32>
│   ├── local.rs         # LocalEngine: loads candle BERT model at startup, runs inference
│   └── openai.rs        # OpenAiEngine: calls text-embedding-3-small via reqwest
│
├── db/                  # Storage layer — all SQL lives here
│   ├── mod.rs
│   ├── connection.rs    # open_db(): returns tokio_rusqlite::Connection; runs migrations
│   ├── migrations.rs    # SQL strings for schema creation (memories + vec0 tables)
│   └── memory.rs        # MemoryRepository: insert, knn_search, delete, filter methods
│
└── models/              # Shared domain types
    ├── mod.rs
    └── memory.rs        # Memory struct: id, agent_id, session_id, content, metadata, created_at
```

### Structure Rationale

- **api/:** Isolated HTTP concerns. Handlers are one-liners that call service methods and map results to HTTP responses. Swapping axum for another framework touches only this module.
- **service/:** The only place with business rules (e.g., "always embed before storing"). No SQL, no HTTP, easily unit-testable with mock repository.
- **embedding/:** Trait-based abstraction lets the service call `engine.embed()` without knowing whether it's running BERT locally or hitting OpenAI. The provider is chosen at startup from config.
- **db/:** All SQL strings and rusqlite calls are contained here. tokio-rusqlite's `.call()` closure pattern keeps async code clean. Migrations run on startup via `connection.call()`.
- **models/:** Shared domain structs used across all layers. No framework imports here — pure Rust.

---

## Architectural Patterns

### Pattern 1: Shared State via Arc<AppState>

**What:** A single `AppState` struct holds all long-lived resources. It is wrapped in `Arc` and attached to the axum Router via `.with_state()`. Handlers extract it with `State(state): State<Arc<AppState>>`.

**When to use:** Always, for a single-binary server. Every resource (DB connection, embedding engine, config) lives here. No global statics except the model weights.

**Trade-offs:** Simple and predictable. The `Arc` clone cost is negligible. Interior mutability via `tokio::sync::RwLock` is only needed for hot-reloadable config — skip it initially.

```rust
#[derive(Clone)]
pub struct AppState {
    pub service: Arc<MemoryService>,
    pub config: Arc<Config>,
}

// In main.rs:
let state = Arc::new(AppState { service, config });
let app = build_router().with_state(state);
```

### Pattern 2: Trait-Based Embedding Provider

**What:** Define an `EmbeddingEngine` trait with a single `async fn embed` method. `LocalEngine` (candle) and `OpenAiEngine` are concrete impls. `MemoryService` holds a `Box<dyn EmbeddingEngine + Send + Sync>`.

**When to use:** Essential here — the OpenAI fallback is a config-time choice. The trait keeps `MemoryService` testable with a fake `ConstantEngine` that returns fixed vectors.

**Trade-offs:** Requires `async_trait` crate (or Rust 1.75+ RPITIT syntax). Negligible runtime cost. Makes future embedding provider additions (e.g., Ollama) trivial.

```rust
#[async_trait]
pub trait EmbeddingEngine: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn dimensions(&self) -> usize;
}
```

### Pattern 3: tokio-rusqlite Call Closures

**What:** tokio-rusqlite's `Connection::call()` accepts a closure that runs on a dedicated blocking thread, returning a future. All SQLite operations go inside these closures so the tokio runtime is never blocked.

**When to use:** Every SQLite read or write. The closure captures prepared statement logic, which avoids string formatting in hot paths.

**Trade-offs:** Can't hold rusqlite types across `.await` points. All query logic must be self-contained within the closure. This is a hard constraint, not optional.

```rust
let memories = conn.call(|db| {
    let mut stmt = db.prepare(
        "SELECT id, content FROM memories
         WHERE agent_id = ?1
         ORDER BY created_at DESC LIMIT ?2"
    )?;
    // map_err to convert rusqlite::Error → AppError
    Ok(stmt.query_map([agent_id, limit], Memory::from_row)?.collect())
}).await?;
```

### Pattern 4: Model Loading at Startup (not per-request)

**What:** Load the BERT model weights once during `AppState` initialization, store the live model struct inside `LocalEngine`. All embedding requests reuse the loaded model.

**When to use:** Always. Loading safetensors weights from disk + tokenizer init takes 1-3 seconds and ~100MB RAM. Doing it per-request would make the server unusable.

**Trade-offs:** Increases startup time by ~1-3s and base memory by ~100MB. Both are acceptable for a persistent server process. The model is CPU-only initially; Metal/CUDA backends can be added later as feature flags.

```rust
pub struct LocalEngine {
    model: BertModel,           // candle model, loaded once
    tokenizer: Tokenizer,       // tokenizers::Tokenizer, loaded once
    device: Device,             // Device::Cpu or Device::new_cuda(0)
}

impl LocalEngine {
    pub fn load(model_path: &Path) -> Result<Self> {
        let device = Device::Cpu;
        let tokenizer = Tokenizer::from_file(model_path.join("tokenizer.json"))?;
        let weights = candle_core::safetensors::load(model_path.join("model.safetensors"), &device)?;
        let model = BertModel::load(VarBuilder::from_tensors(weights, DType::F32, &device), &config)?;
        Ok(Self { model, tokenizer, device })
    }
}
```

---

## Data Flow

### Store Memory Flow

```
POST /memories
  { content, agent_id, session_id, metadata }
        |
        v
  api/memories.rs: store_handler()
    - deserializes StoreRequest
    - calls state.service.store(req)
        |
        v
  service/memory.rs: MemoryService::store()
    - generates UUID for id
    - calls engine.embed(content) → Vec<f32> [384 dims]
    - calls repo.insert(memory, embedding)
        |
        v
  db/memory.rs: MemoryRepository::insert()
    - conn.call(|db| {
        INSERT INTO memories (id, agent_id, session_id, content, metadata, created_at) VALUES (...)
        INSERT INTO vec_memories (id, embedding) VALUES (...)
      })
        |
        v
  Returns 201 Created { id, created_at }
```

### Search Memory Flow

```
GET /memories/search
  { query, agent_id, session_id?, limit?, threshold? }
        |
        v
  api/memories.rs: search_handler()
    - deserializes SearchRequest
    - calls state.service.search(req)
        |
        v
  service/memory.rs: MemoryService::search()
    - calls engine.embed(query) → Vec<f32> [384 dims]
    - calls repo.knn_search(embedding, agent_id, session_id, limit)
        |
        v
  db/memory.rs: MemoryRepository::knn_search()
    - conn.call(|db| {
        SELECT m.*, v.distance
        FROM vec_memories v
        JOIN memories m ON v.id = m.id
        WHERE v.embedding MATCH ?1
          AND m.agent_id = ?2         -- namespace isolation
          AND m.session_id = ?3       -- optional session filter
        ORDER BY v.distance
        LIMIT ?4
      })
        |
        v
  Returns 200 OK [ { id, content, metadata, score, ... } ]
```

### Namespace Isolation

Every memory is tagged with `agent_id` (required) and `session_id` (optional). All queries filter by `agent_id` at minimum. This is enforced in `MemoryRepository` — the service layer never issues unscoped queries. Multi-agent isolation is a SQL-level WHERE clause, not application logic.

---

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 1 agent, hundreds of memories | Single SQLite file, WAL mode off, brute-force KNN is fast enough |
| ~10 agents, ~10K memories | Enable WAL mode + `synchronous=NORMAL` for concurrent reads during writes |
| ~100 agents, ~1M memories | sqlite-vec brute-force becomes the bottleneck; consider `quantize` to int8 vectors; profile first |
| Beyond that | Out of scope for v1; future milestone could add Qdrant/Postgres backends |

### Scaling Priorities

1. **First bottleneck — embedding latency:** The candle BERT forward pass on CPU is ~5-50ms per text. Under concurrent agent requests this becomes the chokepoint. Fix: tokio semaphore to bound concurrent embedding calls, or offload to a thread pool via `spawn_blocking`.
2. **Second bottleneck — KNN scan at high memory counts:** sqlite-vec uses brute-force cosine scan. At ~100K+ vectors per agent this slows down. Fix: vector quantization (int8 or binary) to reduce scan cost before considering external vector DBs.

---

## Anti-Patterns

### Anti-Pattern 1: Blocking SQLite in the Tokio Runtime

**What people do:** Call `rusqlite` directly inside an `async fn` without tokio-rusqlite's `.call()` wrapper.

**Why it's wrong:** Rusqlite is synchronous. Blocking the tokio worker thread stalls all other async tasks sharing that thread. Under concurrent requests the server degrades severely.

**Do this instead:** Always use `tokio_rusqlite::Connection::call(|db| { ... })`. Every rusqlite operation lives inside the closure, which runs on a dedicated background thread.

### Anti-Pattern 2: Loading the Embedding Model Per Request

**What people do:** Instantiate `LocalEngine` (or re-load model weights from disk) inside the request handler or service method.

**Why it's wrong:** Model loading takes 1-3 seconds and ~100MB of RAM. This makes every request slow and causes out-of-memory panics under concurrent load.

**Do this instead:** Load the model once in `main.rs` during `AppState` initialization, store it in `Arc<LocalEngine>` inside `AppState`, and share it across handlers.

### Anti-Pattern 3: Unscoped Vector Queries

**What people do:** Search all vectors without filtering by `agent_id`, then filter results in application code.

**Why it's wrong:** Returns memories from other agents (security bug), scans every vector in the table regardless of ownership (performance bug), and grows linearly with total memory count rather than per-agent count.

**Do this instead:** Always join `vec_memories` with `memories` and apply `WHERE m.agent_id = ?` before the KNN MATCH clause. sqlite-vec evaluates the MATCH first, but post-join filtering ensures correct namespace isolation.

### Anti-Pattern 4: Storing Model Weights in the Repo

**What people do:** Commit `model.safetensors` (~90MB) and `tokenizer.json` to the git repository.

**Why it's wrong:** Bloats the repo, complicates updates, breaks `git clone` for users on slow connections.

**Do this instead:** Download model weights at first startup from Hugging Face Hub using the `hf-hub` crate. Cache to `~/.cache/mnemonic/models/`. Document this in the README as expected startup behavior.

### Anti-Pattern 5: One Table for Metadata and Vectors

**What people do:** Try to store metadata columns directly inside a `vec0` virtual table.

**Why it's wrong:** sqlite-vec's `vec0` virtual tables only support vector columns and a rowid. Attempts to add arbitrary columns will fail or be silently ignored.

**Do this instead:** Use two tables: `memories` (regular table with all metadata) and `vec_memories` (vec0 virtual table with just `id` + `embedding`). Join them at query time. The `id` is the join key.

---

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| OpenAI Embeddings API | HTTP via `reqwest`; called only when `OPENAI_API_KEY` is set | Falls back automatically; same `EmbeddingEngine` trait as local |
| Hugging Face Hub | `hf-hub` crate at startup; downloads model if not cached locally | One-time download ~90MB; requires internet on first run |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| api/ ↔ service/ | Direct async method call via `Arc<MemoryService>` | No channels needed; service is stateless per-request |
| service/ ↔ embedding/ | Trait object call `Box<dyn EmbeddingEngine>` | Provider selected at startup from config |
| service/ ↔ db/ | Direct async method call via `Arc<MemoryRepository>` | Repository holds the `tokio_rusqlite::Connection` |
| db/ ↔ SQLite file | tokio-rusqlite `.call()` closures over a single connection | WAL mode enables concurrent reads while writing |

### Build Order (Component Dependencies)

The dependency graph dictates this build sequence:

```
1. models/         — domain types; no dependencies
2. error.rs        — AppError; depends on models
3. config.rs       — Config struct; no dependencies
4. db/             — requires models + error; foundational layer
5. embedding/      — requires error; standalone
6. service/        — requires db/ + embedding/ + models
7. api/            — requires service/ + models + error
8. state.rs        — requires service/ + config
9. main.rs         — wires everything together
```

**Phase implication:** Build in the order above. The DB schema (step 4) and embedding pipeline (step 5) are independent and can be developed in parallel. The service layer (step 6) is the integration point — it should not be written until both DB and embedding layers have working implementations.

---

## Sources

- [sqlite-vec Rust integration guide](https://alexgarcia.xyz/sqlite-vec/rust.html) — HIGH confidence (official docs)
- [sqlite-vec demo.rs — KNN query patterns](https://github.com/asg017/sqlite-vec/blob/main/examples/simple-rust/demo.rs) — HIGH confidence (official example)
- [tokio-rusqlite docs](https://docs.rs/tokio-rusqlite) — HIGH confidence (official docs)
- [axum docs — State extractor](https://docs.rs/axum/latest/axum/extract/struct.State.html) — HIGH confidence (official docs)
- [candle GitHub — BERT and sentence embeddings](https://github.com/huggingface/candle) — HIGH confidence (official repo)
- [Building Sentence Transformers in Rust with Candle](https://dev.to/mayu2008/building-sentence-transformers-in-rust-a-practical-guide-with-burn-onnx-runtime-and-candle-281k) — MEDIUM confidence (community article, verified against candle repo)
- [redis/agent-memory-server — namespace isolation patterns](https://github.com/redis/agent-memory-server) — MEDIUM confidence (reference implementation, different stack)
- [Axum production patterns](https://leapcell.io/blog/building-modular-web-apis-with-axum-in-rust) — MEDIUM confidence (community, consistent with official docs)
- [Rust web service layering best practices](https://blog.logrocket.com/best-way-structure-rust-web-services/) — MEDIUM confidence (community article)

---
*Architecture research for: Rust agent memory server (mnemonic)*
*Researched: 2026-03-19*
