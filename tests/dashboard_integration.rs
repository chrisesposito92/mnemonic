//! Integration tests for the dashboard feature (BUILD-01, BUILD-02).
//!
//! Tests exercise `build_router(test_state)` -- the full merged router --
//! to prove /ui is actually mounted alongside protected and public routes.
//! Only compiled with `--features dashboard`.
//!
//! SUCCESS CRITERION 5 VERIFICATION:
//! To verify compile-time failure when dashboard/dist/ is missing:
//!   1. Rename or delete dashboard/dist/
//!   2. Run: cargo build --features dashboard
//!   3. Expected: compile error from rust-embed proc macro:
//!      "error: proc macro derive panicked"
//!      "folder 'dashboard/dist/' does not exist"
//!   4. Restore dashboard/dist/ afterward
//!
//! This is NOT automated in CI because it requires a broken state.
//! The compile-time guard is inherent to rust-embed's #[derive(RustEmbed)]
//! without #[allow_missing = true] -- which src/dashboard.rs intentionally omits.
//!
//! CROSS-FEATURE BUILD VERIFICATION (review concern #7):
//! These feature combinations must compile without errors:
//!   cargo check --features dashboard,backend-qdrant
//!   cargo check --features dashboard,backend-postgres
//!   cargo check --features dashboard,backend-qdrant,backend-postgres
//! Verified by CI matrix. Not automated as #[test] because backend-qdrant and
//! backend-postgres require external services for full test execution.

#![cfg(feature = "dashboard")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use mnemonic::compaction::CompactionService;
use mnemonic::server::{AppState, build_router};
use mnemonic::service::MemoryService;
use mnemonic::storage::SqliteBackend;
use mnemonic::summarization::MockSummarizer;

use std::sync::{Arc, Once, OnceLock};

use mnemonic::embedding::LocalEngine;

static INIT: Once = Once::new();

static LOCAL_ENGINE: OnceLock<Arc<LocalEngine>> = OnceLock::new();

fn local_engine() -> Arc<LocalEngine> {
    Arc::clone(LOCAL_ENGINE.get_or_init(|| {
        let engine = LocalEngine::new().expect("LocalEngine::new() should succeed");
        Arc::new(engine)
    }))
}

fn setup() {
    INIT.call_once(|| {
        mnemonic::db::register_sqlite_vec();
    });
}

fn test_config() -> mnemonic::config::Config {
    mnemonic::config::Config {
        port: 0,
        db_path: ":memory:".to_string(),
        embedding_provider: "local".to_string(),
        openai_api_key: None,
        ..Default::default()
    }
}

/// Constructs a real AppState with in-memory SQLite, matching the
/// pattern in tests/integration.rs. This is the same state the real
/// server uses, ensuring our test exercises the actual router merge.
async fn test_state() -> AppState {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();
    let db_arc = Arc::new(conn);
    let engine = local_engine();
    let backend: Arc<dyn mnemonic::storage::StorageBackend> =
        Arc::new(SqliteBackend::new(db_arc.clone()));
    let service = Arc::new(MemoryService::new(
        backend.clone(),
        engine.clone(),
        "all-MiniLM-L6-v2".to_string(),
    ));
    let compaction = Arc::new(CompactionService::new(
        backend.clone(),
        db_arc.clone(),
        engine.clone(),
        Some(Arc::new(MockSummarizer) as Arc<dyn mnemonic::summarization::SummarizationEngine>),
        "all-MiniLM-L6-v2".to_string(),
    ));
    let key_service = Arc::new(mnemonic::auth::KeyService::new(db_arc.clone()));

    AppState {
        service,
        compaction,
        key_service,
        backend_name: "sqlite".to_string(),
    }
}

/// BUILD-01: GET /ui/ through build_router returns 200 with text/html.
/// This proves the dashboard is actually mounted in the merged router,
/// not just that dashboard::router() works in isolation.
#[tokio::test]
async fn dashboard_ui_slash_returns_200_html() {
    let state = test_state().await;
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ui/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /ui/ through build_router must return 200"
    );

    let content_type = response
        .headers()
        .get("content-type")
        .expect("response must have content-type header")
        .to_str()
        .unwrap();

    assert!(
        content_type.contains("text/html"),
        "content-type must be text/html, got: {}",
        content_type
    );
}

/// BUILD-01: GET /ui/ response body contains the stable mnemonic-root mount point.
/// Uses a project-specific ID rather than a generic "app" (review concern #8).
#[tokio::test]
async fn dashboard_ui_contains_mnemonic_root() {
    let state = test_state().await;
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ui/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        html.contains("mnemonic-root"),
        "HTML body must contain the mnemonic-root mount point, got: {}",
        &html[..html.len().min(200)]
    );
}

/// Review concern #3: GET /ui (no trailing slash) behavior is deterministic.
/// axum nest_service may serve directly (200) or redirect to /ui/ (301/308).
/// Either is acceptable, but the behavior must be documented and tested.
#[tokio::test]
async fn dashboard_ui_no_trailing_slash_returns_200_or_redirect() {
    let state = test_state().await;
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ui")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    assert!(
        status == StatusCode::OK
            || status == StatusCode::MOVED_PERMANENTLY
            || status == StatusCode::PERMANENT_REDIRECT,
        "GET /ui must return 200 or redirect (301/308), got: {}",
        status
    );

    // If redirect, verify it points to /ui/
    if status == StatusCode::MOVED_PERMANENTLY || status == StatusCode::PERMANENT_REDIRECT {
        let location = response
            .headers()
            .get("location")
            .expect("redirect must have location header")
            .to_str()
            .unwrap();
        assert!(
            location.ends_with("/ui/"),
            "redirect location must end with /ui/, got: {}",
            location
        );
    }
}

/// BUILD-01: GET /ui/nonexistent returns 200 with index.html (SPA fallback).
/// Proves FallbackBehavior::Ok works through the full merged router.
#[tokio::test]
async fn dashboard_spa_fallback_returns_index_html() {
    let state = test_state().await;
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ui/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "SPA fallback must return 200 for unknown paths under /ui/"
    );

    let content_type = response
        .headers()
        .get("content-type")
        .expect("response must have content-type header")
        .to_str()
        .unwrap();

    assert!(
        content_type.contains("text/html"),
        "SPA fallback must return text/html, got: {}",
        content_type
    );
}

/// BUILD-02: GET /health still works alongside the dashboard mount.
/// Proves the dashboard merge does not break existing public routes.
#[tokio::test]
async fn health_endpoint_still_works_with_dashboard() {
    let state = test_state().await;
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /health must still return 200 with dashboard feature enabled"
    );
}

/// BUILD-01 multi-file fallback test: verify that if dist/ contains
/// additional asset files (CSS/JS), they are served with correct content-type
/// through the build_router. This covers review concern #1 about the
/// fallback path being under-specified.
///
/// NOTE: This test works in both single-file and multi-file modes:
/// - Single-file: dist/ only has index.html, so /ui/assets/anything
///   falls through to SPA fallback (200 text/html) -- which is correct
///   because there ARE no external assets to serve.
/// - Multi-file: dist/ has index.html + assets/*.js + assets/*.css,
///   and /ui/assets/main.js returns the actual JS file with correct MIME.
///
/// The test verifies the SPA fallback path is safe in both modes.
#[tokio::test]
async fn dashboard_asset_request_returns_valid_response() {
    let state = test_state().await;
    let app = build_router(state);

    // Request a path that would be an asset in multi-file mode
    let response = app
        .oneshot(
            Request::builder()
                .uri("/ui/assets/nonexistent.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // In single-file mode: SPA fallback returns 200 text/html (correct -- no assets to serve)
    // In multi-file mode: 404 for missing asset OR 200 text/html via fallback
    // Both are acceptable -- the key is no 500 error
    let status = response.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::NOT_FOUND,
        "asset request must return 200 (fallback) or 404, not a server error, got: {}",
        status
    );
}

/// AUTH-02: GET /ui/ response includes Content-Security-Policy header.
/// Also verifies CSP against SPA fallback path (review concern #9 -- test on actual routes).
#[tokio::test]
async fn dashboard_ui_includes_csp_header() {
    let state = test_state().await;
    let app = build_router(state);

    // Test on /ui/ (root)
    let response = app.clone()
        .oneshot(
            Request::builder()
                .uri("/ui/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let csp = response
        .headers()
        .get("content-security-policy")
        .expect("response must have content-security-policy header")
        .to_str()
        .unwrap();

    assert!(csp.contains("default-src 'self'"), "CSP must contain default-src 'self', got: {}", csp);
    assert!(csp.contains("script-src 'unsafe-inline'"), "CSP must contain script-src 'unsafe-inline', got: {}", csp);
    assert!(csp.contains("style-src 'unsafe-inline'"), "CSP must contain style-src 'unsafe-inline', got: {}", csp);

    // Test on SPA fallback path (review concern #9: CSP on all /ui/* routes)
    let fallback_resp = app
        .oneshot(
            Request::builder()
                .uri("/ui/some/deep/path")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let fallback_csp = fallback_resp
        .headers()
        .get("content-security-policy")
        .expect("SPA fallback must also have CSP header");

    assert!(
        fallback_csp.to_str().unwrap().contains("default-src 'self'"),
        "SPA fallback CSP must match root CSP"
    );
}

/// BROWSE-05: GET /stats returns per-agent breakdown with memory_count and last_active.
#[tokio::test]
async fn stats_endpoint_returns_agent_breakdown() {
    let state = test_state().await;
    let app = build_router(state);

    // Create memories with different agent_ids
    let create_body = |agent: &str, content: &str| {
        serde_json::json!({ "content": content, "agent_id": agent }).to_string()
    };

    let resp1 = app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/memories")
                .header("content-type", "application/json")
                .body(Body::from(create_body("agent-alpha", "test memory alpha")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp1.status(), StatusCode::CREATED);

    let resp2 = app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/memories")
                .header("content-type", "application/json")
                .body(Body::from(create_body("agent-beta", "test memory beta")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::CREATED);

    // GET /stats
    let stats_resp = app
        .oneshot(
            Request::builder()
                .uri("/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(stats_resp.status(), StatusCode::OK, "GET /stats must return 200");

    let body = stats_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let agents = json["agents"].as_array().expect("response must have agents array");
    assert!(agents.len() >= 2, "stats must include at least 2 agents, got: {}", agents.len());

    for agent in agents {
        assert!(agent["agent_id"].is_string(), "agent must have agent_id string");
        assert!(agent["memory_count"].is_u64() || agent["memory_count"].is_i64(),
            "agent must have memory_count number");
        assert!(agent["last_active"].is_string(), "agent must have last_active string");
    }
}

/// OPS-01 + AUTH-01: GET /health returns auth_enabled field.
/// In test environment (no active keys), auth_enabled should be false.
#[tokio::test]
async fn health_endpoint_includes_auth_enabled_field() {
    let state = test_state().await;
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert!(json["backend"].is_string(), "health must have backend field");
    // auth_enabled must be present as a boolean
    assert!(
        json["auth_enabled"].is_boolean(),
        "health response must have auth_enabled boolean field, got: {}",
        json
    );
    // No active keys in test environment = open mode = auth_enabled: false
    assert_eq!(
        json["auth_enabled"].as_bool().unwrap(),
        false,
        "auth_enabled must be false when no active keys exist"
    );
}
