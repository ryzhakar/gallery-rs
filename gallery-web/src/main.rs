mod handlers;
mod state;

use anyhow::Result;
use axum::{
    routing::get,
    Router,
};
use std::env;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gallery_web=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get configuration from environment
    let bucket = env::var("GALLERY_BUCKET")
        .expect("GALLERY_BUCKET environment variable must be set");
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());

    // Create app state
    let state = AppState::new(bucket).await?;

    // Build router
    let app = Router::new()
        .route("/", get(handlers::index))
        .route("/gallery/:album_id", get(handlers::gallery))
        .route("/api/album/:album_id/manifest", get(handlers::get_manifest))
        .route("/api/album/:album_id/image/*path", get(handlers::get_image))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Start server
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Gallery web server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
