# Feature Research

**Domain:** Agent memory server (lightweight, single-binary, REST API)
**Researched:** 2026-03-19
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features that agent developers assume exist. Missing any of these and the product is not a credible alternative.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Store a memory (write) | Fundamental CRUD — no write = no product | LOW | `POST /memories` with content + metadata |
| Retrieve by semantic search | The whole point of a memory server vs. a key-value store | MEDIUM | Vector similarity via embedding + ANN index |
| Delete a memory | Agents need to forget wrong or outdated information | LOW | `DELETE /memories/{id}` |
| List / filter memories | Inspect what's stored; filter by agent, session, time | LOW | `GET /memories?agent_id=&session_id=` |
| agent_id namespacing | Multi-agent isolation is universally expected; every competitor has it | LOW | Query param or header scopes all operations |
| session_id grouping | Session-scoped retrieval is a first-class pattern across all tools reviewed | LOW | Orthogonal to agent_id; narrows recall to a session |
| Persistence across restarts | "Memory" that disappears on restart isn't memory | LOW | SQLite file on disk covers this |
| Health / readiness endpoint | Ops requirement; needed for Docker, Kubernetes, any process manager | LOW | `GET /health` returning 200 |
| Plain JSON API | Every consumer language must be able to call it without an SDK | LOW | Standard REST + `application/json` |

### Differentiators (Competitive Advantage)

These are where Mnemonic competes. The core value proposition is zero-config, single-binary — most competitors require Python, Docker, external services, or cloud accounts.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Bundled local embedding model | No OpenAI key needed to get started; offline-capable; zero cost per embedding | HIGH | all-MiniLM-L6-v2 via candle (pure Rust, no ONNX Runtime). This is the defining differentiator. Competitors either require an API key (Mem0 cloud, Zep cloud) or an external Python service. |
| Single Rust binary | Download and run — no Python, no Docker, no Node.js, no npm install | HIGH | Enables distribution via `cargo install`, GitHub releases, Homebrew. No other reviewed tool ships as a true single binary with bundled inference. |
| SQLite-backed (single file) | All data is one `.db` file — trivial to back up, copy, inspect, or wipe | MEDIUM | sqlite-vec for vectors + standard SQLite for metadata. Simpler than Qdrant, Chroma, Redis. |
| Zero-config startup | Works out of the box with no configuration file required | LOW | Sensible defaults; env vars or TOML for overrides when needed |
| Optional OpenAI embedding fallback | Escape hatch for users who want higher-quality embeddings or have a key | LOW | Activated by env var; no code changes by the caller |
| Metadata filtering on search | Narrow semantic search to specific agents, sessions, or time windows | MEDIUM | Combine vector similarity with SQL WHERE clauses; mirrors Qdrant/Mem0 filter patterns |
| Unix-friendly output | stdout logging, clean exit codes, signal handling | LOW | Expected by anyone running it in a shell script or process supervisor |

### Anti-Features (Commonly Requested, Often Problematic)

These are features that will be requested but should be deliberately deferred or refused for the initial product. Scope creep here kills the "zero-dependency" value proposition.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Web UI / dashboard | "Show me what's stored" is a natural ask | Adds a frontend build pipeline, static asset serving, and significant scope. Violates the single-binary simplicity story. | Users can query the REST API with curl, httpie, or any API client. SQLite file is inspectable with DB Browser for SQLite. |
| Authentication / API keys | Production deployments need access control | Adds auth middleware, key management, storage for hashed keys — meaningful scope. Premature for an embeddable tool used locally. | Run behind a reverse proxy (nginx, Caddy) or use network-level access control. Document this explicitly. |
| Memory summarization / compaction | Long conversations need summarization to stay under context limits | Requires LLM call on every compaction cycle. Adds latency, cost, and error surface. Needs careful prompt engineering. | Future milestone. Users can implement summarization in their agent code and store the summary as a new memory. |
| gRPC / streaming API | Performance-conscious users will ask | Doubles the interface surface. REST is sufficient for all reviewed use cases. gRPC adds a proto compile step and language-specific stubs. | REST with keep-alive is adequate. Add gRPC only after REST is validated. |
| Pluggable storage backends (Qdrant, Postgres) | Power users want to swap out SQLite | Massively increases abstraction complexity. The "single file" story is a feature, not a limitation. | Document that Qdrant/Postgres variants can be forked. Keep the core simple. |
| Automatic entity extraction / knowledge graphs | Zep and Hindsight do this — it looks impressive | Requires an LLM call per write. Adds latency and API key dependency. Out of scope for a local, zero-config tool. | Users can store pre-extracted entities as metadata fields. |
| Memory decay / TTL expiration | "Old memories should fade" is conceptually appealing | Surprising behavior that can silently lose data. Hard to tune correctly. Adds background job complexity. | Future milestone. Let users delete memories explicitly. |
| Multi-node / distributed mode | "What if I have 10 agents on different machines?" | SQLite is not designed for multi-writer distributed use. Would require replacing the storage layer entirely. | Single-node is the correct initial scope. Document that SQLite WAL mode handles concurrent reads well. |

## Feature Dependencies

```
[agent_id namespacing]
    └──required by──> [Semantic search with filtering]
                          └──required by──> [session_id grouping in search]

[Bundled embedding model]
    └──required for──> [Zero-config semantic search]
                           └──enables──> [Single-binary distribution]

[Optional OpenAI embedding fallback]
    └──enhances──> [Bundled embedding model]
    └──requires──> [Embedding provider abstraction in code]

[SQLite storage]
    └──required by──> [Persistence across restarts]
    └──required by──> [Metadata filtering on search]
    └──required by──> [List / filter memories]

[Health endpoint]
    └──independent of all other features]

[Memory summarization / compaction]
    └──requires──> [Store a memory] (compaction writes summaries)
    └──requires──> [Delete a memory] (compaction removes originals)
    └──deferred to v2]
```

### Dependency Notes

- **agent_id required by filtering:** All search and list endpoints must accept agent_id to scope results. This is foundational — if it is not in the schema from day one, retrofitting it requires a migration.
- **Bundled model required for zero-config:** The entire "download and run" story depends on not requiring an external service for embeddings. candle-based inference must ship in the binary.
- **Embedding provider abstraction required for OpenAI fallback:** The fallback cannot be bolted on after the fact. The embedding interface must be a trait with at least two implementations from the beginning.
- **SQLite required for filtering:** Metadata filtering is implemented as SQL WHERE clauses on the memories table. This is only possible because storage is SQLite — it is not an add-on.

## MVP Definition

### Launch With (v1)

The minimum viable product validates: "Can any agent store and semantically search memories with zero configuration?"

- [ ] `POST /memories` — store content + metadata (agent_id, session_id, arbitrary key-value tags)
- [ ] `GET /memories/search?q=&agent_id=&session_id=&limit=` — semantic search with filters
- [ ] `GET /memories` — list memories with filter params
- [ ] `DELETE /memories/{id}` — delete a single memory
- [ ] `GET /health` — liveness check
- [ ] Bundled all-MiniLM-L6-v2 via candle — zero-config local inference
- [ ] Optional `OPENAI_API_KEY` env var to use OpenAI embeddings instead
- [ ] SQLite + sqlite-vec storage in a single `.db` file
- [ ] agent_id + session_id namespacing on all operations
- [ ] Configuration via env vars with documented TOML override
- [ ] Clean README: quickstart in under 3 commands, full API reference, examples for curl + Python + LangChain

### Add After Validation (v1.x)

Add once core is working and real agents are using it.

- [ ] `PUT /memories/{id}` — update memory content (triggers re-embedding) — add when users ask for correction workflows
- [ ] Hybrid search (vector + BM25 keyword) — add when users report poor recall on exact-match queries
- [ ] Memory tags / custom metadata fields beyond agent_id / session_id — add when users report the current schema is too rigid
- [ ] Batch write endpoint (`POST /memories/batch`) — add when users hit write throughput issues
- [ ] OpenTelemetry tracing — add when users ask for observability in production deployments

### Future Consideration (v2+)

Defer until product-market fit is established.

- [ ] Memory summarization / compaction — requires LLM integration; defer until users request it with specific workflows
- [ ] Authentication / API keys — defer until users deploy Mnemonic in multi-user environments
- [ ] Web UI — defer until users ask for a visual inspection tool
- [ ] gRPC interface — defer until REST is validated and performance requirements emerge
- [ ] Pluggable storage backends — defer; revisit only if SQLite limitations are hit in practice

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Store memory (POST /memories) | HIGH | LOW | P1 |
| Semantic search (GET /memories/search) | HIGH | MEDIUM | P1 |
| Delete memory | HIGH | LOW | P1 |
| List / filter memories | HIGH | LOW | P1 |
| agent_id + session_id namespacing | HIGH | LOW | P1 |
| Health endpoint | MEDIUM | LOW | P1 |
| Bundled local embedding model | HIGH | HIGH | P1 |
| SQLite + sqlite-vec storage | HIGH | MEDIUM | P1 |
| OpenAI embedding fallback | MEDIUM | LOW | P1 |
| Zero-config startup | HIGH | LOW | P1 |
| PUT /memories/{id} update | MEDIUM | MEDIUM | P2 |
| Hybrid vector + BM25 search | MEDIUM | HIGH | P2 |
| Batch write endpoint | LOW | MEDIUM | P2 |
| OpenTelemetry tracing | LOW | MEDIUM | P2 |
| Memory summarization / compaction | MEDIUM | HIGH | P3 |
| Authentication / API keys | MEDIUM | HIGH | P3 |
| Web UI | LOW | HIGH | P3 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | Mem0 | Zep | Redis Agent Memory Server | Mnemonic (our approach) |
|---------|------|-----|--------------------------|-------------------------|
| Single binary distribution | No (Python package) | No (Go binary but external DB required) | No (Python + Redis) | Yes — defining differentiator |
| Local embedding inference | No (API key required by default) | No (requires LLM API) | No (requires LLM API) | Yes — bundled candle model |
| Zero-config startup | No | No | No | Yes |
| REST API | Yes | Yes | Yes | Yes |
| Semantic search | Yes | Yes (temporal KG) | Yes | Yes |
| Metadata filtering | Yes | Yes | Yes | Yes |
| agent_id / session namespacing | Yes | Yes (user/session) | Yes | Yes |
| Multi-agent isolation | Yes (cloud) | Yes | Yes | Yes (via agent_id) |
| Single-file storage | No | No | No | Yes (SQLite .db) |
| Memory summarization | Yes | Yes (auto) | Yes (configurable) | No (v2+) |
| Knowledge graph / entity extraction | No | Yes (Graphiti) | Yes (entity recognition) | No (out of scope) |
| Authentication | Yes (cloud) | Yes | Yes | No (v2+) |
| Open source | Yes (SDK) | Yes (community) | Yes | Yes |
| Language | Python / TypeScript SDK | Go backend | Python backend | Rust (no SDK needed) |

## Sources

- [Mem0 GitHub — mem0ai/mem0](https://github.com/mem0ai/mem0) — Feature set, SDK, deployment options
- [Mem0 Research Paper (arXiv 2504.19413)](https://arxiv.org/abs/2504.19413) — Memory architecture analysis
- [Zep: Temporal Knowledge Graph Architecture (arXiv 2501.13956)](https://arxiv.org/abs/2501.13956) — Zep feature set and design
- [Zep Agent Memory Product Page](https://www.getzep.com/product/agent-memory/) — Features and positioning
- [Redis Agent Memory Server GitHub](https://github.com/redis/agent-memory-server) — Feature set, API design
- [Redis Agent Memory Server Docs](https://redis.github.io/agent-memory-server/) — Architecture and capabilities
- [Hindsight Open-Source MCP Memory Server](https://hindsight.vectorize.io/blog/2026/03/04/mcp-agent-memory) — Retain/Recall/Reflect pattern
- [5 AI Agent Memory Systems Compared (DEV Community)](https://dev.to/varun_pratapbhardwaj_b13/5-ai-agent-memory-systems-compared-mem0-zep-letta-supermemory-superlocalmemory-2026-benchmark-59p3) — Benchmark data and differentiation analysis
- [Top 10 AI Memory Products 2026 (Medium)](https://medium.com/@bumurzaqov2/top-10-ai-memory-products-2026-09d7900b5ab1) — Ecosystem landscape
- [Memory for AI Agents: A New Paradigm (The New Stack)](https://thenewstack.io/memory-for-ai-agents-a-new-paradigm-of-context-engineering/) — Context engineering patterns
- [Amazon Bedrock AgentCore Memory Organization](https://docs.aws.amazon.com/bedrock-agentcore/latest/devguide/memory-organization.html) — Namespace and session_id patterns
- [Metadata Filtering and Hybrid Search (Dataquest)](https://www.dataquest.io/blog/metadata-filtering-and-hybrid-search-for-vector-databases/) — Filter API design patterns
- [Why Your Agent's Memory Architecture Is Probably Wrong (DEV Community)](https://dev.to/agentteams/why-your-agents-memory-architecture-is-probably-wrong-55fc) — Anti-patterns and pitfalls

---
*Feature research for: agent memory server (Mnemonic)*
*Researched: 2026-03-19*
