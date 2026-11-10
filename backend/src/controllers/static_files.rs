use axum::{
    body::Body,
    extract::State,
    http::{header, Response, StatusCode, Uri},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use std::path::PathBuf;

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
pub struct EmbeddedAssets;

#[derive(Clone)]
pub struct StaticConfig {
    pub dist_dir: PathBuf,
    pub token: String,
}

pub fn inject_token_into_html(html: &str, token: &str) -> String {
    let meta = format!(r#"<meta name="termdeck-token" content="{}">"#, token);
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

    // 1. Try disk if file exists in dist_dir
    let disk_file = config.dist_dir.join(path);
    if disk_file.is_file() {
        if let Ok(bytes) = tokio::fs::read(&disk_file).await {
            let mime = mime_guess::from_path(&disk_file).first_or_octet_stream();
            return Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(bytes))
                .unwrap();
        }
    }

    // 2. Try embedded assets
    if let Some(asset) = EmbeddedAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(asset.data.into_owned()))
            .unwrap();
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
