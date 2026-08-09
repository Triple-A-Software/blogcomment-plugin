//! Visitor-facing surface: the `comments` render helper and the public
//! `/comments/submit` form handler.

use std::collections::HashMap;

use axum::{
    Json,
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use serde_json::Value;

use crate::{AppState, antispam, database, render};

// ---------------------------------------------------------------------------
// Helper: {{ comments(post.id) }}
// ---------------------------------------------------------------------------

/// Mirrors the response envelope every Neleto inline helper returns.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ServicePluginApiResponse<T> {
    #[allow(dead_code)]
    Error(String),
    Data(T),
}

type HelperResp = Json<ServicePluginApiResponse<String>>;

fn data(html: String) -> HelperResp {
    Json(ServicePluginApiResponse::Data(html))
}

#[derive(Deserialize)]
pub struct PageRenderInput {
    #[allow(dead_code)]
    pub language: Option<String>,
}

#[derive(Deserialize)]
pub struct HelperBody {
    pub json_args: Vec<Value>,
    #[allow(dead_code)]
    pub page: PageRenderInput,
    pub query: HashMap<String, String>,
    pub params: HashMap<String, String>,
    pub route: String,
    #[allow(dead_code)]
    pub interactive: bool,
}

/// Substitute `:param` segments of a route pattern with actual param values, so
/// the redirect target is the concrete page URL (e.g. `/blog/:slug` → `/blog/x`).
fn resolve_path(route: &str, params: &HashMap<String, String>) -> String {
    let path = route
        .split('/')
        .map(|seg| match seg.strip_prefix(':') {
            Some(key) => params.get(key).cloned().unwrap_or_default(),
            None => seg.to_string(),
        })
        .collect::<Vec<_>>()
        .join("/");
    if path.is_empty() { "/".to_string() } else { path }
}

/// `comments(post_id)` — renders the thread + submit form for a post. Prefers
/// the explicit numeric argument; falls back to resolving the post from the
/// `:slug` route param. Always returns *some* HTML (never fails a page render).
pub async fn comments_helper(State(state): State<AppState>, Json(body): Json<HelperBody>) -> HelperResp {
    let explicit = body
        .json_args
        .first()
        .and_then(Value::as_i64)
        .filter(|n| *n > 0)
        .map(|n| n as i32);

    let post_id = match explicit {
        Some(id) => Some(id),
        None => match body.params.get("slug") {
            Some(slug) => database::get_cms_post_by_slug(&state.cms_db, slug)
                .await
                .ok()
                .flatten()
                .map(|p| p.id),
            None => None,
        },
    };
    let Some(post_id) = post_id else {
        return data(String::new());
    };

    let comments = database::approved_comments(&state.db, post_id).await.unwrap_or_default();
    let settings = database::get_settings(&state.db).await.unwrap_or_default();
    let path = resolve_path(&body.route, &body.params);
    let flag = body.query.get("comment").map(String::as_str);

    data(render::thread_and_form(post_id, &path, &comments, &settings, flag))
}

// ---------------------------------------------------------------------------
// Public submit: POST /comments/submit
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CommentForm {
    pub post_id: Option<String>,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub body: Option<String>,
    pub redirect: Option<String>,
    /// Honeypot — humans never see this field, so any value means a bot.
    #[serde(default)]
    pub website: Option<String>,
    /// Millisecond load timestamp for the submit time-trap.
    #[serde(default)]
    pub ts: Option<String>,
    #[serde(rename = "h-captcha-response", default)]
    pub hcaptcha: Option<String>,
    #[serde(rename = "g-recaptcha-response", default)]
    pub recaptcha: Option<String>,
}

/// Handle a comment submission and redirect back to the post with a status flag.
/// The form is `application/x-www-form-urlencoded`; Neleto proxies this
/// (`allow_select_layout: false`) POST body through unchanged.
pub async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Form(form): axum::extract::Form<CommentForm>,
) -> Response {
    let redirect = sanitize_redirect(form.redirect.as_deref());

    // Honeypot trip → look successful, store nothing.
    if antispam::honeypot_tripped(form.website.as_deref()) {
        return redirect_back(&redirect, "received");
    }
    if antispam::too_fast(form.ts.as_deref(), 2) {
        return redirect_back(&redirect, "slow_down");
    }

    let post_id = match form.post_id.as_deref().and_then(|s| s.trim().parse::<i32>().ok()) {
        Some(id) if id > 0 => id,
        _ => return redirect_back(&redirect, "error"),
    };
    let name = form.author_name.as_deref().unwrap_or_default().trim().to_string();
    let body = form.body.as_deref().unwrap_or_default().trim().to_string();
    if name.is_empty() || body.is_empty() || name.chars().count() > 120 || body.chars().count() > 5000 {
        return redirect_back(&redirect, "error");
    }
    let email = form
        .author_email
        .as_deref()
        .map(str::trim)
        .filter(|s| s.contains('@') && s.chars().count() <= 254 && !s.is_empty())
        .map(str::to_string);

    match process(&state, &headers, post_id, &name, email.as_deref(), &body, &form).await {
        Ok(flag) => redirect_back(&redirect, flag),
        Err(e) => {
            tracing::error!("comment submit failed: {e}");
            redirect_back(&redirect, "error")
        }
    }
}

async fn process(
    state: &AppState,
    headers: &HeaderMap,
    post_id: i32,
    name: &str,
    email: Option<&str>,
    body: &str,
    form: &CommentForm,
) -> crate::utils::AppResult<&'static str> {
    // The post must exist and be public.
    if database::get_cms_post(&state.cms_db, post_id).await?.is_none() {
        return Ok("error");
    }
    let settings = database::get_settings(&state.db).await?;

    if !antispam::verify_captcha(&settings, form.hcaptcha.as_deref(), form.recaptcha.as_deref()).await {
        return Ok("captcha");
    }

    // Flood guards: per-IP when we can identify the client, plus a global cap.
    let ip = antispam::client_ip(headers);
    if let Some(ip) = ip.as_deref()
        && database::recent_count_by_ip(&state.db, ip, 60).await? >= 5
    {
        return Ok("slow_down");
    }
    if database::recent_count_global(&state.db, 60).await? >= 30 {
        return Ok("slow_down");
    }

    let ua = headers.get("user-agent").and_then(|v| v.to_str().ok());
    let status = if settings.require_moderation { "pending" } else { "approved" };
    database::insert_comment(&state.db, post_id, None, name, email, body, status, ip.as_deref(), ua).await?;

    Ok(if settings.require_moderation { "received" } else { "posted" })
}

/// Only allow same-site relative paths as the redirect target (no open redirect).
fn sanitize_redirect(redirect: Option<&str>) -> String {
    match redirect {
        Some(r) if r.starts_with('/') && !r.starts_with("//") => r.to_string(),
        _ => "/".to_string(),
    }
}

fn redirect_back(path: &str, flag: &str) -> Response {
    let base = path.split('?').next().unwrap_or(path);
    Redirect::to(&format!("{base}?comment={flag}#comments")).into_response()
}
