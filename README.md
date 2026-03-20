# Mnemonic

Framework-agnostic agent memory server — persistent semantic memory via a simple REST API.

Mnemonic is a single binary that gives any AI agent durable, semantically searchable memory. Run it alongside your agent, store memories with a single POST, and retrieve the most relevant ones with a semantic search query. When memories accumulate, trigger compaction to deduplicate similar memories — with optional LLM-powered summarization. No external services, no configuration required — it works out of the box with a bundled embedding model.

## Table of Contents

- [Quickstart](#quickstart)
- [Concepts](#concepts)
- [Configuration](#configuration)
- [API Reference](#api-reference)
  - [POST /memories/compact](#post-memoriescompact)
- [Usage Examples](#usage-examples)
- [How It Works](#how-it-works)
- [Contributing](#contributing)
- [License](#license)

---

## Quickstart

### Option 1: Download prebuilt binary (fastest)

```bash
# Command 1: Download and extract (macOS Apple Silicon shown; see below for other platforms)
curl -L https://github.com/chrisesposito92/mnemonic/releases/latest/download/mnemonic-macos-aarch64.tar.gz | tar xz

# Command 2: Start the server
./mnemonic

# Command 3: Store your first memory
curl -s -X POST http://localhost:8080/memories \
  -H "Content-Type: application/json" \
  -d '{"content": "The user prefers dark mode", "agent_id": "my-agent"}'
```

**Other platforms:**
- Linux x86_64: `mnemonic-linux-x86_64.tar.gz`
- macOS Intel: `mnemonic-macos-x86_64.tar.gz`

### Option 2: Build from source

```bash
cargo install --git https://github.com/chrisesposito92/mnemonic
```

> Note: `cargo install mnemonic` will work after the first crates.io publish.

**First run:** On the first start, Mnemonic downloads the embedding model (~80 MB) from HuggingFace Hub and caches it at `~/.cache/huggingface/`. Subsequent starts are instant.

---

## Concepts

Understanding three key fields makes the API straightforward:

**`agent_id`** — A namespace string that isolates memories by agent. Use a unique identifier for each agent (e.g., `"research-bot"`, `"summarizer"`). Memories stored with one `agent_id` are not returned when searching with a different `agent_id`. Defaults to an empty string if omitted.

**`session_id`** — Groups memories within an agent by conversation or session. Useful for scoping retrieval to the current context window. Defaults to an empty string if omitted.

**`tags`** — An array of arbitrary string labels attached to a memory. Tags support substring filtering in search and list queries. Defaults to an empty array if omitted.

---

## Configuration

All configuration is optional. Mnemonic works with zero configuration.

| Variable | Default | Description |
|----------|---------|-------------|
| `MNEMONIC_PORT` | `8080` | TCP port to listen on |
| `MNEMONIC_DB_PATH` | `./mnemonic.db` | Path to SQLite database file |
| `MNEMONIC_EMBEDDING_PROVIDER` | `local` | Embedding provider: `local` or `openai` |
| `MNEMONIC_OPENAI_API_KEY` | — | OpenAI API key (required when `MNEMONIC_EMBEDDING_PROVIDER=openai`) |
| `MNEMONIC_CONFIG_PATH` | `./mnemonic.toml` | Path to optional TOML configuration file |
| `MNEMONIC_LLM_PROVIDER` | — | LLM provider for compaction summarization: `openai` (optional) |
| `MNEMONIC_LLM_API_KEY` | — | LLM API key (required when `MNEMONIC_LLM_PROVIDER` is set) |
| `MNEMONIC_LLM_BASE_URL` | — | Custom LLM API base URL (for OpenAI-compatible providers) |
| `MNEMONIC_LLM_MODEL` | — | LLM model name (defaults to provider's default) |

**Precedence:** env vars > TOML file > compiled defaults.

**TOML configuration example (`mnemonic.toml`):**

```toml
port = 9090
db_path = "/data/mnemonic.db"
embedding_provider = "local"
# openai_api_key = "sk-..."

# Optional: enable LLM-powered compaction summarization
# llm_provider = "openai"
# llm_api_key = "sk-..."
# llm_model = "gpt-4o-mini"
```

Set `MNEMONIC_CONFIG_PATH` to point to a different TOML file location.

---

## API Reference

### GET /health

Check server readiness.

**Response 200:**
```json
{"status": "ok"}
```

**curl:**
```bash
curl http://localhost:8080/health
```

---

### POST /memories

Store a new memory. The content is embedded and stored in SQLite for later retrieval.

**Request body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `content` | string | YES | — | The text to remember |
| `agent_id` | string | no | `""` | Agent namespace |
| `session_id` | string | no | `""` | Session/conversation group |
| `tags` | string[] | no | `[]` | Labels for filtering |

**Response 201:**
```json
{
  "id": "019506d2-1c3b-7a2e-8b4f-0a1b2c3d4e5f",
  "content": "The Eiffel Tower is 330 meters tall",
  "agent_id": "research-bot",
  "session_id": "session-42",
  "tags": ["landmarks", "paris"],
  "embedding_model": "all-MiniLM-L6-v2",
  "created_at": "2026-03-19 12:34:56",
  "updated_at": null
}
```

> `updated_at` is `null | string` — reserved for a future update endpoint.

**Error 400:**
```json
{"error": "content must not be empty"}
```

**curl:**
```bash
curl -s -X POST http://localhost:8080/memories \
  -H "Content-Type: application/json" \
  -d '{
    "content": "The Eiffel Tower is 330 meters tall",
    "agent_id": "research-bot",
    "session_id": "session-42",
    "tags": ["landmarks", "paris"]
  }'
```

---

### GET /memories/search

Semantically search memories using KNN vector search. Returns the most similar memories to the query.

**Query parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `q` | string | YES | — | Search query text |
| `agent_id` | string | no | — | Filter to this agent's memories |
| `session_id` | string | no | — | Filter to this session's memories |
| `tag` | string | no | — | Filter by tag (substring match) |
| `limit` | integer | no | `10` | Max results (capped at 100) |
| `threshold` | float | no | — | Max distance filter |
| `after` | string | no | — | ISO datetime lower bound for `created_at` |
| `before` | string | no | — | ISO datetime upper bound for `created_at` |

**Important:** `distance` is an L2 distance — **lower distance = more similar**. Results are ordered from most to least similar.

The `threshold` parameter filters out results with a distance higher than the specified value (i.e., less similar than the threshold).

**Response 200:**
```json
{
  "memories": [
    {
      "id": "019506d2-1c3b-7a2e-8b4f-0a1b2c3d4e5f",
      "content": "The Eiffel Tower is 330 meters tall",
      "agent_id": "research-bot",
      "session_id": "session-42",
      "tags": ["landmarks", "paris"],
      "embedding_model": "all-MiniLM-L6-v2",
      "created_at": "2026-03-19 12:34:56",
      "updated_at": null,
      "distance": 0.234
    }
  ]
}
```

**Error 400:**
```json
{"error": "q parameter is required"}
```

**curl:**
```bash
curl -s "http://localhost:8080/memories/search?q=tall+structures&agent_id=research-bot&limit=5"
```

---

### GET /memories

List memories with optional filters. Returns a paginated list without distance scores (use `/memories/search` for semantic retrieval).

**Query parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `agent_id` | string | no | — | Filter to this agent |
| `session_id` | string | no | — | Filter to this session |
| `tag` | string | no | — | Filter by tag (substring match) |
| `after` | string | no | — | ISO datetime lower bound for `created_at` |
| `before` | string | no | — | ISO datetime upper bound for `created_at` |
| `limit` | integer | no | `20` | Max results (capped at 100) |
| `offset` | integer | no | `0` | Pagination offset |

**Response 200:**
```json
{
  "memories": [
    {
      "id": "019506d2-1c3b-7a2e-8b4f-0a1b2c3d4e5f",
      "content": "The Eiffel Tower is 330 meters tall",
      "agent_id": "research-bot",
      "session_id": "session-42",
      "tags": ["landmarks", "paris"],
      "embedding_model": "all-MiniLM-L6-v2",
      "created_at": "2026-03-19 12:34:56",
      "updated_at": null
    }
  ],
  "total": 42
}
```

**curl:**
```bash
curl -s "http://localhost:8080/memories?agent_id=research-bot&limit=10"
```

---

### DELETE /memories/:id

Delete a memory by ID. Returns the deleted memory object.

**Path parameter:** `id` — UUID string of the memory to delete.

**Response 200:** Full Memory object of the deleted memory.
```json
{
  "id": "019506d2-1c3b-7a2e-8b4f-0a1b2c3d4e5f",
  "content": "The Eiffel Tower is 330 meters tall",
  "agent_id": "research-bot",
  "session_id": "session-42",
  "tags": ["landmarks", "paris"],
  "embedding_model": "all-MiniLM-L6-v2",
  "created_at": "2026-03-19 12:34:56",
  "updated_at": null
}
```

**Error 404:**
```json
{"error": "not found"}
```

**curl:**
```bash
curl -s -X DELETE http://localhost:8080/memories/019506d2-1c3b-7a2e-8b4f-0a1b2c3d4e5f
```

---

### POST /memories/compact

Compact an agent's memories by deduplicating similar entries. Uses vector similarity clustering to find near-duplicate memories and merges them. When an LLM is configured, merged clusters are summarized into rich consolidated memories (Tier 2); otherwise, content is concatenated (Tier 1).

**Request body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `agent_id` | string | YES | — | Agent whose memories to compact |
| `threshold` | float | no | `0.85` | Cosine similarity threshold for clustering (0.0-1.0, higher = stricter) |
| `max_candidates` | integer | no | `100` | Max memories to consider (caps O(n^2) clustering) |
| `dry_run` | boolean | no | `false` | Preview clusters without modifying data |

**Response 200:**
```json
{
  "clusters_found": 3,
  "memories_merged": 7,
  "memories_created": 3,
  "id_mapping": {
    "new-id-1": ["old-id-a", "old-id-b"],
    "new-id-2": ["old-id-c", "old-id-d", "old-id-e"],
    "new-id-3": ["old-id-f", "old-id-g"]
  },
  "dry_run": false
}
```

When `dry_run: true`, no data is modified — `memories_created` is `0` and `id_mapping` shows proposed clusters.

**Error 400:**
```json
{"error": "agent_id is required"}
```

**curl:**
```bash
# Compact memories for an agent
curl -s -X POST http://localhost:8080/memories/compact \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "research-bot"}'

# Preview what would be compacted (no changes made)
curl -s -X POST http://localhost:8080/memories/compact \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "research-bot", "dry_run": true}'

# Stricter threshold (only very similar memories merged)
curl -s -X POST http://localhost:8080/memories/compact \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "research-bot", "threshold": 0.95}'
```

---

## Usage Examples

### curl Workflow

A complete workflow demonstrating all operations:

```bash
# 1. Store a memory for research-bot
curl -s -X POST http://localhost:8080/memories \
  -H "Content-Type: application/json" \
  -d '{
    "content": "The Eiffel Tower is 330 meters tall, located in Paris",
    "agent_id": "research-bot",
    "tags": ["landmarks", "paris"]
  }'

# 2. Store another memory
curl -s -X POST http://localhost:8080/memories \
  -H "Content-Type: application/json" \
  -d '{
    "content": "The Burj Khalifa stands at 828 meters, the world'\''s tallest building",
    "agent_id": "research-bot",
    "tags": ["landmarks", "dubai"]
  }'

# 3. Search semantically — finds relevant memories even with different wording
curl -s "http://localhost:8080/memories/search?q=tall+buildings+and+towers&agent_id=research-bot"

# 4. List all memories for an agent (paginated)
curl -s "http://localhost:8080/memories?agent_id=research-bot&limit=20"

# 5. Delete a memory by ID
curl -s -X DELETE http://localhost:8080/memories/019506d2-1c3b-7a2e-8b4f-0a1b2c3d4e5f
```

---

### Python Client

A simple wrapper class using only the `requests` library:

```python
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

    def compact(self, agent_id, threshold=None, max_candidates=None, dry_run=False):
        payload = {"agent_id": agent_id, "dry_run": dry_run}
        if threshold is not None:  payload["threshold"] = threshold
        if max_candidates is not None: payload["max_candidates"] = max_candidates
        r = requests.post(f"{self.base_url}/memories/compact", json=payload)
        r.raise_for_status()
        return r.json()
```

**Basic usage:**

```python
client = MnemonicClient()

# Store memories
client.store("Python 3.12 introduces type parameter syntax", agent_id="research-bot", tags=["python"])

# Search by semantic similarity
results = client.search("Python language features", agent_id="research-bot")
for m in results:
    print(f"[{m['distance']:.3f}] {m['content']}")

# List recent memories
page = client.list(agent_id="research-bot", limit=10)
print(f"Total memories: {page['total']}")

# Delete a memory
client.delete(results[0]["id"])

# Compact similar memories (preview first, then apply)
preview = client.compact("research-bot", dry_run=True)
print(f"Would merge {preview['memories_merged']} memories into {preview['clusters_found']} clusters")

result = client.compact("research-bot")
print(f"Merged {result['memories_merged']} → {result['memories_created']} memories")
```

---

### Multi-Agent Example

Multiple agents sharing one Mnemonic instance, each with isolated memory:

```python
client = MnemonicClient()

# Each agent uses its own agent_id namespace
client.store("The Eiffel Tower is 330 meters tall", agent_id="research-bot")
client.store("Previous summary covered Paris landmarks", agent_id="summarizer")

# Search is scoped — only research-bot's memories returned
results = client.search("tall structures", agent_id="research-bot")
# → returns research-bot's Eiffel Tower memory; summarizer's memory is not included

# List memories per agent independently
research_memories = client.list(agent_id="research-bot")
summary_memories  = client.list(agent_id="summarizer")
```

---

### Tool-Use Example (Framework-Agnostic)

Define Mnemonic as tools for an LLM agent. This pattern works with any framework that supports tool/function calling:

```python
import requests

client = MnemonicClient()

# Tool schema definitions (compatible with OpenAI, Anthropic, and similar APIs)
MNEMONIC_TOOLS = [
    {
        "name": "store_memory",
        "description": "Store a piece of information for later retrieval",
        "parameters": {
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The information to remember"
                },
                "tags": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional labels for categorizing the memory"
                }
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
                "query": {
                    "type": "string",
                    "description": "What to search for"
                }
            },
            "required": ["query"]
        }
    }
]

# Tool execution dispatcher
def handle_tool_call(tool_name, args, agent_id):
    if tool_name == "store_memory":
        return client.store(args["content"], agent_id=agent_id, tags=args.get("tags"))
    elif tool_name == "search_memory":
        return client.search(args["query"], agent_id=agent_id)

# Usage — pass MNEMONIC_TOOLS to your LLM call, then dispatch tool calls:
# response = llm.complete(messages, tools=MNEMONIC_TOOLS)
# for tool_call in response.tool_calls:
#     result = handle_tool_call(tool_call.name, tool_call.args, agent_id="my-agent")
```

---

## How It Works

Mnemonic stores memories in a single SQLite file using [sqlite-vec](https://github.com/asg017/sqlite-vec) for vector similarity search. When you POST a memory, Mnemonic embeds the content using [all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2) (~22 MB), a compact but high-quality sentence embedding model, running locally via [candle](https://github.com/huggingface/candle) — a pure-Rust ML framework with no native library dependencies. The model is downloaded from HuggingFace Hub on first run and cached at `~/.cache/huggingface/`. When you search, your query is embedded with the same model and KNN vector search finds the closest memories by L2 distance. Optionally, you can switch to OpenAI `text-embedding-3-small` by setting `MNEMONIC_OPENAI_API_KEY` — no other configuration needed.

**Compaction** works in two tiers. **Tier 1 (default)** uses vector cosine similarity to cluster near-duplicate memories and merges them algorithmically — tags are unioned, timestamps take the earliest, and content is concatenated. No LLM required. **Tier 2 (opt-in)** activates when `MNEMONIC_LLM_PROVIDER` is configured — clustered memories are sent to the LLM for rich summarization instead of simple concatenation. If the LLM call fails, compaction falls back to Tier 1 automatically. All merges are atomic (single SQLite transaction) and scoped to the requesting agent — one agent's compaction never touches another agent's memories.

---

## Contributing

Contributions welcome. Please open an issue first to discuss significant changes.

**Development setup:**

```bash
git clone https://github.com/chrisesposito92/mnemonic
cd mnemonic
cargo build
cargo test
```

---

## License

MIT License — see [LICENSE](LICENSE) for details.
