use sqlx::SqlitePool;

use crate::captcha::{default_store, GenerateResult};
use crate::error::AppResult;
use crate::service::config;

pub async fn check_enabled(pool: &SqlitePool) -> i32 {
    if config::is_captcha_enabled(pool).await {
        1
    } else {
        0
    }
}

pub fn generate() -> AppResult<GenerateResult> {
    default_store().generate()
}

pub fn verify(id: &str, code: &str) -> bool {
    default_store().verify(id, code)
}

pub fn secondary(id: &str) -> bool {
    default_store().secondary(id)
}
