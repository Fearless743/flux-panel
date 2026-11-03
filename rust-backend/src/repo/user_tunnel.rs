use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};

use crate::error::AppResult;
use crate::model::UserTunnel;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserTunnelDetail {
    pub id: i64,
    pub user_id: i64,
    pub tunnel_id: i64,
    pub flow: i64,
    pub in_flow: i64,
    pub out_flow: i64,
    pub num: i64,
    pub flow_reset_time: i64,
    pub exp_time: i64,
    pub speed_id: Option<i64>,
    pub status: i32,
    pub tunnel_name: Option<String>,
    pub tunnel_flow: Option<i64>,
    pub in_ip: Option<String>,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub r#type: Option<i32>,
    pub protocol: Option<String>,
    pub speed_limit_name: Option<String>,
    pub speed: Option<i32>,
}

pub async fn count_by_user_and_tunnel(
    pool: &SqlitePool,
    user_id: i64,
    tunnel_id: i64,
) -> AppResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(1) FROM user_tunnel WHERE user_id = ? AND tunnel_id = ?",
    )
    .bind(user_id)
    .bind(tunnel_id)
    .fetch_one(pool)
    .await?)
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> AppResult<Option<UserTunnel>> {
    Ok(
        sqlx::query_as::<_, UserTunnel>("SELECT * FROM user_tunnel WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn find_by_user_and_tunnel(
    pool: &SqlitePool,
    user_id: i64,
    tunnel_id: i64,
) -> AppResult<Option<UserTunnel>> {
    Ok(sqlx::query_as::<_, UserTunnel>(
        "SELECT * FROM user_tunnel WHERE user_id = ? AND tunnel_id = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(tunnel_id)
    .fetch_optional(pool)
    .await?)
}

fn chunk_ids(ids: &[i64], size: usize) -> Vec<&[i64]> {
    ids.chunks(size).collect()
}

// ListByTunnelIDs 批量查询多个隧道下的用户权限（批量操作预取，避免 N+1）
pub async fn list_by_tunnel_ids(
    pool: &SqlitePool,
    tunnel_ids: &[i64],
) -> AppResult<Vec<UserTunnel>> {
    if tunnel_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut out: Vec<UserTunnel> = Vec::new();
    for chunk in chunk_ids(tunnel_ids, 500) {
        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT * FROM user_tunnel WHERE tunnel_id IN (");
        let mut sep = qb.separated(", ");
        for id in chunk {
            sep.push_bind(*id);
        }
        sep.push_unseparated(")");
        out.extend(qb.build_query_as::<UserTunnel>().fetch_all(pool).await?);
    }
    Ok(out)
}

// ListByUserAndTunnelIDs 批量查询某用户在多个隧道上的权限（批量操作预取）
pub async fn list_by_user_and_tunnel_ids(
    pool: &SqlitePool,
    user_id: i64,
    tunnel_ids: &[i64],
) -> AppResult<Vec<UserTunnel>> {
    if tunnel_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut out: Vec<UserTunnel> = Vec::new();
    for chunk in chunk_ids(tunnel_ids, 500) {
        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT * FROM user_tunnel WHERE user_id = ? AND tunnel_id IN (");
        qb.push_bind(user_id);
        let mut sep = qb.separated(", ");
        for id in chunk {
            sep.push_bind(*id);
        }
        sep.push_unseparated(")");
        out.extend(qb.build_query_as::<UserTunnel>().fetch_all(pool).await?);
    }
    Ok(out)
}

pub async fn list_by_user_id(pool: &SqlitePool, user_id: i64) -> AppResult<Vec<UserTunnel>> {
    Ok(
        sqlx::query_as::<_, UserTunnel>("SELECT * FROM user_tunnel WHERE user_id = ? ORDER BY id")
            .bind(user_id)
            .fetch_all(pool)
            .await?,
    )
}

pub async fn list_details_by_user_id(
    pool: &SqlitePool,
    user_id: i64,
) -> AppResult<Vec<UserTunnelDetail>> {
    Ok(sqlx::query_as::<_, UserTunnelDetail>(
        "
		SELECT
			ut.id,
			ut.user_id,
			ut.tunnel_id,
			ut.flow,
			ut.in_flow,
			ut.out_flow,
			ut.num,
			ut.flow_reset_time,
			ut.exp_time,
			ut.speed_id,
			ut.status,
			t.name AS tunnel_name,
			t.flow AS tunnel_flow,
			t.in_ip,
			t.type,
			t.protocol,
			sl.name AS speed_limit_name,
			sl.speed
		FROM user_tunnel ut
		LEFT JOIN tunnel t ON ut.tunnel_id = t.id
		LEFT JOIN speed_limit sl ON ut.speed_id = sl.id
		WHERE ut.user_id = ?
		ORDER BY ut.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

pub async fn insert(pool: &SqlitePool, ut: &mut UserTunnel) -> AppResult<()> {
    let res = sqlx::query(
        "
		INSERT INTO user_tunnel (user_id, tunnel_id, speed_id, num, flow, in_flow, out_flow, flow_reset_time, exp_time, status)
		VALUES (?, ?, ?, ?, ?, 0, 0, ?, ?, ?)",
    )
    .bind(ut.user_id)
    .bind(ut.tunnel_id)
    .bind(ut.speed_id)
    .bind(ut.num)
    .bind(ut.flow)
    .bind(ut.flow_reset_time)
    .bind(ut.exp_time)
    .bind(ut.status)
    .execute(pool)
    .await?;
    ut.id = res.last_insert_rowid();
    Ok(())
}

pub async fn update(pool: &SqlitePool, ut: &UserTunnel) -> AppResult<()> {
    sqlx::query(
        "
		UPDATE user_tunnel
		SET flow = ?, num = ?, flow_reset_time = ?, exp_time = ?, status = ?, speed_id = ?
		WHERE id = ?",
    )
    .bind(ut.flow)
    .bind(ut.num)
    .bind(ut.flow_reset_time)
    .bind(ut.exp_time)
    .bind(ut.status)
    .bind(ut.speed_id)
    .bind(ut.id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM user_tunnel WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_by_tunnel_id(pool: &SqlitePool, tunnel_id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM user_tunnel WHERE tunnel_id = ?")
        .bind(tunnel_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn count_by_tunnel_id(pool: &SqlitePool, tunnel_id: i64) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM user_tunnel WHERE tunnel_id = ?")
            .bind(tunnel_id)
            .fetch_one(pool)
            .await?,
    )
}
