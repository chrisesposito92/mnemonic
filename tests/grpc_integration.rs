//! gRPC integration tests for all four handlers, scope enforcement, health, and reflection.
//!
//! Tests run only when the `interface-grpc` feature is enabled:
//!   cargo test --features interface-grpc test_grpc_
//!
//! Handlers are called directly via the MnemonicService trait — no TCP listener needed.
//! AuthContext is injected into request extensions to simulate GrpcAuthLayer behavior.

#![cfg(feature = "interface-grpc")]

use std::sync::Arc;

use mnemonic::grpc::proto;
use mnemonic::grpc::proto::mnemonic_service_server::MnemonicService;
use mnemonic::grpc::MnemonicGrpcService;
use mnemonic::storage::{StorageBackend, SqliteBackend};

// ── MockEmbeddingEngine ────────────────────────────────────────────────────────

/// Deterministic 384-dim embedding engine based on text hashing.
/// Mirrors the MockEmbeddingEngine in tests/integration.rs — no model download required.
struct MockEmbeddingEngine;

#[async_trait::async_trait]
impl mnemonic::embedding::EmbeddingEngine for MockEmbeddingEngine {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, mnemonic::error::EmbeddingError> {
        if text.is_empty() {
            return Err(mnemonic::error::EmbeddingError::EmptyInput);
        }
        let mut embedding = vec![0.0f32; 384];
        let bytes = text.as_bytes();
        for (i, slot) in embedding.iter_mut().enumerate() {
            let mut hash: u32 = 5381;
            for &b in bytes {
                hash = hash.wrapping_mul(33).wrapping_add(b as u32);
            }
            hash = hash.wrapping_mul(31).wrapping_add(i as u32);
            *slot = (hash as f32 % 1000.0) / 1000.0;
        }
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in embedding.iter_mut() {
                *v /= norm;
            }
        }
        Ok(embedding)
    }
}

// ── Test harness helpers ───────────────────────────────────────────────────────

/// Creates a MnemonicGrpcService backed by in-memory SQLite and MockEmbeddingEngine.
/// No model download or network access needed.
async fn test_grpc_service() -> MnemonicGrpcService {
    mnemonic::db::register_sqlite_vec();
    let config = mnemonic::config::Config {
        port: 0,
        db_path: ":memory:".to_string(),
        embedding_provider: "local".to_string(),
        openai_api_key: None,
        ..Default::default()
    };
    let conn = Arc::new(mnemonic::db::open(&config).await.unwrap());
    let key_service = Arc::new(mnemonic::auth::KeyService::new(Arc::clone(&conn)));
    let embedding: Arc<dyn mnemonic::embedding::EmbeddingEngine> = Arc::new(MockEmbeddingEngine);
    let backend: Arc<dyn StorageBackend> = Arc::new(SqliteBackend::new(Arc::clone(&conn)));
    let memory_service = Arc::new(mnemonic::service::MemoryService::new(
        Arc::clone(&backend),
        Arc::clone(&embedding),
        "mock-model".to_string(),
    ));
    let compaction_service = Arc::new(mnemonic::compaction::CompactionService::new(
        Arc::clone(&backend),
        Arc::clone(&conn),
        Arc::clone(&embedding),
        None,
        "mock-model".to_string(),
    ));
    MnemonicGrpcService {
        memory_service,
        key_service,
        compaction_service,
        backend_name: "sqlite".to_string(),
    }
}

/// Creates a tonic::Request with AuthContext injected into extensions.
/// Simulates what GrpcAuthLayer does for authenticated requests.
fn request_with_auth<T>(msg: T, auth: mnemonic::auth::AuthContext) -> tonic::Request<T> {
    let mut req = tonic::Request::new(msg);
    req.extensions_mut().insert(auth);
    req
}

// ── Task 1: Handler happy-path and error-case tests ────────────────────────────

/// GRPC-01: StoreMemory happy path returns memory with non-empty ID.
#[tokio::test]
async fn test_grpc_store_memory() {
    let svc = test_grpc_service().await;
    let req = tonic::Request::new(proto::StoreMemoryRequest {
        content: "test memory content".to_string(),
        agent_id: "agent-1".to_string(),
        session_id: "sess-1".to_string(),
        tags: vec!["tag1".to_string()],
    });
    let resp = svc.store_memory(req).await.unwrap();
    let memory = resp.into_inner().memory.unwrap();
    assert!(!memory.id.is_empty(), "stored memory must have non-empty ID");
    assert_eq!(memory.content, "test memory content");
    assert_eq!(memory.agent_id, "agent-1");
}

/// GRPC-05 (validation): StoreMemory with whitespace-only content returns InvalidArgument.
#[tokio::test]
async fn test_grpc_store_memory_empty_content() {
    let svc = test_grpc_service().await;
    let req = tonic::Request::new(proto::StoreMemoryRequest {
        content: "   ".to_string(),
        agent_id: "agent-1".to_string(),
        ..Default::default()
    });
    let err = svc.store_memory(req).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(
        err.message().contains("content"),
        "error message should mention content, got: {}",
        err.message()
    );
}

/// GRPC-02: SearchMemories happy path returns ranked results with float distance.
#[tokio::test]
async fn test_grpc_search_memories() {
    let svc = test_grpc_service().await;
    // Store a memory first
    let store_req = tonic::Request::new(proto::StoreMemoryRequest {
        content: "The quick brown fox jumps over the lazy dog".to_string(),
        agent_id: "agent-1".to_string(),
        ..Default::default()
    });
    svc.store_memory(store_req).await.unwrap();

    // Search for it
    let search_req = tonic::Request::new(proto::SearchMemoriesRequest {
        query: "quick fox".to_string(),
        agent_id: "agent-1".to_string(),
        limit: 10,
        ..Default::default()
    });
    let resp = svc.search_memories(search_req).await.unwrap();
    let results = resp.into_inner().results;
    assert!(!results.is_empty(), "search must return at least one result");
    assert!(results[0].memory.is_some(), "search result must contain memory");
    assert!(results[0].distance >= 0.0, "distance must be non-negative");
}

/// GRPC-05 (validation): SearchMemories with empty query returns InvalidArgument.
#[tokio::test]
async fn test_grpc_search_memories_empty_query() {
    let svc = test_grpc_service().await;
    let req = tonic::Request::new(proto::SearchMemoriesRequest {
        query: "".to_string(),
        ..Default::default()
    });
    let err = svc.search_memories(req).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

/// GRPC-03: ListMemories happy path returns memories and total count.
#[tokio::test]
async fn test_grpc_list_memories() {
    let svc = test_grpc_service().await;
    // Store a memory
    let store_req = tonic::Request::new(proto::StoreMemoryRequest {
        content: "list test memory".to_string(),
        agent_id: "agent-list".to_string(),
        ..Default::default()
    });
    svc.store_memory(store_req).await.unwrap();

    let list_req = tonic::Request::new(proto::ListMemoriesRequest {
        agent_id: "agent-list".to_string(),
        limit: 10,
        ..Default::default()
    });
    let resp = svc.list_memories(list_req).await.unwrap();
    let inner = resp.into_inner();
    assert!(!inner.memories.is_empty(), "list must return stored memory");
    assert!(inner.total > 0, "total must be positive");
}

/// GRPC-04: DeleteMemory happy path returns deleted memory with matching ID.
#[tokio::test]
async fn test_grpc_delete_memory() {
    let svc = test_grpc_service().await;
    // Store first
    let store_req = tonic::Request::new(proto::StoreMemoryRequest {
        content: "delete test memory".to_string(),
        agent_id: "agent-del".to_string(),
        ..Default::default()
    });
    let stored = svc
        .store_memory(store_req)
        .await
        .unwrap()
        .into_inner()
        .memory
        .unwrap();

    // Delete it
    let del_req = tonic::Request::new(proto::DeleteMemoryRequest {
        id: stored.id.clone(),
    });
    let resp = svc.delete_memory(del_req).await.unwrap();
    let deleted = resp.into_inner().memory.unwrap();
    assert_eq!(deleted.id, stored.id, "deleted memory ID must match");
}

/// GRPC-04/05: DeleteMemory with non-existent ID returns NotFound.
#[tokio::test]
async fn test_grpc_delete_memory_not_found() {
    let svc = test_grpc_service().await;
    let req = tonic::Request::new(proto::DeleteMemoryRequest {
        id: "nonexistent-id-12345".to_string(),
    });
    let err = svc.delete_memory(req).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::NotFound);
}

/// GRPC-05 (validation): DeleteMemory with empty id returns InvalidArgument.
#[tokio::test]
async fn test_grpc_delete_memory_empty_id() {
    let svc = test_grpc_service().await;
    let req = tonic::Request::new(proto::DeleteMemoryRequest {
        id: "".to_string(),
    });
    let err = svc.delete_memory(req).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

// ── Task 2: Per-handler scope enforcement tests ────────────────────────────────

/// Scope enforcement for StoreMemory: scoped key with mismatched agent_id must return PermissionDenied.
/// This test catches missing enforce_scope() calls — not type-enforced in the compiler.
#[tokio::test]
async fn test_grpc_store_memory_scope_enforcement() {
    let svc = test_grpc_service().await;
    let auth = mnemonic::auth::AuthContext {
        key_id: "key-1".to_string(),
        allowed_agent_id: Some("agent-A".to_string()),
    };
    let req = request_with_auth(
        proto::StoreMemoryRequest {
            content: "test content".to_string(),
            agent_id: "agent-B".to_string(), // mismatch!
            ..Default::default()
        },
        auth,
    );
    let err = svc.store_memory(req).await.unwrap_err();
    assert_eq!(
        err.code(),
        tonic::Code::PermissionDenied,
        "scoped key with mismatched agent_id must return PermissionDenied, got: {:?}",
        err.code()
    );
    assert!(
        err.message().contains("agent-A"),
        "error message must mention allowed agent, got: {}",
        err.message()
    );
    assert!(
        err.message().contains("agent-B"),
        "error message must mention requested agent, got: {}",
        err.message()
    );
}

/// Scope enforcement for SearchMemories: scoped key with mismatched agent_id returns PermissionDenied.
#[tokio::test]
async fn test_grpc_search_memories_scope_enforcement() {
    let svc = test_grpc_service().await;
    let auth = mnemonic::auth::AuthContext {
        key_id: "key-1".to_string(),
        allowed_agent_id: Some("agent-A".to_string()),
    };
    let req = request_with_auth(
        proto::SearchMemoriesRequest {
            query: "test query".to_string(),
            agent_id: "agent-B".to_string(), // mismatch!
            ..Default::default()
        },
        auth,
    );
    let err = svc.search_memories(req).await.unwrap_err();
    assert_eq!(
        err.code(),
        tonic::Code::PermissionDenied,
        "search with mismatched agent_id must return PermissionDenied, got: {:?}",
        err.code()
    );
}

/// Scope enforcement for ListMemories: scoped key with mismatched agent_id returns PermissionDenied.
/// This is "Pitfall 4" from RESEARCH.md — easy to miss on list handler.
#[tokio::test]
async fn test_grpc_list_memories_scope_enforcement() {
    let svc = test_grpc_service().await;
    let auth = mnemonic::auth::AuthContext {
        key_id: "key-1".to_string(),
        allowed_agent_id: Some("agent-A".to_string()),
    };
    let req = request_with_auth(
        proto::ListMemoriesRequest {
            agent_id: "agent-B".to_string(), // mismatch!
            ..Default::default()
        },
        auth,
    );
    let err = svc.list_memories(req).await.unwrap_err();
    assert_eq!(
        err.code(),
        tonic::Code::PermissionDenied,
        "list with mismatched agent_id must return PermissionDenied, got: {:?}",
        err.code()
    );
}

/// Scope enforcement for DeleteMemory: scoped key cannot delete a memory owned by a different agent.
/// Uses ownership lookup (D-08 pattern): fetches memory owner before enforcing scope.
#[tokio::test]
async fn test_grpc_delete_memory_scope_enforcement() {
    let svc = test_grpc_service().await;
    // Store a memory as agent-B (open mode, no auth context)
    let store_req = tonic::Request::new(proto::StoreMemoryRequest {
        content: "memory owned by agent-B".to_string(),
        agent_id: "agent-B".to_string(),
        ..Default::default()
    });
    let stored = svc
        .store_memory(store_req)
        .await
        .unwrap()
        .into_inner()
        .memory
        .unwrap();

    // Try to delete as agent-A (scoped key)
    let auth = mnemonic::auth::AuthContext {
        key_id: "key-1".to_string(),
        allowed_agent_id: Some("agent-A".to_string()),
    };
    let del_req = request_with_auth(
        proto::DeleteMemoryRequest {
            id: stored.id.clone(),
        },
        auth,
    );
    let err = svc.delete_memory(del_req).await.unwrap_err();
    assert_eq!(
        err.code(),
        tonic::Code::PermissionDenied,
        "delete with mismatched owner must return PermissionDenied, got: {:?}",
        err.code()
    );
}

// ── Task 2: Health and reflection smoke tests ──────────────────────────────────

/// HEALTH-01: Health reporter can set MnemonicServiceServer as SERVING without panicking.
#[tokio::test]
async fn test_grpc_health_serving() {
    use mnemonic::grpc::proto::mnemonic_service_server::MnemonicServiceServer;
    let (health_reporter, _health_service) = tonic_health::server::health_reporter();
    // set_serving must not panic when given a correctly typed service
    health_reporter
        .set_serving::<MnemonicServiceServer<MnemonicGrpcService>>()
        .await;
    // If we get here without panic, the health reporter is correctly typed.
}

/// HEALTH-02: FILE_DESCRIPTOR_SET is non-empty and can build a tonic-reflection service.
#[tokio::test]
async fn test_grpc_reflection_builds() {
    let descriptor = mnemonic::grpc::FILE_DESCRIPTOR_SET;
    assert!(
        !descriptor.is_empty(),
        "FILE_DESCRIPTOR_SET must be non-empty"
    );
    // Verify reflection service builds successfully from the descriptor
    let _service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(descriptor)
        .build_v1()
        .expect("reflection service must build from FILE_DESCRIPTOR_SET");
}
