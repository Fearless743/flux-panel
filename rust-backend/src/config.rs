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
            jwt_secret: env::var("JWT_SECRET").unwrap_or_default(),
            log_dir: getenv("LOG_DIR", "./logs"),
        }
    }

    /// JWT_SECRET 强度校验：禁止缺省/默认值，至少 32 字符
    pub fn validate(&self) -> Result<(), String> {
        if self.jwt_secret.is_empty() {
            return Err("JWT_SECRET 未设置，拒绝启动（请设置至少 32 字符的随机密钥）".to_string());
        }
        if self.jwt_secret == "flux-panel-dev-secret" {
            return Err("JWT_SECRET 仍为开发默认值，拒绝启动".to_string());
        }
        if self.jwt_secret.chars().count() < 32 {
            return Err("JWT_SECRET 过短（至少 32 字符），拒绝启动".to_string());
        }
        Ok(())
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
