use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::model::{SpeedLimit, Tunnel};

pub async fn get_tunnel_by_id(pool: &SqlitePool, id: i64) -> AppResult<Option<Tunnel>> {
    Ok(sqlx::query_as::<_, Tunnel>(
        "SELECT id, name, traffic_ratio, type, protocol, flow, created_time, updated_time, status, in_ip FROM tunnel WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn create(pool: &SqlitePool, sl: &SpeedLimit) -> AppResult<i64> {
    let res = sqlx::query(
        "INSERT INTO speed_limit (name, speed, tunnel_id, tunnel_name, created_time, updated_time, status)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&sl.name)
    .bind(sl.speed)
    .bind(sl.tunnel_id)
    .bind(&sl.tunnel_name)
    .bind(sl.created_time)
    .bind(sl.updated_time)
    .bind(sl.status)
    .execute(pool)
    .await?;
    Ok(res.last_insert_rowid())
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<SpeedLimit>> {
    Ok(sqlx::query_as::<_, SpeedLimit>(
        "SELECT id, name, speed, tunnel_id, tunnel_name, created_time, updated_time, status FROM speed_limit",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn get_by_id(pool: &SqlitePool, id: i64) -> AppResult<Option<SpeedLimit>> {
    Ok(sqlx::query_as::<_, SpeedLimit>(
        "SELECT id, name, speed, tunnel_id, tunnel_name, created_time, updated_time, status FROM speed_limit WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn update(pool: &SqlitePool, sl: &SpeedLimit) -> AppResult<()> {
    sqlx::query("UPDATE speed_limit SET name = ?, speed = ?, updated_time = ? WHERE id = ?")
        .bind(&sl.name)
        .bind(sl.speed)
        .bind(sl.updated_time)
        .bind(sl.id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM speed_limit WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 返回隧道链上存在的节点 ID（节点表有记录才返回）
pub async fn list_chain_node_ids(pool: &SqlitePool, tunnel_id: i64) -> AppResult<Vec<i64>> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT ct.node_id FROM chain_tunnel ct
         INNER JOIN node n ON n.id = ct.node_id
         WHERE ct.tunnel_id = ?",
    )
    .bind(tunnel_id)
    .fetch_all(pool)
    .await?)
}

pub async fn count_user_tunnel_by_speed_id(pool: &SqlitePool, speed_id: i64) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM user_tunnel WHERE speed_id = ?")
            .bind(speed_id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn list_all_tunnels(pool: &SqlitePool) -> AppResult<Vec<Tunnel>> {
    Ok(sqlx::query_as::<_, Tunnel>(
        "SELECT id, name, traffic_ratio, type, protocol, flow, created_time, updated_time, status, in_ip FROM tunnel",
    )
    .fetch_all(pool)
    .await?)
}
