use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ollama_api::{app, with_default_model};
use tower::ServiceExt;

#[tokio::test]
async fn version_route_contract() {
    let state = with_default_model().await;
    let response = app(state)
        .oneshot(Request::builder().uri("/api/version").body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("version"));
}

#[tokio::test]
async fn generate_streams_ndjson_by_default() {
    let state = with_default_model().await;
    let req = Request::builder()
        .method("POST")
        .uri("/api/generate")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(r#"{"model":"llama3","prompt":"hello"}"#))
        .unwrap();

    let response = app(state).oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/x-ndjson"
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.lines().count() >= 2);
}

#[tokio::test]
async fn generate_unknown_model_404_error_envelope() {
    let state = with_default_model().await;
    let req = Request::builder()
        .method("POST")
        .uri("/api/generate")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(r#"{"model":"missing","prompt":"hello","stream":false}"#))
        .unwrap();

    let response = app(state).oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(text, r#"{"error":"model 'missing' not found"}"#);
}

#[tokio::test]
async fn malformed_json_400() {
    let state = with_default_model().await;
    let req = Request::builder()
        .method("POST")
        .uri("/api/chat")
        .header("content-type", "application/json")
        .body(axum::body::Body::from("{"))
        .unwrap();

    let response = app(state).oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
