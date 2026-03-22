# Requirements: Mnemonic

**Defined:** 2026-03-22
**Core Value:** Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run

## v1.5 Requirements

Requirements for gRPC milestone. Each maps to roadmap phases.

### Proto & Codegen

- [ ] **PROTO-01**: Proto service definition (mnemonic.proto) with MnemonicService containing StoreMemory, SearchMemories, ListMemories, DeleteMemory RPCs and corresponding request/response messages
- [ ] **PROTO-02**: tonic-build integration in build.rs with explicit rerun-if-changed path to prevent always-dirty builds
- [ ] **PROTO-03**: CI release workflow updated with protoc installation step for all build targets
- [ ] **PROTO-04**: All gRPC dependencies (tonic, prost, tonic-build, tonic-health, tonic-reflection) feature-gated behind `interface-grpc` flag

### gRPC Service Handlers

- [ ] **GRPC-01**: StoreMemory RPC accepts content, agent_id, session_id, tags and returns stored memory with ID
- [ ] **GRPC-02**: SearchMemories RPC accepts query, agent_id, optional session_id/tags, limit and returns ranked results with distances
- [ ] **GRPC-03**: ListMemories RPC accepts agent_id, optional session_id/tags, limit/offset and returns memory list
- [ ] **GRPC-04**: DeleteMemory RPC accepts memory ID and returns success/not-found status
- [ ] **GRPC-05**: Consistent gRPC status code mapping (INVALID_ARGUMENT for bad input, NOT_FOUND for missing memory, UNAUTHENTICATED/PERMISSION_DENIED for auth, INTERNAL for server errors)

### Health & Discoverability

- [ ] **HEALTH-01**: grpc.health.v1 standard health service via tonic-health reporting SERVING status
- [ ] **HEALTH-02**: tonic-reflection enabled for grpcurl/grpc_cli service discovery

### Auth & Security

- [ ] **AUTH-01**: Bearer token auth via gRPC `authorization` metadata key using async Tower Layer (not sync interceptor)
- [ ] **AUTH-02**: Agent scope enforcement — gRPC handlers enforce agent_id matches API key scope (same logic as REST)
- [ ] **AUTH-03**: Open mode bypass — no auth required when zero API keys exist (consistent with REST behavior)

### Server & Config

- [ ] **SERVER-01**: Dual-port startup — REST on existing port, gRPC on configurable grpc_port via tokio::try_join!
- [ ] **SERVER-02**: grpc_port configuration field in Config struct (env var MNEMONIC_GRPC_PORT + TOML grpc_port)
- [ ] **SERVER-03**: Shared AppState — gRPC server shares Arc<MemoryService>, Arc<KeyService>, Arc<EmbeddingEngine> with REST server

### Tech Debt

- [ ] **DEBT-01**: recall CLI routes all operations through StorageBackend trait instead of raw SQLite (fixes v1.4 known gap)

## Future Requirements

Deferred to future release. Tracked but not in current roadmap.

### gRPC Extensions

- **STREAM-01**: Server-streaming SearchMemories for large result sets
- **STREAM-02**: Bidirectional streaming for real-time memory change notifications
- **TLS-01**: Native TLS termination on gRPC port (grpc_tls_cert, grpc_tls_key config)
- **SDK-01**: Generated gRPC client SDKs (Python, TypeScript) from proto definitions
- **GRPC-EXT-01**: Compact and key management RPCs (currently REST-only)

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Same-port multiplexing | Documented body-type mismatch bugs (tonic #1964); dual-port is correct |
| gRPC-web support | Adds GrpcWebLayer complexity; agents use native gRPC clients, not browsers |
| Client-side load balancing | Agents connect directly; LB is infrastructure concern |
| gRPC streaming (v1.5) | Unary RPCs sufficient for all current hot-path operations |
| TLS termination (v1.5) | Users can terminate TLS externally via reverse proxy; simplifies initial scope |
| Compaction/keys over gRPC | Low-frequency operations; REST-only is sufficient |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| PROTO-01 | Phase 26 | Pending |
| PROTO-02 | Phase 26 | Pending |
| PROTO-03 | Phase 26 | Pending |
| PROTO-04 | Phase 26 | Pending |
| GRPC-01 | Phase 28 | Pending |
| GRPC-02 | Phase 28 | Pending |
| GRPC-03 | Phase 28 | Pending |
| GRPC-04 | Phase 28 | Pending |
| GRPC-05 | Phase 28 | Pending |
| HEALTH-01 | Phase 28 | Pending |
| HEALTH-02 | Phase 28 | Pending |
| AUTH-01 | Phase 27 | Pending |
| AUTH-02 | Phase 27 | Pending |
| AUTH-03 | Phase 27 | Pending |
| SERVER-01 | Phase 27 | Pending |
| SERVER-02 | Phase 27 | Pending |
| SERVER-03 | Phase 27 | Pending |
| DEBT-01 | Phase 29 | Pending |

**Coverage:**
- v1.5 requirements: 18 total
- Mapped to phases: 18
- Unmapped: 0

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after roadmap creation*
