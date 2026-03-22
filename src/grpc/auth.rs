//! gRPC auth Tower Layer — async authentication middleware for the tonic gRPC server.
//!
//! Implements GrpcAuthLayer (Tower Layer) and GrpcAuthService (Tower Service) that wrap
//! the gRPC service with async authentication. Reuses KeyService.validate() and
//! KeyService.count_active_keys() from src/auth.rs — identical logic to the REST
//! auth_middleware, adapted for HTTP/2 Tower semantics.
//!
//! Key design points (per RESEARCH.md Pattern 2):
//! - Tower Layer receives http::Request<tonic::body::Body>, NOT tonic::Request<T>
//! - Use req.headers().get("authorization"), NOT req.metadata()
//! - Must use clone+swap pattern (mem::replace) in Service::call
//! - Status::into_http() for error responses

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// Tower Layer that wraps a gRPC service with async authentication.
///
/// Apply with: `Server::builder().layer(GrpcAuthLayer { key_service: ... })`
#[derive(Clone)]
pub struct GrpcAuthLayer {
    pub key_service: Arc<crate::auth::KeyService>,
}

impl<S> tower::Layer<S> for GrpcAuthLayer {
    type Service = GrpcAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcAuthService {
            inner,
            key_service: Arc::clone(&self.key_service),
        }
    }
}

/// Tower Service that performs async authentication before delegating to the inner service.
///
/// Auth flow (mirrors src/auth.rs auth_middleware):
/// 1. Open mode check: count_active_keys() == 0 -> pass through (AUTH-03)
/// 2. Health check bypass: /grpc.health.v1.Health/* -> pass through always
/// 3. Extract Authorization header, strip "Bearer " prefix
/// 4. Validate token via key_service.validate()
/// 5. On success: inject AuthContext into request extensions
/// 6. Error mapping: missing header -> Unauthenticated, malformed -> InvalidArgument, invalid -> Unauthenticated
#[derive(Clone)]
pub struct GrpcAuthService<S> {
    inner: S,
    key_service: Arc<crate::auth::KeyService>,
}

impl<S, ResBody> tower::Service<http::Request<tonic::body::Body>> for GrpcAuthService<S>
where
    S: tower::Service<http::Request<tonic::body::Body>, Response = http::Response<ResBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ResBody: Default + Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<tonic::body::Body>) -> Self::Future {
        // CRITICAL: clone+swap to avoid moving self.inner into async block (Pitfall 2)
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let key_service = Arc::clone(&self.key_service);

        Box::pin(async move {
            // Health check bypass: skip auth for health service paths (Pitfall 7)
            if req.uri().path().starts_with("/grpc.health.v1.Health/") {
                return inner.call(req).await;
            }

            // Reflection bypass: skip auth for server reflection paths (Pitfall 3)
            // Allows `grpcurl list` to work in both open mode and when auth is active.
            if req.uri().path().starts_with("/grpc.reflection.v1.ServerReflection/") {
                return inner.call(req).await;
            }

            // Open mode check (per D-19, AUTH-03)
            match key_service.count_active_keys().await {
                Ok(0) => return inner.call(req).await,
                Err(_) => {
                    return Ok(
                        tonic::Status::unauthenticated("auth service unavailable").into_http(),
                    );
                }
                Ok(_) => {} // auth active, continue
            }

            // Extract authorization header from HTTP headers (NOT tonic metadata -- Pitfall 1)
            let auth_header = req.headers().get("authorization");
            let bearer = match auth_header.and_then(|v| v.to_str().ok()) {
                None => {
                    // Missing header when auth active -> Unauthenticated (per D-18)
                    return Ok(
                        tonic::Status::unauthenticated("missing authorization header").into_http(),
                    );
                }
                Some(raw) => match raw.strip_prefix("Bearer ") {
                    Some(token) if !token.is_empty() => token.to_string(),
                    _ => {
                        // Malformed header -> InvalidArgument (per D-18)
                        return Ok(tonic::Status::invalid_argument(
                            "authorization header must use format: Bearer <token>",
                        )
                        .into_http());
                    }
                },
            };

            // Validate token via KeyService (per D-16)
            match key_service.validate(&bearer).await {
                Ok(auth_ctx) => {
                    // Inject AuthContext into extensions (per D-17)
                    let mut req = req;
                    req.extensions_mut().insert(auth_ctx);
                    inner.call(req).await
                }
                Err(_) => {
                    // Invalid or revoked token -> Unauthenticated (per D-18)
                    Ok(tonic::Status::unauthenticated("invalid or revoked API key").into_http())
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;
    use std::sync::Arc;
    use tower::{Layer, ServiceExt};

    /// Type alias for the boxed auth service used in tests.
    type BoxAuthSvc = GrpcAuthService<
        tower::util::BoxCloneService<
            http::Request<tonic::body::Body>,
            http::Response<tonic::body::Body>,
            std::convert::Infallible,
        >,
    >;

    /// Creates a real KeyService backed by an in-memory SQLite DB.
    /// Mirrors the test_key_service() helper in src/auth.rs.
    async fn test_key_service() -> crate::auth::KeyService {
        crate::db::register_sqlite_vec();
        let config = crate::config::Config {
            port: 0,
            db_path: ":memory:".to_string(),
            embedding_provider: "local".to_string(),
            openai_api_key: None,
            ..Default::default()
        };
        let conn = crate::db::open(&config).await.unwrap();
        crate::auth::KeyService::new(Arc::new(conn))
    }

    /// Creates an auth-wrapped service using tower::service_fn as the inner service.
    /// The inner service always responds with HTTP 200 (simulating successful gRPC service).
    fn make_auth_service(key_service: crate::auth::KeyService) -> BoxAuthSvc {
        let layer = GrpcAuthLayer {
            key_service: Arc::new(key_service),
        };
        let inner = tower::service_fn(|_req: http::Request<tonic::body::Body>| async {
            let resp = http::Response::builder()
                .status(StatusCode::OK)
                .body(tonic::body::Body::default())
                .unwrap();
            Ok::<_, std::convert::Infallible>(resp)
        });
        // Box the inner service so the type is concrete and inferrable
        let boxed = tower::util::BoxCloneService::new(inner);
        layer.layer(boxed)
    }

    /// Build a plain HTTP request to a given path with no authorization header.
    fn make_request(path: &str) -> http::Request<tonic::body::Body> {
        http::Request::builder()
            .uri(path)
            .body(tonic::body::Body::default())
            .unwrap()
    }

    /// Build an HTTP request with an Authorization header.
    fn make_request_with_auth(path: &str, auth_value: &str) -> http::Request<tonic::body::Body> {
        http::Request::builder()
            .uri(path)
            .header("authorization", auth_value)
            .body(tonic::body::Body::default())
            .unwrap()
    }

    /// Extracts the gRPC status code from response headers set by Status::into_http().
    /// Returns None if no grpc-status is present (i.e., inner service responded normally).
    fn grpc_status_from_response(
        resp: &http::Response<tonic::body::Body>,
    ) -> Option<tonic::Code> {
        resp.headers()
            .get("grpc-status")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<i32>().ok())
            .map(tonic::Code::from)
    }

    // --- Test 1 (AUTH-03): Open mode bypass ---

    #[tokio::test]
    async fn test_grpc_auth_open_mode_bypasses() {
        // No keys in DB -- open mode. Request without auth header must pass through.
        let ks = test_key_service().await;
        let svc = make_auth_service(ks);

        let req = make_request("/mnemonic.v1.MnemonicService/StoreMemory");
        let resp = svc.oneshot(req).await.unwrap();

        // Inner service responded with 200 -- no auth rejection
        assert_eq!(resp.status(), StatusCode::OK);
        // No grpc-status header means the inner service handled it (no auth error)
        assert!(
            grpc_status_from_response(&resp).is_none()
                || grpc_status_from_response(&resp) == Some(tonic::Code::Ok),
            "open mode must pass through without auth error"
        );
    }

    // --- Test 2 (AUTH-01): Valid token injects AuthContext into extensions ---

    #[tokio::test]
    async fn test_grpc_auth_valid_token_injects_context() {
        let ks = test_key_service().await;
        let (_api_key, raw_token) = ks
            .create("test-key".to_string(), Some("agent-x".to_string()))
            .await
            .unwrap();

        // Build a layer using the same key_service Arc that was used to create the key
        let ks_arc = Arc::new(ks);
        let layer = GrpcAuthLayer {
            key_service: Arc::clone(&ks_arc),
        };

        // Inner service that checks for AuthContext in extensions
        let inner = tower::service_fn(|req: http::Request<tonic::body::Body>| async move {
            let auth_ctx = req.extensions().get::<crate::auth::AuthContext>().cloned();
            let status = if auth_ctx.is_some() {
                StatusCode::OK
            } else {
                StatusCode::UNAUTHORIZED
            };
            let resp = http::Response::builder()
                .status(status)
                .body(tonic::body::Body::default())
                .unwrap();
            Ok::<_, std::convert::Infallible>(resp)
        });
        let boxed = tower::util::BoxCloneService::new(inner);
        let svc = layer.layer(boxed);

        let req = make_request_with_auth(
            "/mnemonic.v1.MnemonicService/StoreMemory",
            &format!("Bearer {}", raw_token),
        );
        let resp = svc.oneshot(req).await.unwrap();

        // Inner service must have seen AuthContext (returned 200)
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "valid token must inject AuthContext into extensions"
        );
    }

    // --- Test 3 (AUTH-01): Invalid token returns Unauthenticated ---

    #[tokio::test]
    async fn test_grpc_auth_invalid_token_returns_unauthenticated() {
        let ks = test_key_service().await;
        // Create a key to activate auth mode, but don't use its token
        let _ = ks.create("activate-auth".to_string(), None).await.unwrap();

        let svc = make_auth_service(ks);

        let wrong_token = "mnk_0000000000000000000000000000000000000000000000000000000000000000";
        let req = make_request_with_auth(
            "/mnemonic.v1.MnemonicService/StoreMemory",
            &format!("Bearer {}", wrong_token),
        );
        let resp = svc.oneshot(req).await.unwrap();

        let code = grpc_status_from_response(&resp);
        assert_eq!(
            code,
            Some(tonic::Code::Unauthenticated),
            "invalid token must return Status::Unauthenticated, got: {:?}",
            code
        );
    }

    // --- Test 4 (AUTH-01): Missing authorization header returns Unauthenticated ---

    #[tokio::test]
    async fn test_grpc_auth_missing_header_returns_unauthenticated() {
        let ks = test_key_service().await;
        // Create a key to activate auth mode
        let _ = ks.create("activate-auth".to_string(), None).await.unwrap();

        let svc = make_auth_service(ks);

        let req = make_request("/mnemonic.v1.MnemonicService/StoreMemory");
        let resp = svc.oneshot(req).await.unwrap();

        let code = grpc_status_from_response(&resp);
        assert_eq!(
            code,
            Some(tonic::Code::Unauthenticated),
            "missing header must return Status::Unauthenticated, got: {:?}",
            code
        );
    }

    // --- Test 5 (AUTH-01): Malformed authorization header returns InvalidArgument ---

    #[tokio::test]
    async fn test_grpc_auth_malformed_header_returns_invalid_argument() {
        let ks = test_key_service().await;
        // Create a key to activate auth mode
        let _ = ks.create("activate-auth".to_string(), None).await.unwrap();

        let svc = make_auth_service(ks);

        // "Basic xxx" is malformed (not "Bearer <token>")
        let req = make_request_with_auth(
            "/mnemonic.v1.MnemonicService/StoreMemory",
            "Basic c29tZXVzZXI6c29tZXBhc3N3b3Jk",
        );
        let resp = svc.oneshot(req).await.unwrap();

        let code = grpc_status_from_response(&resp);
        assert_eq!(
            code,
            Some(tonic::Code::InvalidArgument),
            "malformed header must return Status::InvalidArgument, got: {:?}",
            code
        );
    }

    // --- Test 6: Health check path bypass ---

    #[tokio::test]
    async fn test_grpc_auth_health_bypass() {
        let ks = test_key_service().await;
        // Create a key to activate auth mode
        let _ = ks.create("activate-auth".to_string(), None).await.unwrap();

        let svc = make_auth_service(ks);

        // No auth header, but it's a health check -- must bypass
        let req = make_request("/grpc.health.v1.Health/Check");
        let resp = svc.oneshot(req).await.unwrap();

        // Inner service responded with 200 -- health check bypassed auth
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "health check path must bypass auth even when API keys are configured"
        );
    }
}
