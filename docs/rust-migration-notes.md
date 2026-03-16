# Rust API Migration Notes (Core Surface)

This change introduces a **native Rust HTTP service** scaffold (`rust/ollama-api`) that mirrors core Ollama API route contracts and streaming shape for:

- `GET /`
- `GET /api/version`
- `GET /api/status`
- `POST /api/generate`
- `POST /api/chat`
- `POST /api/embed`
- `POST /api/embeddings`

## Compatibility guarantees implemented

- Route paths and methods for the core endpoints above.
- Error envelope shape: `{"error":"..."}`.
- Unknown model behavior for generation route: `404` with model-not-found error text.
- Streaming default for generate/chat (`stream` omitted means streaming enabled).
- NDJSON framing for streaming with `application/x-ndjson` content type and newline-delimited JSON chunks.
- Non-streaming mode for generate/chat returns one JSON object.

## Known differences / remaining migration work

This is a focused migration slice; full parity with the existing Go server still requires additional work:

1. Full endpoint coverage from `server/routes.go` (model lifecycle, create/pull/push/copy/show/delete/tags, cloud passthrough, OpenAI/Anthropic compatibility routes, web experimental routes).
2. Full DTO parity with all optional fields and coercion/default behavior from `api/types.go`.
3. Runtime model loader/scheduler integration equivalent to Go scheduler (`server/sched.go`, `llm/*`, runners).
4. Streamed error behavior mid-stream and exact edge-case semantics for all handlers.
5. Keep-alive/load/unload semantics and model metadata behavior.
6. Performance benchmarking and regression tests against Go behavior with fixture/golden tests.

## Implementation architecture

- `src/lib.rs` is layered around route handlers + typed DTOs + a minimal in-memory backend registry.
- Shared state is concurrency-safe via `Arc<RwLock<...>>`.
- Streaming responses use `Body::from_stream` with preframed newline-delimited JSON bytes.

## Next steps

- Add a backend adapter trait and migrate generation/chat/embed business logic from Go internals.
- Add contract-golden tests that compare current Go server and Rust responses for the same fixtures.
- Expand middleware and compatibility adapters for `/v1/*` and cloud pass-through semantics.
- Promote Rust service to default runtime path after parity gates pass.
