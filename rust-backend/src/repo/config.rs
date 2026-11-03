use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::model::ViteConfig;

pub async fn find_by_name(pool: &SqlitePool, name: &str) -> AppResult<Option<ViteConfig>> {
    let row = sqlx::query_as::<_, ViteConfig>(
        "SELECT * FROM vite_config WHERE name = ? LIMIT 1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_all(pool: &SqlitePool) -> AppResult<Vec<ViteConfig>> {
    let list = sqlx::query_as::<_, ViteConfig>("SELECT * FROM vite_config ORDER BY id")
        .fetch_all(pool)
        .await?;
    Ok(list)
}

pub async fn upsert(pool: &SqlitePool, name: &str, value: &str) -> AppResult<()> {
    let now = crate::response::now_ms();
    if let Some(existing) = find_by_name(pool, name).await? {
        sqlx::query("UPDATE vite_config SET value=?, time=? WHERE id=?")
            .bind(value)
            .bind(now)
            .bind(existing.id)
            .execute(pool)
            .await?;
    } else {
        sqlx::query("INSERT INTO vite_config (name, value, time) VALUES (?, ?, ?)")
            .bind(name)
            .bind(value)
            .bind(now)
            .execute(pool)
            .await?;
    }
    Ok(())
}
