# Feature Research

**Domain:** gRPC interface for agent memory server (Rust/tonic, v1.5 milestone)
**Researched:** 2026-03-22
**Confidence:** HIGH

## Scope Note

This research covers only the **new gRPC features for v1.5**. The existing REST API (9 endpoints), API key auth, CLI (7 subcommands), and pluggable storage backends (SQLite, Qdrant, Postgres) are already built and out of scope. The question is: what does a well-designed gRPC interface look like for hot-path memory operations — and what does the ecosystem expect?

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist in a gRPC memory service. Missing these means agents cannot adopt the gRPC path.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| `StoreMemory` unary RPC | Every memory API needs a write path; agents call this on every turn | LOW | Mirrors POST /memories; takes content, agent_id, session_id, tags in request message |
| `SearchMemories` unary RPC | Semantic search is the core value prop; must be gRPC-accessible | LOW | Takes query string + agent_id filter + limit; returns ranked list with scores |
| `ListMemories` unary RPC | Agents need to enumerate memories without semantic query (session replay, pagination) | LOW | Mirrors GET /memories with optional agent_id/session_id filters |
| `DeleteMemory` unary RPC | Cleanup is expected; sessions end and memories become stale | LOW | Takes memory_id; returns success or NOT_FOUND status |
| `HealthCheck` via `grpc.health.v1` | Load balancers, Kubernetes probes, and agent orchestrators use the standard health proto — not a custom service | LOW | `tonic-health` crate provides the standard `grpc.health.v1.Health` service out of the box |
| Dual-protocol server (REST + gRPC on separate ports) | Existing REST clients must not break; agents choose their protocol | MEDIUM | Separate port (e.g., 50051) is simpler than same-port multiplexing; PROJECT.md explicitly specifies separate port |
| Auth via gRPC metadata (authorization header) | Existing API keys must work over gRPC; agents cannot re-authenticate on protocol change | MEDIUM | Bearer token in gRPC metadata key `authorization` (lowercase, per HTTP/2 header convention); validated in tonic interceptor mirroring existing axum middleware |
| `google.protobuf.Timestamp` for time fields | Proto3 best practice; standard well-known type for `created_at`; do not use int64 epoch seconds | LOW | Requires `prost-types` crate; Qdrant uses the same convention |
| Canonical gRPC status codes | Callers expect NOT_FOUND (5), UNAUTHENTICATED (16), INVALID_ARGUMENT (3) — not raw HTTP codes or generic errors | LOW | Map StorageBackend errors to `tonic::Status` codes in service impl |
| Proto3 field stability guarantees | Agents build clients from the .proto file; field tag numbers must never be reused | LOW | Reserve deleted tags; use `optional` keyword for presence-tracked fields (proto3 v3.15+) |

### Differentiators (Competitive Advantage)

Features beyond baseline that give Mnemonic's gRPC interface an edge.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| `agent_id` and `session_id` in request messages (not metadata) | Namespacing is a first-class Mnemonic concept; putting routing fields in the proto body makes clients statically typed and explicit — metadata is stringly typed and easy to forget | LOW | Industry norm (Qdrant, Weaviate) puts collection/namespace in the request body, not metadata; same approach here |
| `score` in `SearchMemoriesResponse` per result | Agents need to threshold results; returning a float distance per result enables quality filtering client-side without round-trips | LOW | Already computed in storage layer (`lower_is_better` contract); just expose it in the proto message |
| `tonic-reflection` for service discoverability | gRPC reflection lets `grpcurl` and Postman introspect services without the .proto file — reduces agent developer friction considerably | LOW | Add `tonic-reflection` crate; can be behind a config flag to allow disabling in production |
| Optional TLS via rustls (config-driven) | Production deployments need encryption; dev mode should work without certs | MEDIUM | Tonic supports rustls natively (tls-ring or tls-aws-lc feature); expose `grpc_tls_cert_path` and `grpc_tls_key_path` in Config; if not set, server starts without TLS |
| `tags` repeated field on `StoreMemoryRequest` | Mirrors REST behavior exactly; agents can tag memories at write time with zero additional calls | LOW | Repeated string field in proto; no complexity |
| `storage_backend` in `HealthCheckResponse` | Operators want to know which backend is active — already reported in REST /health; gRPC health response should match | LOW | Return the backend name string alongside ServingStatus via extended `HealthCheckResponse` or a separate custom health RPC |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| gRPC streaming for search results | Looks like a natural fit for paginated result delivery | Streaming adds connection-lifecycle complexity; agents use small result sets (k=10-20 typical); unary is simpler to implement, debug, and test; PROJECT.md explicitly scopes v1.5 to unary only | Unary RPC with `limit` field; callers paginate via subsequent calls with offset if needed |
| Same-port HTTP/REST + gRPC multiplexing | Simplifies firewall rules (one port exposed) | Requires a `HybridService` tower wrapper that inspects `Content-Type: application/grpc` on every request; adds complexity and latency; debugging split-protocol traffic is harder; community guidance favors separate ports | Separate port (e.g., 50051 for gRPC, existing for REST); cleaner separation, consistent with Qdrant (6333 REST, 6334 gRPC) and standard production practice |
| gRPC for compaction and key management | "Everything should be gRPC" | Hot-path only was the explicit v1.5 scope decision in PROJECT.md; compaction is admin-tier (infrequent, latency-tolerant); key management is auth-tier (CLI-driven); expanding scope balloons the proto surface area and implementation cost | Keep compaction and key management REST-only; document this explicitly in proto file comments |
| Bidirectional streaming for live memory updates | Agents could subscribe to memory changes in real time | Complex server-side state management; no existing event/pub-sub system in Mnemonic; agent-memory access patterns are pull-based (request-response), not push-based | Polling via `ListMemories` is sufficient for current agent patterns; streaming is a v2+ consideration if event infrastructure is added |
| gRPC-Web support | Browser clients using gRPC | Requires `tonic-web` crate and CORS configuration; Mnemonic targets server-side agents, not browsers; REST API already covers browser use cases | REST API handles any browser or non-gRPC client |
| Client-side load balancing / name resolution | Enterprise multi-instance patterns | Tonic supports DNS SRV, but it requires infrastructure most Mnemonic users don't have; adds complexity disproportionate to target user (single-server deployments) | Single-server deployment; agents connect to one configured endpoint; load balancing at the infrastructure layer (nginx, AWS ALB) if needed |

---

## Feature Dependencies

```
[gRPC server process]
    └──requires──> [tokio runtime] (already present — shared with REST server)
    └──requires──> [tonic crate + tonic-build codegen in build.rs]
    └──requires──> [prost + prost-types for proto3 messages and google.protobuf.Timestamp]
    └──requires──> [.proto file defining MnemonicService]

[StoreMemory RPC]
    └──requires──> [MemoryService via Arc<dyn StorageBackend>] (present since v1.4)
    └──requires──> [EmbeddingEngine for vector generation] (present since v1.0)
    └──shares──> [Arc<Mutex<LocalEngineInner>>] (must NOT instantiate a second embedding model)

[SearchMemories RPC]
    └──requires──> [EmbeddingEngine] (present since v1.0)
    └──requires──> [StorageBackend.search()] (present since v1.4)

[ListMemories RPC]
    └──requires──> [StorageBackend.list()] (present since v1.4)
    └──BLOCKED BY──> [v1.4 tech debt: recall CLI bypasses StorageBackend trait]
    └──prerequisite: recall routing fix must land before ListMemories RPC]

[DeleteMemory RPC]
    └──requires──> [StorageBackend.delete()] (present since v1.4)

[HealthCheck via grpc.health.v1]
    └──requires──> [tonic-health crate]
    └──enhances──> [existing GET /health REST endpoint] (parallel, not replacing)

[Auth interceptor for gRPC]
    └──requires──> [KeyService on Arc<Connection>] (present since v1.2, stays SQLite-only)
    └──mirrors──> [axum auth middleware] (same validation logic, different entry point)
    └──convention: reads gRPC metadata key "authorization" value "Bearer mnk_..."]

[gRPC server configuration]
    └──requires──> [Config struct extension: grpc_port, grpc_tls_cert_path, grpc_tls_key_path]
    └──requires──> [separate tokio::spawn task for gRPC server alongside REST server]

[Optional TLS]
    └──requires──> [rustls feature flag in tonic dependency]
    └──requires──> [grpc_tls_cert_path / grpc_tls_key_path config fields]
    └──enhances──> [gRPC server startup]

[tonic-reflection]
    └──enhances──> [gRPC server] (optional discoverability, not required for operation)
    └──conflicts with──> [minimizing binary size] (adds generated descriptor bytes — minor, worth flagging)
```

### Dependency Notes

- **ListMemories is blocked by a tech debt prerequisite.** The v1.3 recall CLI bypasses the StorageBackend trait, accessing SQLite directly. Before exposing ListMemories via gRPC, the underlying list call must be normalized through the trait so it works across all three backends. PROJECT.md explicitly names this as a v1.5 prerequisite.
- **Embedding model must be shared, not duplicated.** The gRPC server and REST server must share the same `Arc<Mutex<LocalEngineInner>>`. If the gRPC server initializes its own embedding engine, cold start doubles (two 2-3s model loads) and memory doubles. AppState must be extended to pass the shared engine to both servers.
- **Auth interceptor mirrors axum middleware.** Both validate the same `mnk_...` bearer tokens from the same SQLite key store. The gRPC interceptor reads the `authorization` metadata key (lowercase, per gRPC HTTP/2 header conventions). The validation logic should be extracted into a shared function callable from both the axum middleware and the tonic interceptor.
- **tonic v0.14.5 is the stable production release.** The GitHub master branch is preparing breaking changes as of March 2026; pin to `0.14.x` in Cargo.toml.

---

## MVP Definition

### Launch With (v1.5)

The five RPCs specified in PROJECT.md as the v1.5 target, plus their prerequisites.

- [ ] StorageBackend routing fix for `list` (v1.4 tech debt — prerequisite for ListMemories)
- [ ] `.proto` file defining `MnemonicService` with all five operations
- [ ] `tonic-build` codegen in `build.rs`
- [ ] `StoreMemory` unary RPC — core write path; agents cannot use gRPC without it
- [ ] `SearchMemories` unary RPC — core semantic read path; primary hot-path operation
- [ ] `ListMemories` unary RPC — list/filter without semantic query; session recall
- [ ] `DeleteMemory` unary RPC — cleanup path; completes CRUD
- [ ] `HealthCheck` via `grpc.health.v1` — required for load balancers and orchestrators
- [ ] Auth via gRPC metadata interceptor — security parity with REST
- [ ] gRPC server on separate port with config (`grpc_port`, optional TLS config fields)
- [ ] Shared embedding engine between REST and gRPC servers (no second model load)

### Add After Validation (v1.5.x)

- [ ] `tonic-reflection` for grpcurl discoverability — add when agent developer adoption is measured; currently adds binary size for marginal benefit at launch
- [ ] Optional TLS (rustls) — add when any production user requests it; current users are localhost or VPN-connected

### Future Consideration (v2+)

- [ ] Server-streaming `SearchMemoriesStream` — defer until agents demonstrably need >50 results per call
- [ ] Bidirectional streaming for memory subscriptions — defer until event system exists
- [ ] gRPC for compaction — defer; admin operations don't need low-latency binary protocol

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| StorageBackend routing fix (tech debt) | HIGH | LOW | P1 — blocks ListMemories |
| `.proto` file + `tonic-build` codegen | HIGH | LOW | P1 — foundation for all RPCs |
| `StoreMemory` RPC | HIGH | LOW | P1 |
| `SearchMemories` RPC | HIGH | LOW | P1 |
| `ListMemories` RPC | HIGH | LOW | P1 |
| `DeleteMemory` RPC | MEDIUM | LOW | P1 |
| `HealthCheck` (grpc.health.v1) | HIGH | LOW | P1 |
| Auth interceptor | HIGH | MEDIUM | P1 |
| gRPC port + config | HIGH | LOW | P1 |
| Shared embedding engine across servers | HIGH | LOW | P1 — correctness, not optional |
| `score` in SearchResponse | MEDIUM | LOW | P1 — already in storage layer |
| `google.protobuf.Timestamp` fields | MEDIUM | LOW | P1 — proto best practice |
| Canonical status codes | MEDIUM | LOW | P1 |
| Optional TLS (rustls) | MEDIUM | MEDIUM | P2 |
| `tonic-reflection` | LOW | LOW | P2 |
| Streaming RPCs | LOW | HIGH | P3 |
| Same-port multiplexing | LOW | HIGH | P3 |

**Priority key:**
- P1: Must have for v1.5 launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

---

## Proto Message Shape Reference

Based on ecosystem research (Qdrant gRPC proto, Weaviate proto patterns, Mnemosyne MemoryService, and proto3 best practices), here are the recommended message shapes for the Mnemonic proto file.

### Service Definition Sketch

```proto
syntax = "proto3";
package mnemonic.v1;

import "google/protobuf/timestamp.proto";

service MnemonicService {
  rpc StoreMemory(StoreMemoryRequest) returns (StoreMemoryResponse);
  rpc SearchMemories(SearchMemoriesRequest) returns (SearchMemoriesResponse);
  rpc ListMemories(ListMemoriesRequest) returns (ListMemoriesResponse);
  rpc DeleteMemory(DeleteMemoryRequest) returns (DeleteMemoryResponse);
}
// Health: use the standard grpc.health.v1.Health service (tonic-health crate)
// Compaction and key management: REST only, not exposed via gRPC
```

### Key Message Shapes

```proto
message StoreMemoryRequest {
  string content = 1;              // required — validate non-empty before embedding
  string agent_id = 2;             // required — namespace isolation
  optional string session_id = 3;  // optional presence-tracked (proto3 optional keyword)
  repeated string tags = 4;        // optional tagging, zero or more values
}

message StoreMemoryResponse {
  string id = 1;                   // UUID of stored memory
}

message SearchMemoriesRequest {
  string query = 1;                // required — semantic search text
  string agent_id = 2;             // required — namespace filter
  optional string session_id = 3;  // optional narrowing filter
  uint32 limit = 4;                // 0 = use server default (e.g., 10)
}

message MemoryResult {
  string id = 1;
  string content = 2;
  string agent_id = 3;
  optional string session_id = 4;
  repeated string tags = 5;
  float score = 6;                 // lower = more similar (normalized cosine distance)
  google.protobuf.Timestamp created_at = 7;
}

message SearchMemoriesResponse {
  repeated MemoryResult memories = 1;
}

message ListMemoriesRequest {
  optional string agent_id = 1;   // filter by agent_id; if absent, returns all (admin use)
  optional string session_id = 2;
  uint32 limit = 3;               // 0 = use server default
  uint32 offset = 4;              // for pagination
}

message ListMemoriesResponse {
  repeated MemoryResult memories = 1;
}

message DeleteMemoryRequest {
  string id = 1;                  // memory UUID
}

message DeleteMemoryResponse {
  bool deleted = 1;               // true if deleted, false if not found (or return NOT_FOUND status)
}
```

### Auth Convention

Token passed as gRPC metadata key `authorization` with value `Bearer mnk_...` (lowercase key, matching HTTP/2 header conventions that gRPC is built on). Server-side tonic interceptor reads `request.metadata().get("authorization")`. Status `UNAUTHENTICATED` (16) returned when token is missing or malformed; `PERMISSION_DENIED` (7) returned when token is valid but the token's authorized `agent_id` does not match the request's `agent_id` field.

### Status Code Mapping

| Condition | gRPC Status Code |
|-----------|-----------------|
| Memory not found by ID | NOT_FOUND (5) |
| Missing or invalid auth token | UNAUTHENTICATED (16) |
| Token valid but wrong agent namespace | PERMISSION_DENIED (7) |
| Empty content or query string | INVALID_ARGUMENT (3) |
| Storage backend error | INTERNAL (13) |
| Embedding model overloaded | RESOURCE_EXHAUSTED (8) |

---

## Competitor / Reference Analysis

| Feature | Qdrant gRPC | Weaviate gRPC | Mnemosyne gRPC | Mnemonic v1.5 Approach |
|---------|-------------|---------------|----------------|------------------------|
| Protocol | proto3, port 6334 | proto3, stable v1.23.7 | proto3 | proto3, configurable port (default 50051) |
| Health check | Custom service | Not documented | Dedicated HealthService | Standard `grpc.health.v1` via tonic-health |
| Auth | API key in metadata | API key in metadata | Not documented | Bearer `mnk_...` in `authorization` metadata |
| Namespace isolation | Collection name in request body | Class name in request body | Not documented | `agent_id` field in every request message |
| Timestamps | google.protobuf.Timestamp | google.protobuf.Timestamp | Not documented | google.protobuf.Timestamp |
| Streaming | Batch upsert streaming | Batch search streaming | RecallStream, ListMemoriesStream | None (unary only for v1.5) |
| Reflection | Available | Not documented | Not documented | Optional via tonic-reflection (P2) |
| TLS | Supported | Supported | Not documented | Optional via rustls config (P2) |
| Separate vs shared port | Separate (6333/6334) | Separate | Separate | Separate port (simpler, consistent with ecosystem) |

---

## Sources

- [gRPC official docs — metadata](https://grpc.io/docs/guides/metadata/) — metadata conventions, authorization header patterns. HIGH confidence.
- [gRPC official docs — authentication](https://grpc.io/docs/guides/auth/) — bearer token passing, interceptor-based validation. HIGH confidence.
- [gRPC official docs — status codes](https://grpc.io/docs/guides/status-codes/) — canonical status code definitions and when to use each. HIGH confidence.
- [gRPC official docs — health checking](https://grpc.io/docs/guides/health-checking/) — standard grpc.health.v1 protocol, Check vs Watch RPCs. HIGH confidence.
- [tonic GitHub (v0.14.5, Feb 2026)](https://github.com/hyperium/tonic) — current stable version, rustls TLS, tonic-health, tonic-reflection. HIGH confidence.
- [tonic-health crate docs](https://docs.rs/tonic-health) — standard grpc.health.v1 implementation for tonic. HIGH confidence.
- [Proto3 best practices (protobuf.dev)](https://protobuf.dev/best-practices/dos-donts/) — never reuse tag numbers, reserve deleted fields, use google.protobuf.Timestamp, make all fields optional. HIGH confidence.
- [Qdrant gRPC API services (DeepWiki)](https://deepwiki.com/qdrant/qdrant/9.2-grpc-api-services) — UpsertPoints, SearchPoints, GetPoints, DeletePoints message shapes. MEDIUM confidence.
- [Weaviate gRPC API docs](https://docs.weaviate.io/weaviate/api/grpc) — proto file organization, batch.proto, search_get.proto, base.proto pattern. MEDIUM confidence.
- [Axum + Tonic hybrid service pattern](https://academy.fpblock.com/blog/axum-hyper-tonic-tower-part4/) — Content-Type inspection for same-port routing; confirms separate port is simpler. MEDIUM confidence.
- [Mnemosyne gRPC memory service](https://rand.github.io/mnemosyne/) — MemoryService with 13 methods including streaming; HealthService pattern. MEDIUM confidence.
- [gRPC basics for Rust developers (DockYard, 2025)](https://dockyard.com/blog/2025/04/08/grpc-basics-for-rust-developers) — tonic setup, build.rs codegen, async/await patterns. MEDIUM confidence.

---
*Feature research for: Mnemonic v1.5 gRPC Interface*
*Researched: 2026-03-22*
