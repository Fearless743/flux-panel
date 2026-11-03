use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::error::AppResult;
use crate::model::Tunnel;

pub async fn count_by_name(pool: &SqlitePool, name: &str) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM tunnel WHERE name = ?")
            .bind(name)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> AppResult<Option<Tunnel>> {
    Ok(
        sqlx::query_as::<_, Tunnel>("SELECT * FROM tunnel WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn list_all(pool: &SqlitePool) -> AppResult<Vec<Tunnel>> {
    Ok(
        sqlx::query_as::<_, Tunnel>("SELECT * FROM tunnel ORDER BY id")
            .fetch_all(pool)
            .await?,
    )
}

pub async fn list_by_ids(pool: &SqlitePool, ids: &[i64]) -> AppResult<Vec<Tunnel>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT * FROM tunnel WHERE id IN (");
    let mut sep = qb.separated(", ");
    for id in ids {
        sep.push_bind(*id);
    }
    sep.push_unseparated(") AND status = 1");
    Ok(qb.build_query_as::<Tunnel>().fetch_all(pool).await?)
}

pub async fn list_enabled(pool: &SqlitePool) -> AppResult<Vec<Tunnel>> {
    Ok(
        sqlx::query_as::<_, Tunnel>("SELECT * FROM tunnel WHERE status = 1 ORDER BY id")
            .fetch_all(pool)
            .await?,
    )
}

pub async fn create(pool: &SqlitePool, t: &mut Tunnel) -> AppResult<()> {
    let res = sqlx::query(
        "
		INSERT INTO tunnel (name, traffic_ratio, type, protocol, flow, created_time, updated_time, status, in_ip)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&t.name)
    .bind(t.traffic_ratio)
    .bind(t.r#type)
    .bind(&t.protocol)
    .bind(t.flow)
    .bind(t.created_time)
    .bind(t.updated_time)
    .bind(t.status)
    .bind(&t.in_ip)
    .execute(pool)
    .await?;
    t.id = res.last_insert_rowid();
    Ok(())
}

pub async fn update_basic(
    pool: &SqlitePool,
    id: i64,
    name: &str,
    flow: i64,
    traffic_ratio: f64,
    in_ip: Option<&str>,
    updated_time: i64,
) -> AppResult<()> {
    sqlx::query(
        "
		UPDATE tunnel SET name = ?, flow = ?, traffic_ratio = ?, in_ip = ?, updated_time = ?
		WHERE id = ?",
    )
    .bind(name)
    .bind(flow)
    .bind(traffic_ratio)
    .bind(in_ip)
    .bind(updated_time)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_in_ip(
    pool: &SqlitePool,
    id: i64,
    in_ip: &str,
    updated_time: i64,
) -> AppResult<()> {
    sqlx::query("UPDATE tunnel SET in_ip = ?, updated_time = ? WHERE id = ?")
        .bind(in_ip)
        .bind(updated_time)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM tunnel WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub fn str_ptr(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}
