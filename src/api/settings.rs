use axum::{Json, extract::State};

use crate::{AppState, database, model::CommentSettings, utils::AppResult};

/// GET /api/settings
pub async fn route_get_settings(State(state): State<AppState>) -> AppResult<Json<CommentSettings>> {
    Ok(Json(database::get_settings(&state.db).await?))
}

/// PUT /api/settings
pub async fn route_update_settings(
    State(state): State<AppState>,
    Json(body): Json<CommentSettings>,
) -> AppResult<Json<CommentSettings>> {
    let provider = match body.captcha_provider.as_str() {
        "hcaptcha" | "recaptcha" => body.captcha_provider,
        _ => "none".to_string(),
    };
    let trim_opt = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let clean = CommentSettings {
        require_moderation: body.require_moderation,
        collect_email: body.collect_email,
        captcha_provider: provider,
        captcha_site_key: trim_opt(body.captcha_site_key),
        captcha_secret: trim_opt(body.captcha_secret),
    };
    database::update_settings(&state.db, &clean).await?;
    Ok(Json(database::get_settings(&state.db).await?))
}
