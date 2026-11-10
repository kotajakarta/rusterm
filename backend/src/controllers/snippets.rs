use crate::controllers::servers::{check_auth, AppState};
use crate::models::snippet::{Snippet, SnippetInput};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, put},
    Json, Router,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/snippets", get(list_snippets).post(create_snippet))
        .route(
            "/snippets/:id",
            put(update_snippet).delete(delete_snippet),
        )
        .with_state(state)
}

async fn list_snippets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Snippet>>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state
        .db
        .list_snippets()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn create_snippet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SnippetInput>,
) -> Result<Json<Snippet>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    let id = state
        .db
        .create_snippet(&input)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let snippets = state.db.list_snippets().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let snippet = snippets.into_iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(snippet))
}

async fn update_snippet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<SnippetInput>,
) -> Result<Json<Snippet>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state
        .db
        .update_snippet(id, &input)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let snippets = state.db.list_snippets().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let snippet = snippets.into_iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(snippet))
}

async fn delete_snippet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state
        .db
        .delete_snippet(id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
