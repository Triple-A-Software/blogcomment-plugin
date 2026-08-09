use std::{env, sync::Arc};

use minijinja::{Environment, path_loader};
use sqlx::PgPool;

pub mod antispam;
pub mod api;
pub mod database;
pub mod email;
pub mod model;
pub mod render;
pub mod utils;

/// Shared, cheaply-cloneable application state handed to every axum handler.
#[derive(Clone)]
pub struct AppState {
    /// The plugin's own database (comments + settings).
    pub db: PgPool,
    /// The Neleto CMS database, read to validate posts + show their titles.
    pub cms_db: PgPool,
    /// minijinja environment loading dashboard-card templates from `./templates`.
    pub env: Arc<Environment<'static>>,
}

/// Connect to the plugin's own database and run migrations.
pub async fn create_db() -> PgPool {
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = PgPool::connect(&db_url)
        .await
        .expect("failed to connect to plugin database");
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("failed to run migrations");
    db
}

/// Connect to the Neleto CMS database (read-only in practice).
pub async fn create_cms_db() -> PgPool {
    let db_url = env::var("CMS_DATABASE_URL").expect("CMS_DATABASE_URL must be set");
    PgPool::connect(&db_url)
        .await
        .expect("failed to connect to CMS database")
}

/// Build the template environment used for server-rendered dashboard cards.
pub fn create_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_loader(path_loader("./templates"));
    env
}
