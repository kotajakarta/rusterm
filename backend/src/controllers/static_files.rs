use axum::{
    body::Body,
    extract::State,
    http::{header, Response, StatusCode, Uri},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use std::path::{Component, Path, PathBuf};

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
pub struct EmbeddedAssets;

#[derive(Clone)]
pub struct StaticConfig {
    pub dist_dir: PathBuf,
    pub token: String,
}

pub fn inject_token_into_html(html: &str, token: &str) -> String {
    let meta = format!(
        r#"<meta name="rusterm-token" content="{}"><meta name="termdeck-token" content="{}">"#,
        token, token
    );
    html.replacen("<head>", &format!("<head>\n    {}", meta), 1)
}

pub fn router(config: StaticConfig) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/index.html", get(index_handler))
        .fallback(static_fallback_handler)
        .with_state(config)
}

async fn index_handler(State(config): State<StaticConfig>) -> Response<Body> {
    // 1. Try disk first if dist_dir/index.html exists
    let index_path = config.dist_dir.join("index.html");
    if let Ok(content) = tokio::fs::read_to_string(&index_path).await {
        let injected = inject_token_into_html(&content, &config.token);
        return Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-store")
            .body(Body::from(injected))
            .unwrap();
    }

    // 2. Fallback to embedded index.html
    if let Some(embedded_index) = EmbeddedAssets::get("index.html") {
        if let Ok(content) = std::str::from_utf8(&embedded_index.data) {
            let injected = inject_token_into_html(content, &config.token);
            return Response::builder()
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .header(header::CACHE_CONTROL, "no-store")
                .body(Body::from(injected))
                .unwrap();
        }
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from(
            "Frontend build (dist/index.html) not found. Run npm run build first.",
        ))
        .unwrap()
}

async fn static_fallback_handler(
    State(config): State<StaticConfig>,
    uri: Uri,
) -> Response<Body> {
    let path = uri.path().trim_start_matches('/');

    // Unknown API routes must not fall through to the SPA index.html: the frontend
    // would parse the HTML as JSON ("Unexpected token '<'") instead of seeing a 404.
    if path == "api" || path.starts_with("api/") || path.starts_with("ws/") {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"detail":"Endpoint API tidak ditemukan"}"#))
            .unwrap();
    }

    // Security check: block path traversal attempts
    let rel_path = Path::new(path);
    if rel_path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::from("403 Forbidden"))
            .unwrap();
    }

    // 1. Try disk if file exists in dist_dir (enforce canonical bounds)
    if let Ok(canonical_dist) = config.dist_dir.canonicalize() {
        let candidate = config.dist_dir.join(path);
        if let Ok(canonical_file) = candidate.canonicalize() {
            if canonical_file.starts_with(&canonical_dist) && canonical_file.is_file() {
                if let Ok(bytes) = tokio::fs::read(&canonical_file).await {
                    let mime = mime_guess::from_path(&canonical_file).first_or_octet_stream();
                    let mut builder = Response::builder()
                        .header(header::CONTENT_TYPE, mime.as_ref());
                    if path.starts_with("assets/") {
                        builder = builder.header(header::CACHE_CONTROL, "public, max-age=31536000, immutable");
                    }
                    return builder.body(Body::from(bytes)).unwrap();
                }
            } else if !canonical_file.starts_with(&canonical_dist) {
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Body::from("403 Forbidden"))
                    .unwrap();
            }
        }
    }

    // 2. Try embedded assets (zero-copy static memory)
    if let Some(asset) = EmbeddedAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let mut builder = Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref());
        if path.starts_with("assets/") {
            builder = builder.header(header::CACHE_CONTROL, "public, max-age=31536000, immutable");
        }
        return builder.body(Body::from(asset.data)).unwrap();
    }

    // 3. SPA fallback: if request doesn't look like a file with extension, serve index.html
    if !path.contains('.') {
        return index_handler(State(config)).await;
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("404 Not Found"))
        .unwrap()
}
