use axum::{extract::State, response::Html};
use minijinja::context;
use serde::Serialize;

use crate::{AppState, database, model::Comment, utils::AppResult};

/// A comment reduced to the fields a dashboard card shows. minijinja
/// autoescapes `.html` templates, so these raw strings are escaped on render.
#[derive(Serialize)]
struct CardRow {
    author: String,
    snippet: String,
    date: String,
    post_title: String,
}

fn card_rows(comments: Vec<Comment>) -> Vec<CardRow> {
    comments
        .into_iter()
        .map(|c| CardRow {
            author: c.author_name,
            snippet: truncate(&c.body, 90),
            date: c.created_at.format("%d.%m.%Y %H:%M").to_string(),
            post_title: c.post_title.unwrap_or_else(|| format!("Post #{}", c.post_id)),
        })
        .collect()
}

fn truncate(s: &str, max: usize) -> String {
    let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max {
        return collapsed;
    }
    let cut: String = collapsed.chars().take(max).collect();
    format!("{}…", cut.trim_end())
}

/// GET /dashboard/pending — comments awaiting moderation.
pub async fn dashboard_pending(State(state): State<AppState>) -> AppResult<Html<String>> {
    let stats = database::stats(&state.db, &state.cms_db).await?;
    let mut pending = database::list_comments(&state.db, Some("pending")).await?;
    pending.truncate(6);
    database::attach_titles(&state.cms_db, &mut pending).await?;
    let tmpl = state.env.get_template("dashboard_pending.html")?;
    Ok(Html(tmpl.render(context! {
        count => stats.pending,
        comments => card_rows(pending),
    })?))
}

/// GET /dashboard/recent — recently approved comments.
pub async fn dashboard_recent(State(state): State<AppState>) -> AppResult<Html<String>> {
    let stats = database::stats(&state.db, &state.cms_db).await?;
    let tmpl = state.env.get_template("dashboard_recent.html")?;
    Ok(Html(tmpl.render(context! {
        approved => stats.approved,
        comments => card_rows(stats.recent),
    })?))
}
