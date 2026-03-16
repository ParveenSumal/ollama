use std::{collections::HashMap, sync::Arc};

use axum::{
    body::Body,
    extract::State,
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct AppState {
    models: Arc<RwLock<HashMap<String, ModelBackend>>>,
}

#[derive(Clone, Default)]
struct ModelBackend;

#[derive(Debug, Deserialize)]
pub struct GenerateRequest {
    pub model: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct EmbedRequest {
    pub model: String,
    pub input: Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: String,
}

#[derive(Debug, Serialize)]
struct StatusResponse {
    cloud: CloudStatus,
}

#[derive(Debug, Serialize)]
struct CloudStatus {
    disabled: bool,
    source: String,
}

#[derive(Debug, Error)]
enum ApiError {
    #[error("model '{0}' not found")]
    ModelNotFound(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            ApiError::ModelNotFound(_) => StatusCode::NOT_FOUND,
        };
        (status, Json(ErrorEnvelope { error: self.to_string() })).into_response()
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/api/version", get(version))
        .route("/api/status", get(status))
        .route("/api/generate", post(generate))
        .route("/api/chat", post(chat))
        .route("/api/embed", post(embed))
        .route("/api/embeddings", post(embed))
        .with_state(state)
}

async fn root() -> impl IntoResponse {
    (StatusCode::OK, "Ollama is running")
}

async fn version() -> impl IntoResponse {
    Json(json!({"version": env!("CARGO_PKG_VERSION")}))
}

async fn status() -> impl IntoResponse {
    Json(StatusResponse {
        cloud: CloudStatus {
            disabled: false,
            source: "default".to_string(),
        },
    })
}

async fn generate(
    State(state): State<AppState>,
    Json(req): Json<GenerateRequest>,
) -> Result<Response, ApiError> {
    ensure_model(&state, &req.model).await?;
    let should_stream = req.stream.unwrap_or(true);
    if !should_stream {
        return Ok(Json(json!({
            "model": req.model,
            "response": req.prompt,
            "done": true,
        }))
        .into_response());
    }

    let chunks = vec![
        json!({"model": req.model, "response": req.prompt, "done": false}),
        json!({"model": req.model, "response": "", "done": true}),
    ];

    Ok(ndjson_stream(chunks))
}

async fn chat(State(state): State<AppState>, Json(req): Json<ChatRequest>) -> Result<Response, ApiError> {
    ensure_model(&state, &req.model).await?;
    let should_stream = req.stream.unwrap_or(true);

    let combined = req.messages.into_iter().map(|m| m.content).collect::<Vec<_>>().join(" ");

    if !should_stream {
        return Ok(Json(json!({
            "model": req.model,
            "message": {"role": "assistant", "content": combined},
            "done": true,
        }))
        .into_response());
    }

    let chunks = vec![
        json!({"model": req.model, "message": {"role": "assistant", "content": combined}, "done": false}),
        json!({"model": req.model, "message": {"role": "assistant", "content": ""}, "done": true}),
    ];

    Ok(ndjson_stream(chunks))
}

async fn embed(State(state): State<AppState>, Json(req): Json<EmbedRequest>) -> Result<Response, ApiError> {
    ensure_model(&state, &req.model).await?;
    let count = match req.input {
        Value::Array(v) => v.len(),
        _ => 1,
    };
    let embeddings = (0..count).map(|_| vec![0.0f32, 0.0, 0.0]).collect::<Vec<_>>();
    Ok(Json(json!({"model": req.model, "embeddings": embeddings})).into_response())
}

fn ndjson_stream(chunks: Vec<Value>) -> Response {
    let mapped = stream::iter(chunks.into_iter().map(|v| {
        let mut bytes = serde_json::to_vec(&v).expect("json serialize");
        bytes.push(b'\n');
        Ok::<Bytes, std::convert::Infallible>(Bytes::from(bytes))
    }));

    let mut resp = Response::new(Body::from_stream(mapped));
    resp.headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/x-ndjson"));
    resp
}

async fn ensure_model(state: &AppState, model: &str) -> Result<(), ApiError> {
    let models = state.models.read().await;
    if !models.contains_key(model) {
        return Err(ApiError::ModelNotFound(model.to_string()));
    }
    Ok(())
}

pub async fn with_default_model() -> AppState {
    let state = AppState::default();
    let mut models = state.models.write().await;
    models.insert("llama3".to_string(), ModelBackend);
    drop(models);
    state
}
