//! 经认证的有界 HTTP 入口 / Authenticated bounded HTTP entry point.

use std::sync::Arc;
use axum::{Json, Router};
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use ospf_rust_bridge::{BridgeError, HostCapabilities, JobStatus, JobView, SolveRequest};
use ospf_rust_native::NativeEngine;
use tokio::sync::Semaphore;
use tower_http::cors::CorsLayer;

/// 宿主状态 / Host state.
#[derive(Clone)]
pub struct HostState {
    /// 共享原生执行器 / Shared native executor.
    pub engine: NativeEngine,
    /// 不回显的认证凭据 / Authentication credential, never echoed.
    pub token: String,
}

struct HttpError(BridgeError);

impl From<BridgeError> for HttpError {
    fn from(error: BridgeError) -> Self { Self(error) }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let status = match self.0.code.as_str() {
            "NotFound" => StatusCode::NOT_FOUND,
            "Busy" | "CapacityExceeded" => StatusCode::CONFLICT,
            "Unauthorized" => StatusCode::UNAUTHORIZED,
            "PayloadTooLarge" => StatusCode::PAYLOAD_TOO_LARGE,
            "ExecutionFailure" => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_REQUEST,
        };
        (status, Json(self.0)).into_response()
    }
}

async fn authenticate(
    State(state): State<HostState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let expected = format!("Bearer {}", state.token);
    let supplied = request.headers().get(header::AUTHORIZATION).map(|v| v.as_bytes()).unwrap_or_default();
    let mismatch = expected.as_bytes().iter().enumerate().fold(supplied.len() ^ expected.len(), |diff, (i, byte)| {
        diff | usize::from(*byte ^ supplied.get(i).copied().unwrap_or(0))
    });
    if mismatch != 0 { return HttpError(BridgeError::new("Unauthorized", "invalid bearer token")).into_response(); }
    next.run(request).await
}

async fn capabilities(State(state): State<HostState>) -> Json<HostCapabilities> {
    Json(state.engine.capabilities())
}

async fn submit(State(state): State<HostState>, body: std::result::Result<Json<SolveRequest>, axum::extract::rejection::JsonRejection>) -> std::result::Result<(StatusCode, Json<JobView>), HttpError> {
    let Json(request) = body.map_err(|err| {
        let code = if err.status() == StatusCode::PAYLOAD_TOO_LARGE { "PayloadTooLarge" } else { "InvalidInput" };
        HttpError(BridgeError::new(code, err.body_text()))
    })?;
    let view = tokio::task::spawn_blocking(move || state.engine.submit(request)).await
        .map_err(|_| HttpError(BridgeError::new("ExecutionFailure", "submission task failed")))??;
    Ok((StatusCode::ACCEPTED, Json(view)))
}

async fn status(State(state): State<HostState>, Path(id): Path<String>) -> std::result::Result<Json<JobView>, HttpError> {
    Ok(Json(state.engine.get(&id)?))
}

async fn cancel(State(state): State<HostState>, Path(id): Path<String>) -> std::result::Result<Json<JobView>, HttpError> {
    Ok(Json(state.engine.cancel(&id)?))
}

async fn result(State(state): State<HostState>, Path(id): Path<String>) -> std::result::Result<Json<JobView>, HttpError> {
    let view = state.engine.get(&id)?;
    if view.status == JobStatus::Running { return Err(BridgeError::new("Busy", "result is not ready").into()); }
    Ok(Json(view))
}

async fn release(State(state): State<HostState>, Path(id): Path<String>) -> std::result::Result<StatusCode, HttpError> {
    state.engine.release(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 创建路由；只允许显式来源 / Build routes with explicitly allowed origins only.
pub fn router(state: HostState, origins: Vec<HeaderValue>) -> ospf_rust_bridge::Result<Router> {
    if state.token.len() < 32 || !state.token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(BridgeError::new("InvalidConfiguration", "token must contain at least 32 printable ASCII characters"));
    }
    if origins.iter().any(|origin| !origin.to_str().is_ok_and(|v| (v.starts_with("http://") || v.starts_with("https://")) && !v.contains('*'))) {
        return Err(BridgeError::new("InvalidConfiguration", "origins must be explicit HTTP(S) origins"));
    }
    let cors = CorsLayer::new().allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
    let submissions = Arc::new(Semaphore::new(1));
    let submit_route = post(submit).layer(middleware::from_fn(move |request: axum::extract::Request, next: Next| {
        let submissions = submissions.clone();
        async move {
            // 在 JSON 提取前拒绝并发提交，避免大请求在内存及阻塞池中排队。
            // Reject concurrent submissions before JSON extraction, without body or thread-pool queues.
            let Ok(_permit) = submissions.try_acquire_owned() else {
                return HttpError(BridgeError::new("Busy", "another submission is being prepared")).into_response();
            };
            next.run(request).await
        }
    }));
    Ok(Router::new()
        .route("/ospf/v1/capabilities", get(capabilities))
        .route("/ospf/v1/jobs", submit_route)
        .route("/ospf/v1/jobs/{id}", get(status).delete(release))
        .route("/ospf/v1/jobs/{id}/cancel", post(cancel))
        .route("/ospf/v1/jobs/{id}/result", get(result))
        .layer(DefaultBodyLimit::max(9 * 1024 * 1024))
        .route_layer(middleware::from_fn_with_state(state.clone(), authenticate))
        .layer(cors)
        .with_state(state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn app() -> Router {
        router(HostState { engine: NativeEngine::default(), token: TOKEN.into() }, vec![HeaderValue::from_static("http://localhost:3000")]).unwrap()
    }

    #[tokio::test]
    async fn auth_and_capability_contract() {
        let response = app().oneshot(Request::builder().uri("/ospf/v1/capabilities").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let response = app().oneshot(Request::builder().uri("/ospf/v1/capabilities")
            .header("Authorization", format!("Bearer {TOKEN}")).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let value: HostCapabilities = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value.protocol_version, 1);
    }

    #[tokio::test]
    async fn cors_preflight_and_malformed_request() {
        let response = app().oneshot(Request::builder().method("OPTIONS").uri("/ospf/v1/jobs")
            .header("Origin", "http://localhost:3000").header("Access-Control-Request-Method", "POST")
            .header("Access-Control-Request-Headers", "authorization,content-type").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.headers().get("access-control-allow-origin").unwrap(), "http://localhost:3000");
        let response = app().oneshot(Request::builder().method("POST").uri("/ospf/v1/jobs")
            .header("Authorization", format!("Bearer {TOKEN}")).header("Content-Type", "application/json")
            .body(Body::from("{}" )).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn body_limits_missing_jobs_and_disallowed_origins() {
        let response = app().oneshot(Request::builder().method("POST").uri("/ospf/v1/jobs")
            .header("Authorization", format!("Bearer {TOKEN}")).header("Content-Type", "application/json")
            .body(Body::from(vec![b' '; 9 * 1024 * 1024 + 1])).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let response = app().oneshot(Request::builder().uri("/ospf/v1/jobs/missing")
            .header("Authorization", format!("Bearer {TOKEN}")).header("Origin", "http://untrusted.example")
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(response.headers().get("access-control-allow-origin").is_none());
    }

    #[test]
    fn rejects_weak_tokens_and_wildcard_origins() {
        assert!(router(HostState { engine: NativeEngine::default(), token: "weak".into() }, vec![]).is_err());
        assert!(router(HostState { engine: NativeEngine::default(), token: TOKEN.into() }, vec![HeaderValue::from_static("*")]).is_err());
    }
}
