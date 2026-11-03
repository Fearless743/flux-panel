mod auth;
mod captcha;
mod config;
mod crypto;
mod db;
mod error;
mod gost;
mod handler;
mod middleware;
mod model;
mod repo;
mod response;
mod service;
mod state;
mod task;
mod ws;

use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = config::Config::load();
    let port = cfg.port.clone();
    let database = match db::open(&cfg.db_path).await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("open db: {}", e);
            std::process::exit(1);
        }
    };
    let jwt = auth::Jwt::new(cfg.jwt_secret.clone());
    let hub = ws::Hub::new();
    let state = AppState::new(database, jwt, hub, cfg);
    captcha::start_cleanup();
    task::tasks::start(state.clone());

    let app = handler::router(state);
    let addr = format!("0.0.0.0:{}", port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("bind {}: {}", addr, e);
            std::process::exit(1);
        }
    };
    tracing::info!("rust-backend listening on {}", addr);
    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("server exit: {}", e);
        std::process::exit(1);
    }
}
