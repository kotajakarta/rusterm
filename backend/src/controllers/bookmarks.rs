use crate::controllers::servers::{check_auth, AppState};
use crate::models::bookmark::{Bookmark, BookmarkInput};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/bookmarks", get(list_bookmarks).post(create_bookmark))
        .route(
            "/bookmarks/:id",
            get(get_bookmark).put(update_bookmark).delete(delete_bookmark),
        )
        .with_state(state)
}

async fn list_bookmarks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Bookmark>>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state
        .db
        .list_bookmarks()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn create_bookmark(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<BookmarkInput>,
) -> Result<Json<Bookmark>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    let id = state
        .db
        .create_bookmark(&input)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state
        .db
        .get_bookmark(id)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_bookmark(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Bookmark>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state
        .db
        .get_bookmark(id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

async fn update_bookmark(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<BookmarkInput>,
) -> Result<Json<Bookmark>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state
        .db
        .update_bookmark(id, &input)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state
        .db
        .get_bookmark(id)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn delete_bookmark(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state
        .db
        .delete_bookmark(id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
