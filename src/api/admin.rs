use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState, database,
    model::{Comment, ModerationRequest, Stats},
    utils::{AppError, AppResult},
};

#[derive(Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
}

/// GET /api/comments?status=pending|approved|spam — moderation list.
pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Vec<Comment>>> {
    let status = q
        .status
        .as_deref()
        .filter(|s| ["pending", "approved", "spam"].contains(s));
    let mut comments = database::list_comments(&state.db, status).await?;
    database::attach_titles(&state.cms_db, &mut comments).await?;
    Ok(Json(comments))
}

/// POST /api/comments/moderate — {id, action: approve|spam|pending|delete}.
pub async fn moderate(
    State(state): State<AppState>,
    Json(req): Json<ModerationRequest>,
) -> AppResult<Json<Value>> {
    if !database::moderate(&state.db, req.id, &req.action).await? {
        return Err(AppError::BadRequest(format!("unknown action: {}", req.action)));
    }
    Ok(Json(json!({ "success": true })))
}

/// GET /api/stats — counts + recent approved comments.
pub async fn stats(State(state): State<AppState>) -> AppResult<Json<Stats>> {
    Ok(Json(database::stats(&state.db, &state.cms_db).await?))
}

#[derive(Deserialize)]
pub struct EraseRequest {
    pub email: String,
    /// Keep the comment text but strip identifying fields, instead of deleting.
    #[serde(default)]
    pub anonymize: bool,
}

/// POST /api/comments/erase — GDPR erasure of all comments for an email address.
pub async fn erase(
    State(state): State<AppState>,
    Json(req): Json<EraseRequest>,
) -> AppResult<Json<Value>> {
    let email = req.email.trim();
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::BadRequest("a valid email is required".into()));
    }
    let affected = database::erase_by_email(&state.db, email, req.anonymize).await?;
    Ok(Json(json!({ "affected": affected })))
}
