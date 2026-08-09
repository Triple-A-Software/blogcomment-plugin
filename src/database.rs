use std::collections::{HashMap, HashSet};

use sqlx::PgPool;

use crate::{
    model::{Comment, CommentPublic, CommentSettings, CmsPost, Stats},
    utils::AppResult,
};

// ---------------------------------------------------------------------------
// CMS reads
// ---------------------------------------------------------------------------

/// Look up a public, non-deleted post by id — used to validate a comment's
/// target before storing it.
pub async fn get_cms_post(cms_db: &PgPool, id: i32) -> AppResult<Option<CmsPost>> {
    Ok(sqlx::query_as(
        r#"select id, title, slug
           from post
           where id = $1 and status = 'public' and deleted_at is null
           limit 1"#,
    )
    .bind(id)
    .fetch_optional(cms_db)
    .await?)
}

/// Resolve a public post by slug — the fallback the `comments` helper uses when
/// no explicit post id is passed (it reads the `:slug` route param).
pub async fn get_cms_post_by_slug(cms_db: &PgPool, slug: &str) -> AppResult<Option<CmsPost>> {
    Ok(sqlx::query_as(
        r#"select id, title, slug
           from post
           where slug = $1 and status = 'public' and deleted_at is null
           limit 1"#,
    )
    .bind(slug)
    .fetch_optional(cms_db)
    .await?)
}

/// Fill in `post_title` on a batch of comments from the CMS (separate database,
/// so this is a second query rather than a join).
pub async fn attach_titles(cms_db: &PgPool, comments: &mut [Comment]) -> AppResult<()> {
    if comments.is_empty() {
        return Ok(());
    }
    let ids: Vec<i32> = comments
        .iter()
        .map(|c| c.post_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let posts: Vec<CmsPost> = sqlx::query_as(r#"select id, title, slug from post where id = any($1)"#)
        .bind(&ids)
        .fetch_all(cms_db)
        .await?;
    let titles: HashMap<i32, String> = posts.into_iter().map(|p| (p.id, p.title)).collect();
    for c in comments.iter_mut() {
        c.post_title = titles.get(&c.post_id).cloned();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Comments — public
// ---------------------------------------------------------------------------

/// Approved comments for a post, oldest first (how a thread reads), each with
/// its like count. The caller assembles them into a reply tree via `parent_id`.
pub async fn approved_comments(db: &PgPool, post_id: i32) -> AppResult<Vec<CommentPublic>> {
    Ok(sqlx::query_as(
        r#"select c.id, c.parent_id, c.author_name, c.body, c.created_at,
                  (select count(*) from comment_reaction r where r.comment_id = c.id) as likes
           from comment c
           where c.post_id = $1 and c.status = 'approved'
           order by c.created_at asc"#,
    )
    .bind(post_id)
    .fetch_all(db)
    .await?)
}

/// A reply's parent must be an approved comment on the same post.
pub async fn is_valid_parent(db: &PgPool, parent_id: i64, post_id: i32) -> AppResult<bool> {
    Ok(sqlx::query_scalar::<_, bool>(
        r#"select exists(
               select 1 from comment
               where id = $1 and post_id = $2 and status = 'approved'
           )"#,
    )
    .bind(parent_id)
    .bind(post_id)
    .fetch_one(db)
    .await?)
}

/// The email of a comment's author, if they left one — used to notify them of a
/// reply.
pub async fn author_email(db: &PgPool, id: i64) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar::<_, Option<String>>(r#"select author_email from comment where id = $1"#)
        .bind(id)
        .fetch_optional(db)
        .await?
        .flatten())
}

/// Record a like for an approved comment (deduped per IP) and return the new
/// total. Returns `None` when the comment isn't approved / doesn't exist.
pub async fn react(db: &PgPool, comment_id: i64, ip: &str) -> AppResult<Option<i64>> {
    let approved = sqlx::query_scalar::<_, String>(r#"select status from comment where id = $1"#)
        .bind(comment_id)
        .fetch_optional(db)
        .await?;
    if approved.as_deref() != Some("approved") {
        return Ok(None);
    }
    sqlx::query(
        r#"insert into comment_reaction (comment_id, ip) values ($1, $2)
           on conflict do nothing"#,
    )
    .bind(comment_id)
    .bind(ip)
    .execute(db)
    .await?;
    Ok(Some(
        sqlx::query_scalar::<_, i64>(r#"select count(*) from comment_reaction where comment_id = $1"#)
            .bind(comment_id)
            .fetch_one(db)
            .await?,
    ))
}

/// GDPR erasure for a data subject identified by email. `anonymize` keeps the
/// comment text but strips identifying fields; otherwise the comments are
/// deleted outright. Returns the number of rows affected.
pub async fn erase_by_email(db: &PgPool, email: &str, anonymize: bool) -> AppResult<u64> {
    let result = if anonymize {
        sqlx::query(
            r#"update comment
               set author_name = 'Anonym', author_email = null, ip = null, ua = null
               where author_email = $1"#,
        )
        .bind(email)
        .execute(db)
        .await?
    } else {
        sqlx::query(r#"delete from comment where author_email = $1"#)
            .bind(email)
            .execute(db)
            .await?
    };
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_comment(
    db: &PgPool,
    post_id: i32,
    parent_id: Option<i64>,
    author_name: &str,
    author_email: Option<&str>,
    body: &str,
    status: &str,
    ip: Option<&str>,
    ua: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"insert into comment
               (post_id, parent_id, author_name, author_email, body, status, ip, ua)
           values ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    )
    .bind(post_id)
    .bind(parent_id)
    .bind(author_name)
    .bind(author_email)
    .bind(body)
    .bind(status)
    .bind(ip)
    .bind(ua)
    .execute(db)
    .await?;
    Ok(())
}

/// Comments posted from `ip` in the last `secs` seconds (per-IP flood guard).
pub async fn recent_count_by_ip(db: &PgPool, ip: &str, secs: i32) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        r#"select count(*) from comment
           where ip = $1 and created_at > now() - make_interval(secs => $2)"#,
    )
    .bind(ip)
    .bind(secs)
    .fetch_one(db)
    .await?)
}

/// Comments posted anywhere in the last `secs` seconds (global flood guard, a
/// backstop for when the real client IP isn't available behind the proxy).
pub async fn recent_count_global(db: &PgPool, secs: i32) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        r#"select count(*) from comment
           where created_at > now() - make_interval(secs => $1)"#,
    )
    .bind(secs)
    .fetch_one(db)
    .await?)
}

// ---------------------------------------------------------------------------
// Comments — moderation
// ---------------------------------------------------------------------------

/// List comments, optionally filtered by status (`None` = all).
pub async fn list_comments(db: &PgPool, status: Option<&str>) -> AppResult<Vec<Comment>> {
    Ok(sqlx::query_as(
        r#"select id, post_id, author_name, author_email, body, status, created_at,
                  null::text as post_title
           from comment
           where $1::text is null or status = $1
           order by created_at desc
           limit 500"#,
    )
    .bind(status)
    .fetch_all(db)
    .await?)
}

/// Apply a moderation action. Returns false for an unknown action.
pub async fn moderate(db: &PgPool, id: i64, action: &str) -> AppResult<bool> {
    let new_status = match action {
        "approve" => "approved",
        "spam" => "spam",
        "pending" => "pending",
        "delete" => {
            sqlx::query(r#"delete from comment where id = $1"#)
                .bind(id)
                .execute(db)
                .await?;
            return Ok(true);
        }
        _ => return Ok(false),
    };
    sqlx::query(r#"update comment set status = $2 where id = $1"#)
        .bind(id)
        .bind(new_status)
        .execute(db)
        .await?;
    Ok(true)
}

pub async fn stats(db: &PgPool, cms_db: &PgPool) -> AppResult<Stats> {
    let count = |status: &'static str| async move {
        sqlx::query_scalar::<_, i64>("select count(*) from comment where status = $1")
            .bind(status)
            .fetch_one(db)
            .await
    };
    let mut recent: Vec<Comment> = sqlx::query_as(
        r#"select id, post_id, author_name, author_email, body, status, created_at,
                  null::text as post_title
           from comment
           where status = 'approved'
           order by created_at desc
           limit 8"#,
    )
    .fetch_all(db)
    .await?;
    attach_titles(cms_db, &mut recent).await?;

    let pending = count("pending").await?;
    let approved = count("approved").await?;
    let spam = count("spam").await?;
    Ok(Stats {
        pending,
        approved,
        spam,
        total: pending + approved + spam,
        recent,
    })
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

pub async fn get_settings(db: &PgPool) -> AppResult<CommentSettings> {
    Ok(sqlx::query_as(
        r#"insert into comment_settings (id) values ('settings')
           on conflict (id) do update set id = 'settings'
           returning require_moderation, collect_email, captcha_provider,
                     captcha_site_key, captcha_secret, notify_email, akismet_key"#,
    )
    .fetch_one(db)
    .await?)
}

pub async fn update_settings(db: &PgPool, s: &CommentSettings) -> AppResult<()> {
    sqlx::query(
        r#"insert into comment_settings
               (id, require_moderation, collect_email, captcha_provider, captcha_site_key,
                captcha_secret, notify_email, akismet_key)
           values ('settings', $1, $2, $3, $4, $5, $6, $7)
           on conflict (id) do update set
               require_moderation = excluded.require_moderation,
               collect_email = excluded.collect_email,
               captcha_provider = excluded.captcha_provider,
               captcha_site_key = excluded.captcha_site_key,
               captcha_secret = excluded.captcha_secret,
               notify_email = excluded.notify_email,
               akismet_key = excluded.akismet_key"#,
    )
    .bind(s.require_moderation)
    .bind(s.collect_email)
    .bind(&s.captcha_provider)
    .bind(s.captcha_site_key.as_deref())
    .bind(s.captcha_secret.as_deref())
    .bind(s.notify_email.as_deref())
    .bind(s.akismet_key.as_deref())
    .execute(db)
    .await?;
    Ok(())
}
