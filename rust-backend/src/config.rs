use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: String,
    pub db_path: String,
    pub jwt_secret: String,
    pub log_dir: String,
}

impl Config {
    pub fn load() -> Self {
        Self {
            port: getenv("PORT", "6365"),
            db_path: getenv("DB_PATH", "./data/gost.db"),
            jwt_secret: getenv("JWT_SECRET", "flux-panel-dev-secret"),
            log_dir: getenv("LOG_DIR", "./logs"),
        }
    }
}

fn getenv(key: &str, def: &str) -> String {
    match env::var(key) {
        Ok(v) if !v.is_empty() => v,
        _ => def.to_string(),
    }
}

pub fn getenv_int(key: &str, def: i32) -> i32 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(def)
}
