use sqlx::SqlitePool;
use std::sync::Arc;

use crate::auth::Jwt;
use crate::config::Config;
use crate::ws::Hub;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub jwt: Arc<Jwt>,
    pub hub: Arc<Hub>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: SqlitePool, jwt: Jwt, hub: Hub, config: Config) -> Self {
        Self {
            db,
            jwt: Arc::new(jwt),
            hub: Arc::new(hub),
            config: Arc::new(config),
        }
    }
}
