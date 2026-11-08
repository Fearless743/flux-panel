use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::time::Duration;

use crate::error::{AppError, AppResult};

pub async fn open(path: &str) -> AppResult<SqlitePool> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true)
        .pragma("cache_size", "-64000");

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .min_connections(1)
        .connect_with(options)
        .await?;

    apply_sql(&pool, include_str!("../migrations/schema.sql")).await?;
    apply_sql(&pool, include_str!("../migrations/data.sql")).await?;
    migrate(&pool).await?;
    tracing::info!("database ready: {}", path);
    Ok(pool)
}

async fn apply_sql(pool: &SqlitePool, sql: &str) -> AppResult<()> {
    for stmt in sql.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        sqlx::query(stmt).execute(pool).await.map_err(|e| {
            let preview: String = stmt.chars().take(80).collect();
            AppError::new(format!("sql exec {:?}: {}", preview, e))
        })?;
    }
    Ok(())
}

async fn migrate(pool: &SqlitePool) -> AppResult<()> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('chain_tunnel') WHERE name='exit_node_ids'",
    )
    .fetch_one(pool)
    .await?;
    if count == 0 {
        sqlx::query("ALTER TABLE chain_tunnel ADD COLUMN exit_node_ids TEXT")
            .execute(pool)
            .await?;
        tracing::info!("migrated: added exit_node_ids column to chain_tunnel");
    }
    Ok(())
}

pub async fn checkpoint(pool: &SqlitePool) {
    if let Err(e) = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await
    {
        tracing::warn!("wal checkpoint failed: {}", e);
    }
}

/// 便捷读取 i64 列
pub fn get_i64(row: &sqlx::sqlite::SqliteRow, col: &str) -> i64 {
    row.get::<i64, _>(col)
}
