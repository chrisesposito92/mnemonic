# Pitfalls Research

**Domain:** Rust agent memory server — embedded SQLite + vector search + local ML inference
**Researched:** 2026-03-19
**Confidence:** HIGH (critical pitfalls verified via official docs and known issues; performance numbers from benchmarks)

---

## Critical Pitfalls

### Pitfall 1: Holding a `std::sync::Mutex` Lock Across `.await` Points

**What goes wrong:**
When axum handler code locks a `std::sync::Mutex` (e.g., wrapping a `Connection` or shared state), then calls `.await` while the guard is held, the future becomes `!Send`. tokio may suspend the task at the `.await` point and schedule another task on the same thread that also tries to acquire the lock — resulting in a deadlock. The compiler will flag `!Send` as an error in most cases, but when it does not, the program silently deadlocks under concurrent load.

**Why it happens:**
Developers reach for `std::sync::Mutex` first because it is familiar and lighter than `tokio::sync::Mutex`. The `!Send` error message from the compiler is not always intuitive, and developers sometimes refactor around it in ways that introduce deadlocks rather than removing the lock-across-await.

**How to avoid:**
Use one of three patterns depending on context:
1. Lock only inside non-async methods on a wrapper struct, never in an async function body.
2. Use `tokio::sync::Mutex` when holding the lock across async work is unavoidable.
3. Use a dedicated actor task (tokio channel + single background thread) for all state that requires mutation — this is also the pattern tokio-rusqlite uses internally and is the right shape for SQLite writes.

**Warning signs:**
- Compiler error: `future is not Send` mentioning `MutexGuard` or similar.
- Timeout errors under concurrent load that go away when concurrency drops to 1.
- Tests pass sequentially but hang or fail with multiple concurrent requests.

**Phase to address:** Foundation / HTTP layer setup (the phase where axum + shared DB state is wired together).

---

### Pitfall 2: Using a Multi-Connection Pool for SQLite Writes

**What goes wrong:**
Using a connection pool (even with `tokio-rusqlite` or `deadpool-sqlite`) with more than one connection for write operations causes SQLite's exclusive write lock to become a contention point. When multiple tasks hold transactions with `await` points between write statements, other writers hit `SQLITE_BUSY` errors and either fail or queue up — destroying throughput. One benchmark showed a 20x write throughput difference between a single-writer design and a naive multi-connection pool.

**Why it happens:**
Developers assume "more connections = more parallelism" as they would with Postgres. SQLite's write serialization is fundamentally different. The `SQLITE_BUSY` error only appears under concurrent load, not during development with a single agent.

**How to avoid:**
Mirror SQLite's native architecture at the application layer:
- One single connection (or `max_connections = 1` pool) for all writes, backed by a tokio channel queue.
- A separate read pool with multiple connections for concurrent SELECT queries.
- Enable WAL mode (`PRAGMA journal_mode = WAL`) so readers never block writers and vice versa.
- Set `PRAGMA synchronous = NORMAL` and `PRAGMA busy_timeout = 5000` to handle brief lock waits gracefully.

**Warning signs:**
- `SQLITE_BUSY` or `database is locked` errors in logs under concurrent agent traffic.
- Write latency spikes as concurrency increases.
- Works fine with one agent, fails with several.

**Phase to address:** Database / storage layer setup. Must be in place before any load testing.

---

### Pitfall 3: Wrong Embedding Pooling Strategy Produces Bad Search Results

**What goes wrong:**
all-MiniLM-L6-v2 requires **mean pooling with attention mask** to produce correct sentence embeddings. If the candle implementation uses CLS token extraction, last-token extraction, or simple mean without respecting the attention mask (which zeros out padding tokens), the resulting embedding vectors are incorrect. Semantic search will return nonsensical results — similar sentences ranked far apart, dissimilar sentences ranked close together. This is a silent failure: the system runs without errors.

**Why it happens:**
candle's BERT example code does not always apply the full pooling pipeline. Developers copy the forward pass but miss the post-processing. The attention mask is passed to `model.forward()` but the mask-weighted averaging of token embeddings is a separate step that must be implemented explicitly.

**How to avoid:**
Implement mean pooling as: sum of (token embeddings * attention_mask) divided by sum of attention_mask values, applied per sequence in the batch. Then L2-normalize the result. Validate against the official Python sentence-transformers output for a known sentence pair before shipping.

**Warning signs:**
- Cosine similarity scores between clearly related phrases are near 0 or negative.
- All stored memories are returned with similar scores regardless of query relevance.
- Benchmark recall against Python sentence-transformers output — any divergence above ~0.01 cosine distance indicates a pooling or normalization bug.

**Phase to address:** Embedding / inference implementation phase, before any search evaluation.

---

### Pitfall 4: Storing Embeddings Without Normalization, Then Switching Similarity Metric

**What goes wrong:**
sqlite-vec's KNN search uses either L2 (Euclidean) or inner product distance. If embeddings are not L2-normalized to unit length, using inner product as a proxy for cosine similarity produces wrong rankings (inner product = cosine similarity only for unit vectors). Worse, if the similarity metric is changed after data has been stored, all existing vectors become incomparable under the new metric — requiring a full re-embedding pass.

**Why it happens:**
Developers treat the metric as a deployment detail. They get reasonable-looking results in testing (all vectors happen to be near-unit-length from the BERT output), then discover the subtle ranking errors later. Changing the metric mid-project forces a costly migration.

**How to avoid:**
Decide on the metric (cosine via normalized inner product) during schema design, not after. Always L2-normalize the embedding vector before storing it. Document the normalization contract so future contributors do not accidentally skip it.

**Warning signs:**
- KNN results are consistent but wrong relative to expected semantic rankings.
- Embedding vectors stored in the DB have varying L2 norms (check with a diagnostic query).

**Phase to address:** Schema design and embedding storage implementation, before any data is written to disk.

---

### Pitfall 5: Model Weights Bundled With `include_bytes!` Cause Compile-Time and Binary Size Problems

**What goes wrong:**
The naive approach to "zero-config, self-contained" is to embed the all-MiniLM-L6-v2 model weights (~22MB SafeTensors + tokenizer) into the binary via `include_bytes!`. This is documented to cause significantly longer compile times (the Rust compiler processes the entire blob as a constant) and produces a binary that is ~45MB+ before stripping. More critically, every change to any source file triggers a full relink of the binary including the embedded blob, making the development loop painful.

**Why it happens:**
`include_bytes!` is the obvious mechanism for "I want a file inside my binary." The compile-time cost only becomes apparent after iteration — initial build times are slow but feel like "first build" cost.

**How to avoid:**
Embed a minimal bootstrap instead. Use one of these strategies:
1. Ship the model weights as a sidecar file alongside the binary, extracted on first run from a compressed blob embedded via `include_bytes!` (compress with zstd, accept the decompression cost once).
2. Download the model weights on first run to `~/.cache/mnemonic/` (with a progress indicator and checksum verification) — this is what candle's own examples do.
3. Accept the binary size if sub-50ms compile iteration is not a priority.

Strategy 2 (download on first run to cache dir) best matches the "zero-config single binary" goal while keeping the development loop fast.

**Warning signs:**
- Incremental rebuilds take 10+ seconds even for trivial code changes.
- Binary is >40MB before stripping.
- `cargo check` is fast but `cargo build` is slow — the link step is the bottleneck.

**Phase to address:** Binary distribution design phase (early). Must be decided before the embedding module is wired up.

---

### Pitfall 6: sqlite-vec Brute-Force Search Becomes Unacceptable Past ~100K Memories

**What goes wrong:**
sqlite-vec uses brute-force KNN (no ANN index as of 2026). Benchmarks show: at 1M vectors of 384 dimensions, a single KNN query takes ~500ms on typical hardware. At 100K vectors, it is around 50ms — acceptable for interactive use but already noticeable. For an agent memory server that may accumulate years of memories, this is a hard ceiling unless mitigation is in place.

**Why it happens:**
sqlite-vec's ANN (Approximate Nearest Neighbors) index is planned but not yet implemented. Developers choose it for correctness and simplicity (no index training, no approximate results) and assume the scale is "just agent memories" — but active agents can generate thousands of memories per day.

**How to avoid:**
- Add an `agent_id` + `session_id` index to enable pre-filtering before KNN — this drastically reduces the vector search space for any real query.
- Document the scale threshold (~100K total memories, or ~50K per agent) in the README.
- Design the schema with a `LIMIT` on vec0 queries and return `top_k` (default 10) rather than full scans.
- Consider partitioning vec0 virtual tables per agent_id if a single agent approaches 50K memories.

ANN support is tracked in the sqlite-vec repo and should be checked before committing to the brute-force-only approach for large-scale deployments.

**Warning signs:**
- KNN query latency increases linearly with row count.
- `/search` endpoint becomes noticeably slow after an agent has been running for weeks.
- No `WHERE agent_id = ?` pre-filter on the vec0 virtual table query.

**Phase to address:** Schema design phase. The pre-filter pattern must be in the schema from day one — retrofitting it is a migration.

---

### Pitfall 7: Embedding Model Mismatch After Changing Provider

**What goes wrong:**
When a user switches from local candle inference to the OpenAI API fallback (or vice versa), new embeddings are generated in a different vector space than existing stored embeddings. Cosine similarity between old and new vectors is meaningless — they are not in the same space. The system returns silently wrong results without any error.

**Why it happens:**
The embedding provider is configurable (candle vs. OpenAI). Developers test each path separately and do not consider the mixed state — e.g., 10K memories stored with local embeddings, then a few days of using the OpenAI provider because the local model was slow, then switching back.

**How to avoid:**
Store the embedding provider and model identifier with each memory row. On startup, warn (or error) if the configured provider differs from what was used to embed existing records. Provide a re-embed migration command that regenerates all vectors with the current provider. Never mix vectors from different models in the same KNN query.

**Warning signs:**
- No `embedding_model` column in the memories table.
- Semantic search quality degrades after changing the `OPENAI_API_KEY` env var.
- Users report "memories seem random" after config changes.

**Phase to address:** Schema design phase. Add `embedding_model VARCHAR NOT NULL` to the schema from the start.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Single WAL connection for all reads + writes | Simpler connection management | Write throughput collapses under concurrent agents; SQLITE_BUSY errors | Never — split read/write pools from the start |
| `include_bytes!` for model weights | Truly zero-config single binary | 10+ second incremental rebuilds; hard to update model without recompile | Acceptable only if model will never change and compile time is not a concern |
| No `embedding_model` tracking per memory | Simpler schema | Full re-embed migration required if model changes | Never — one column, add it day one |
| Skip attention-mask weighting in mean pooling | Simpler candle code | Silent semantic search quality degradation | Never — no benefit to cutting this corner |
| Using `Arc<tokio::sync::Mutex<Connection>>` for DB | Quick to implement | tokio Mutex overhead; holding across await introduces subtle bugs | Only as a stepping stone; replace with actor pattern before first production use |
| Offset-based pagination for memory listing | Simpler to implement | Skipped/duplicated memories when concurrent writes happen during pagination | Acceptable for MVP; switch to cursor-based for production |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| sqlite-vec + rusqlite | Forgetting `unsafe { sqlite3_auto_extension(...) }` before opening any connection | Call `sqlite3_auto_extension(Some(sqlite3_vec_init))` once at program start, before the first `Connection::open()` |
| rusqlite + tokio | Running rusqlite operations directly on tokio async tasks (blocks executor thread) | Use `tokio-rusqlite`'s `Connection::call()` which dispatches to a dedicated background thread via mpsc channel |
| candle + tokenizers crate | Using the wrong vocabulary file or not applying WordPiece post-processing | Load `tokenizer.json` from the official HuggingFace repo for all-MiniLM-L6-v2; do not hand-roll tokenization |
| OpenAI embeddings fallback | Sending raw text without trimming; exceeding 8191 token limit | Trim inputs and chunk if needed; the `text-embedding-3-small` model has a hard input limit |
| axum + `State<T>` | Putting mutable state inside `State<T>` directly | All mutable state must be wrapped in `Arc<T>` where T provides interior mutability (Mutex, RwLock, or channel) |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| No WAL mode pragma | Reads block during writes; single agent locks out others | Set `PRAGMA journal_mode = WAL` on connection open | Immediately with >1 concurrent agent |
| KNN scan without pre-filter | Search latency grows as total memory count grows | Always filter by `agent_id` before KNN; use compound WHERE clause | ~50K total memories (~500ms latency) |
| Synchronous embedding generation in request path | Memory store latency = embedding time (~30-60ms) | This is unavoidable with local inference; document it; consider queued/async writes for bulk imports | Acceptable at normal agent write rates; problematic for bulk ingestion |
| candle GELU activation on CPU (no SIMD) | Candle inference is 2x slower than Python sentence-transformers | Build with `RUSTFLAGS="-C target-cpu=native"` for dev; for distribution, verify AVX2 is present at runtime | All CPUs pre-AVX2 (pre-2013 Intel, pre-2017 AMD) |
| Model weights loaded from disk on every request | Startup latency on every embedding call | Load model once at server startup into an `Arc<EmbeddingModel>`; reuse across all requests | First request, and every subsequent request if not fixed |
| Unbounded result sets from `/search` | Memory and latency blow up on large datasets | Always apply `LIMIT` in the KNN query; default `top_k = 10`, max `top_k = 100` | First query against a large dataset |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| No input length limits on memory content | Malicious agents send 100MB strings to exhaust RAM during embedding | Enforce max content length (e.g., 8192 chars) at the API layer before any embedding work |
| Path traversal in DB file path configuration | `db_path = "../../etc/passwd"` or similar env var manipulation | Validate and canonicalize the configured DB path; reject paths outside an allowed prefix |
| No rate limiting on `/search` | Expensive KNN scans triggered in a tight loop by misbehaving agents | Apply per-agent rate limiting on search; KNN is CPU-bound and cannot be parallelized efficiently |
| Logging memory content verbatim | Memory content may include secrets, PII, API keys | Log only memory IDs and metadata; never log `content` fields in production |
| No size limit on batch operations | A `POST /memories/batch` with 10K items blocks the entire server for seconds | Enforce a max batch size (e.g., 100 items per request) |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Silent model download on first start (no progress) | Server appears hung for 30-60 seconds on first run | Print a clear message: "Downloading all-MiniLM-L6-v2 (22MB) to ~/.cache/mnemonic/..." with progress |
| Generic 500 errors for embedding failures | Agent framework gets no signal about what went wrong | Return structured errors with `error_code` (e.g., `embedding_failed`, `model_not_loaded`) so callers can retry or fall back |
| No indication of which embedding model is active | Users debugging wrong search results cannot tell which model generated stored embeddings | Include `embedding_model` in API responses for GET /memories/:id and in the health endpoint |
| `DELETE` returns 200 even when `memory_id` not found | Agents cannot distinguish "deleted" from "never existed" | Return 404 for deletes where the row did not exist |
| No `total_count` in search responses | Agents cannot tell if they are seeing all relevant memories or just the top slice | Include `{ results: [...], total_searched: N, returned: K }` in search response body |

---

## "Looks Done But Isn't" Checklist

- [ ] **Embedding pooling:** Uses mean pooling with attention mask weighting — not just raw average of all token vectors. Validate against Python sentence-transformers for at least 5 sentence pairs.
- [ ] **Embedding normalization:** Vectors are L2-normalized to unit length before storage. Check with a diagnostic query that all stored norms are ~1.0.
- [ ] **WAL mode:** `PRAGMA journal_mode = WAL` is set before any reads or writes. Check via `PRAGMA journal_mode;` on the open connection.
- [ ] **Write serialization:** Only one connection ever writes to the DB at a time. No code path issues concurrent write transactions.
- [ ] **Model loaded once:** The candle model and tokenizer are loaded at startup into a shared `Arc<>`, not reloaded per request.
- [ ] **Mutex not held across await:** Grep for `MutexGuard` in async functions — none should cross an `.await` point.
- [ ] **`agent_id` pre-filter on KNN:** All vector search queries include `WHERE agent_id = ?` before the vec0 KNN operation.
- [ ] **Embedding model tracked per memory:** Schema includes `embedding_model` column; queries warn on mismatch.
- [ ] **Input length validated:** API layer rejects content over configured max length before invoking candle.
- [ ] **sqlite-vec extension registered:** `sqlite3_auto_extension` is called before first connection open; `vec_version()` returns a value in a startup self-check.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Wrong pooling / normalization in production | HIGH | Write a migration that re-embeds all stored memories; this is a full table scan + N embedding calls; plan for downtime or a background migration job |
| Mixed embedding models in the DB | HIGH | Query all distinct `embedding_model` values; re-embed rows from stale model; requires the old model weights still available |
| No WAL mode on existing DB | LOW | Run `PRAGMA journal_mode = WAL;` on the open database — SQLite applies this in place; no data migration needed |
| Binary with `include_bytes!` model weights is too slow to develop | MEDIUM | Refactor to download-on-first-run; requires updating the startup path and adding a cache dir abstraction |
| KNN performance collapse on large dataset | MEDIUM | Add `agent_id` pre-filter to all queries; requires only an index and query change, no schema migration |
| `std::sync::Mutex` held across await causing deadlock | MEDIUM | Refactor to actor pattern (tokio channel + single background task); requires rewiring state access throughout the handler layer |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Mutex held across await | Foundation — axum + state setup | Compile with a concurrent load test; check for `!Send` errors; no deadlocks under 10 concurrent requests |
| SQLite multi-connection write pool | Foundation — DB layer setup | Benchmark writes under 5 concurrent agents; confirm zero `SQLITE_BUSY` errors |
| Wrong embedding pooling | Embedding / inference implementation | Cosine similarity of known-similar pairs > 0.85; output matches Python sentence-transformers within 0.01 |
| Missing L2 normalization | Embedding / inference + schema design | Diagnostic query: `SELECT AVG(vec_length(embedding)) FROM memories` should return ~1.0 |
| `include_bytes!` binary size trap | Binary distribution design (early planning) | Incremental rebuild time < 3 seconds for a one-line code change |
| sqlite-vec brute-force scale ceiling | Schema design | Schema has `agent_id` pre-filter; documented scale limit in README |
| Embedding model mismatch | Schema design | Schema has `embedding_model VARCHAR NOT NULL`; startup check warns on mismatch |
| Model loaded per request | Embedding / inference + server initialization | One model load log line at startup; no re-load log lines during request handling |
| No WAL mode | DB initialization | `PRAGMA journal_mode;` returns `wal` in a startup self-check log line |
| No input length limit | API layer / handler implementation | Integration test: POST with 1MB content body returns 400 |

---

## Sources

- [sqlite-vec Rust usage guide](https://alexgarcia.xyz/sqlite-vec/rust.html) — official extension loading patterns for rusqlite
- [sqlite-vec stable release blog post](https://alexgarcia.xyz/blog/2024/sqlite-vec-stable-release/index.html) — brute-force limitation acknowledgment and scale benchmarks (1M vector data)
- [ANN index tracking issue — sqlite-vec #25](https://github.com/asg017/sqlite-vec/issues/25) — confirmed ANN not yet implemented
- [candle issue #2418: all-MiniLM-L6-v2 2x slower than Python](https://github.com/huggingface/candle/issues/2418) — GELU activation performance root cause
- [PSA: SQLite connection pool write performance — Evan Schwartz](https://emschwartz.me/psa-your-sqlite-connection-pool-might-be-ruining-your-write-performance/) — 20x throughput difference with single writer pattern
- [Tokio shared state docs](https://tokio.rs/tokio/tutorial/shared-state) — Mutex-across-await deadlock mechanics
- [tokio-rusqlite docs.rs](https://docs.rs/tokio-rusqlite) — background thread actor model for async SQLite
- [SQLite WAL mode official docs](https://sqlite.org/wal.html) — concurrent reader/single writer model
- [Milvus FAQ: sentence transformer mistakes](https://milvus.io/ai-quick-reference/what-are-common-mistakes-that-could-lead-to-poor-results-when-using-sentence-transformer-embeddings-for-semantic-similarity-tasks) — pooling and normalization pitfalls
- [rusqlite bundled feature and buildtime_bindgen requirement](https://github.com/launchbadge/sqlx/issues/3147) — sqlite version sync pitfall
- [Rust SIMD distribution guide](https://curiouscoding.nl/posts/distributing-rust-simd-binaries/) — AVX2/NEON binary compatibility
- [include_bytes! compile time issue — rust-lang #65818](https://github.com/rust-lang/rust/issues/65818) — large blob compile time cost

---
*Pitfalls research for: Rust agent memory server (embedded SQLite + sqlite-vec + candle inference)*
*Researched: 2026-03-19*
