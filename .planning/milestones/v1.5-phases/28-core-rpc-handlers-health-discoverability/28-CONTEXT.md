# Phase 28: Core RPC Handlers, Health, and Discoverability - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement all four gRPC hot-path RPCs (StoreMemory, SearchMemories, ListMemories, DeleteMemory) with correct status codes and scope enforcement, wire tonic-health as SERVING, and enable tonic-reflection for grpcurl discoverability. The proto contract (Phase 26) and dual-server skeleton + auth layer (Phase 27) are already in place.

</domain>

<decisions>
## Implementation Decisions

### Error mapping (ApiError to tonic::Status)
- **D-01:** Direct mapping from ApiError variants to tonic status codes: BadRequest -> InvalidArgument, NotFound -> NotFound, Forbidden -> PermissionDenied, Unauthorized -> Unauthenticated, Internal -> Internal
- **D-02:** Create a helper function `api_error_to_status(e: ApiError) -> tonic::Status` in grpc/mod.rs (not a trait impl) — keeps gRPC-specific mapping inside the grpc module, avoids polluting the error module with tonic dependency
- **D-03:** Error messages pass through — the tonic::Status message contains the same user-facing string as the ApiError (e.g., "content must not be empty")

### AuthContext extraction from gRPC requests
- **D-04:** Extract AuthContext from `request.extensions()` — the GrpcAuthLayer (Phase 27) injects AuthContext into http::Request extensions, and tonic::Request exposes these via `.extensions()`
- **D-05:** Each handler extracts `Option<&AuthContext>` — None means open mode (no auth), Some means auth is active. Mirrors the REST `Option<Extension<AuthContext>>` pattern exactly.

### Scope enforcement
- **D-06:** Reuse the existing `enforce_scope()` logic by moving it from server.rs to a shared location (auth.rs) or duplicating a small gRPC-local version in grpc/mod.rs — the function is 13 lines, duplication is acceptable if moving creates churn
- **D-07:** Every handler calls enforce_scope before delegating to MemoryService — not type-enforced, covered by per-handler integration tests (per STATE.md critical research flag)
- **D-08:** Delete handler follows REST pattern: scoped key requires DB lookup of memory's agent_id before deletion (same ownership verification as REST delete_memory_handler)

### Timestamp conversion
- **D-09:** Parse created_at ISO 8601 string to prost_types::Timestamp using chrono or manual parsing — created_at is stored as "YYYY-MM-DDTHH:MM:SS.fffZ" format in the DB
- **D-10:** If parsing fails, fall back to Timestamp { seconds: 0, nanos: 0 } rather than erroring — a missing timestamp should not prevent returning the memory

### Memory type conversion
- **D-11:** Create a `fn memory_to_proto(m: service::Memory) -> proto::Memory` helper in grpc/mod.rs that converts the service-layer Memory struct to the proto-generated Memory message
- **D-12:** proto3 defaults apply: empty session_id becomes "" (not optional), empty tags becomes empty repeated field — matches the proto design comment (line 18-19 of mnemonic.proto)

### Reflection wiring
- **D-13:** Add FILE_DESCRIPTOR_SET const via tonic-build's `file_descriptor_set_path` in build.rs, include_bytes! in grpc/mod.rs
- **D-14:** Wire tonic-reflection in serve_grpc() via `add_service(tonic_reflection::server::Builder::configure().register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET).build_v1()?)` — makes `grpcurl list` work

### Health service
- **D-15:** tonic-health is already wired in serve_grpc() (Phase 27) — Phase 28 only needs to verify it responds correctly. No new health code needed.

### Handler input validation
- **D-16:** StoreMemory: reject empty content with InvalidArgument (mirrors REST)
- **D-17:** SearchMemories: reject empty query with InvalidArgument (mirrors REST)
- **D-18:** ListMemories: no required fields — empty agent_id returns all memories (matches REST GET /memories behavior)
- **D-19:** DeleteMemory: reject empty id with InvalidArgument; non-existent id returns NotFound

### Claude's Discretion
- Exact function signatures and argument ordering in handler implementations
- Whether to use a separate `handlers.rs` submodule under grpc/ or keep handlers in grpc/mod.rs
- Test structure — integration tests vs unit tests per handler
- Whether to add tracing spans per handler (recommended but not required)

</decisions>

<specifics>
## Specific Ideas

- Handlers should delegate to the existing MemoryService methods (create_memory, search_memories, list_memories, delete_memory) — no duplicate business logic
- The gRPC handlers must produce identical results to the REST handlers for the same inputs (same memories, same ordering, same distance values)
- Per STATE.md critical research flag: "Every gRPC handler MUST call enforce_scope(auth_ctx, agent_id). This is not type-enforced. Add per-handler integration test asserting Code::PermissionDenied for mismatched agent_id."

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Proto contract
- `proto/mnemonic.proto` -- All 4 RPC definitions, request/response message shapes, Memory type with field numbers

### gRPC skeleton (Phase 27)
- `src/grpc/mod.rs` -- MnemonicGrpcService struct (fields), stub trait impl, serve_grpc() with health + auth layer
- `src/grpc/auth.rs` -- GrpcAuthLayer/GrpcAuthService, AuthContext injection into extensions

### Service layer (business logic to delegate to)
- `src/service.rs` -- MemoryService methods: create_memory, search_memories, list_memories, get_memory_agent_id, delete_memory
- `src/service.rs` lines 56-66 -- Memory struct (service-layer) that must be converted to proto::Memory

### Auth and scope enforcement
- `src/auth.rs` lines 30-35 -- AuthContext struct definition (key_id, allowed_agent_id)
- `src/server.rs` lines 78-95 -- enforce_scope() function to reuse or mirror

### Error types
- `src/error.rs` -- ApiError variants (BadRequest, NotFound, Forbidden, Unauthorized, Internal) and their HTTP mappings

### Requirements
- `.planning/REQUIREMENTS.md` lines 19-29 -- GRPC-01 through GRPC-05, HEALTH-01, HEALTH-02

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `MemoryService` (src/service.rs): All four operations already exist as async methods — gRPC handlers are thin wrappers
- `enforce_scope()` (src/server.rs:78-95): 13-line function for scope enforcement — can be extracted or duplicated
- `AuthContext` (src/auth.rs:32-35): Already injected by GrpcAuthLayer into request extensions
- `MnemonicGrpcService` (src/grpc/mod.rs:19-25): Struct already holds Arc<MemoryService>, Arc<KeyService>, backend_name

### Established Patterns
- REST handlers extract `Option<Extension<AuthContext>>` then call `enforce_scope()` — gRPC handlers should mirror this with `request.extensions().get::<AuthContext>()`
- REST delete handler does ownership DB lookup before deletion for scoped keys — gRPC delete must replicate
- All REST handlers that take agent_id apply effective scope override before calling MemoryService
- ApiError variants map cleanly to both HTTP status codes and gRPC status codes

### Integration Points
- `MnemonicGrpcService` trait impl in `src/grpc/mod.rs` — replace 4 stub methods with real implementations
- `serve_grpc()` in `src/grpc/mod.rs` — add tonic-reflection service registration
- `build.rs` — add `file_descriptor_set_path` for reflection support

</code_context>

<deferred>
## Deferred Ideas

- gRPC streaming for SearchMemories (STREAM-01) — future requirement, unary sufficient for v1.5
- Compaction/keys over gRPC (GRPC-EXT-01) — explicitly out of scope per PROJECT.md

</deferred>

---

*Phase: 28-core-rpc-handlers-health-discoverability*
*Context gathered: 2026-03-22*
