use axum::{
    body::Body,
    extract::State,
    http::{header, Response, StatusCode},
    routing::get,
    Router,
};
use std::path::PathBuf;
use tower_http::services::ServeDir;

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
    let serve_dir = ServeDir::new(&config.dist_dir);

    Router::new()
        .route("/", get(index_handler))
        .route("/index.html", get(index_handler))
        .fallback_service(serve_dir)
        .with_state(config)
}

async fn index_handler(State(config): State<StaticConfig>) -> Response<Body> {
    let index_path = config.dist_dir.join("index.html");
    if let Ok(content) = tokio::fs::read_to_string(&index_path).await {
        let injected = inject_token_into_html(&content, &config.token);
        Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-store")
            .body(Body::from(injected))
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(
                "Frontend build (dist/index.html) not found. Run npm run build first.",
            ))
            .unwrap()
    }
}
