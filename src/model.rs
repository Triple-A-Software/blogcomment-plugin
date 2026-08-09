use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ---------------------------------------------------------------------------
// CMS-side rows (read from CMS_DATABASE_URL)
// ---------------------------------------------------------------------------

/// A post as stored by the Neleto core in the `post` table.
#[derive(Debug, Clone, FromRow)]
pub struct CmsPost {
    pub id: i32,
    pub title: String,
    pub slug: String,
}

// ---------------------------------------------------------------------------
// Public rendering
// ---------------------------------------------------------------------------

/// The fields of an approved comment that are safe to render on the page.
#[derive(Debug, Clone, FromRow)]
pub struct CommentPublic {
    pub id: i64,
    pub author_name: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Admin / moderation
// ---------------------------------------------------------------------------

/// A comment row as returned to the moderation UI. `post_title` is joined in
/// from the CMS after loading (the two live in separate databases).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Comment {
    pub id: i64,
    pub post_id: i32,
    pub author_name: String,
    pub author_email: Option<String>,
    pub body: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    #[sqlx(default)]
    pub post_title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModerationRequest {
    pub id: i64,
    /// One of: approve | spam | pending | delete
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Stats {
    pub pending: i64,
    pub approved: i64,
    pub spam: i64,
    pub total: i64,
    pub recent: Vec<Comment>,
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CommentSettings {
    pub require_moderation: bool,
    pub collect_email: bool,
    /// 'none' | 'hcaptcha' | 'recaptcha'
    pub captcha_provider: String,
    pub captcha_site_key: Option<String>,
    pub captcha_secret: Option<String>,
}

impl Default for CommentSettings {
    fn default() -> Self {
        Self {
            require_moderation: true,
            collect_email: true,
            captcha_provider: "none".to_string(),
            captcha_site_key: None,
            captcha_secret: None,
        }
    }
}
