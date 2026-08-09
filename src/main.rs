use std::{env, net::SocketAddr, sync::Arc};

use axum::{
    Router,
    routing::{get, post},
};
use blog_comments::{
    AppState,
    api::{admin, dashboard, public, settings},
    create_cms_db, create_db, create_env,
};
use tokio::net::TcpListener;
use tower_http::{
    normalize_path::NormalizePathLayer,
    services::ServeDir,
    trace::{self, TraceLayer},
};
use tracing::Level;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_target(false).compact().init();

    let db = create_db().await;
    let cms_db = create_cms_db().await;
    let state = AppState {
        db,
        cms_db,
        env: Arc::new(create_env()),
    };

    let router = Router::new()
        // Admin UI (static assets)
        .nest_service("/ui", ServeDir::new("ui/dist"))
        // Admin API
        .route("/api/comments", get(admin::list))
        .route("/api/comments/moderate", post(admin::moderate))
        .route("/api/comments/erase", post(admin::erase))
        .route("/api/stats", get(admin::stats))
        .route(
            "/api/settings",
            get(settings::route_get_settings).put(settings::route_update_settings),
        )
        // Dashboard cards (server-rendered HTML)
        .route("/dashboard/pending", get(dashboard::dashboard_pending))
        .route("/dashboard/recent", get(dashboard::dashboard_recent))
        // Render helper: {{ comments(post.id) }}
        .route("/helper/comments", post(public::comments_helper))
        // Public comment submit (the form always POSTs urlencoded data, which
        // Neleto forwards unchanged for an allow_select_layout=false page).
        .route("/comments/submit", post(public::submit))
        // Public like endpoint (JSON for enhanced clients, redirect otherwise).
        .route("/comments/react", post(public::react))
        .layer(NormalizePathLayer::trim_trailing_slash())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(state);

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("failed to bind port");
    println!("blog-comments listening on {}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
