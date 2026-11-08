use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

use crate::error::{AppError, AppResult};
use crate::model::ViteConfig;
use crate::repo::config as repo;

/// 未登录可读的公开键白名单（登录页/站点名展示必需，其余一律需鉴权）
pub fn public_read_whitelist() -> HashSet<&'static str> {
    ["app_name", "captcha_enabled", "captcha_type"]
        .into_iter()
        .collect()
}

pub fn is_valid_config_key(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    name.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
}

pub async fn get_public_by_name(pool: &SqlitePool, name: &str) -> AppResult<ViteConfig> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::new("配置名称不能为空"));
    }
    if !public_read_whitelist().contains(name) {
        return Err(AppError::new("配置不存在"));
    }
    match repo::find_by_name(pool, name).await? {
        Some(c) => Ok(c),
        None => Err(AppError::new("配置不存在")),
    }
}

pub async fn get_by_name(pool: &SqlitePool, name: &str) -> AppResult<ViteConfig> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::new("配置名称不能为空"));
    }
    match repo::find_by_name(pool, name).await? {
        Some(c) => Ok(c),
        None => Err(AppError::new("配置不存在")),
    }
}

pub async fn list_map(pool: &SqlitePool) -> AppResult<HashMap<String, String>> {
    let list = repo::list_all(pool).await?;
    Ok(list.into_iter().map(|c| (c.name, c.value)).collect())
}

pub async fn update_map(pool: &SqlitePool, config_map: HashMap<String, String>) -> AppResult<()> {
    if config_map.is_empty() {
        return Err(AppError::new("配置数据不能为空"));
    }
    if config_map.len() > 100 {
        return Err(AppError::new("配置数据过多"));
    }
    for (name, value) in config_map {
        if name.trim().is_empty() {
            continue;
        }
        if !is_valid_config_key(name.trim()) {
            return Err(AppError::new("配置名称不合法"));
        }
        if value.len() > 65535 {
            return Err(AppError::new("配置值过长"));
        }
        repo::upsert(pool, &name, &value).await?;
    }
    Ok(())
}

pub async fn update_single(pool: &SqlitePool, name: &str, value: &str) -> AppResult<()> {
    if name.trim().is_empty() {
        return Err(AppError::new("配置名称不能为空"));
    }
    if !is_valid_config_key(name.trim()) {
        return Err(AppError::new("配置名称不合法"));
    }
    if value.trim().is_empty() {
        return Err(AppError::new("配置值不能为空"));
    }
    if value.len() > 65535 {
        return Err(AppError::new("配置值过长"));
    }
    repo::upsert(pool, name, value).await
}

pub async fn is_captcha_enabled(pool: &SqlitePool) -> bool {
    match repo::find_by_name(pool, "captcha_enabled").await {
        Ok(Some(c)) => c.value == "true",
        _ => false,
    }
}
