use std::collections::HashMap;

use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::error::{AppError, AppResult};
use crate::model::Node;
use crate::response::now_ms;

fn null_listen(s: &str) -> String {
    if s.is_empty() {
        "[::]".to_string()
    } else {
        s.to_string()
    }
}

pub async fn create(pool: &SqlitePool, n: &mut Node) -> AppResult<()> {
    n.created_time = now_ms();
    n.updated_time = Some(n.created_time);
    let res = sqlx::query(
        "INSERT INTO node (name, secret, server_ip, port, interface_name, version, http, tls, socks,
            created_time, updated_time, status, tcp_listen_addr, udp_listen_addr)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&n.name)
    .bind(&n.secret)
    .bind(&n.server_ip)
    .bind(&n.port)
    .bind(&n.interface_name)
    .bind(&n.version)
    .bind(n.http)
    .bind(n.tls)
    .bind(n.socks)
    .bind(n.created_time)
    .bind(n.updated_time)
    .bind(n.status)
    .bind(null_listen(&n.tcp_listen_addr))
    .bind(null_listen(&n.udp_listen_addr))
    .execute(pool)
    .await?;
    n.id = res.last_insert_rowid();
    Ok(())
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> AppResult<Option<Node>> {
    Ok(sqlx::query_as::<_, Node>("SELECT * FROM node WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?)
}

pub async fn find_by_secret(pool: &SqlitePool, secret: &str) -> AppResult<Option<Node>> {
    Ok(
        sqlx::query_as::<_, Node>("SELECT * FROM node WHERE secret = ? LIMIT 1")
            .bind(secret)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn list_order_by_status_desc(pool: &SqlitePool) -> AppResult<Vec<Node>> {
    Ok(
        sqlx::query_as::<_, Node>("SELECT * FROM node ORDER BY status DESC, id DESC")
            .fetch_all(pool)
            .await?,
    )
}

pub async fn list_by_ids(pool: &SqlitePool, ids: &[i64]) -> AppResult<Vec<Node>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT * FROM node WHERE id IN (");
    let mut sep = qb.separated(", ");
    for id in ids {
        sep.push_bind(id);
    }
    sep.push_unseparated(")");
    Ok(qb.build_query_as::<Node>().fetch_all(pool).await?)
}

pub async fn map_by_ids(pool: &SqlitePool, ids: &[i64]) -> AppResult<HashMap<i64, Node>> {
    let list = list_by_ids(pool, ids).await?;
    let mut m = HashMap::with_capacity(list.len());
    for n in list {
        m.insert(n.id, n);
    }
    Ok(m)
}

pub async fn update(pool: &SqlitePool, n: &mut Node) -> AppResult<()> {
    n.updated_time = Some(now_ms());
    sqlx::query(
        "UPDATE node SET name=?, server_ip=?, port=?, interface_name=?,
            http=?, tls=?, socks=?, updated_time=?, tcp_listen_addr=?, udp_listen_addr=?
         WHERE id=?",
    )
    .bind(&n.name)
    .bind(&n.server_ip)
    .bind(&n.port)
    .bind(&n.interface_name)
    .bind(n.http)
    .bind(n.tls)
    .bind(n.socks)
    .bind(n.updated_time)
    .bind(null_listen(&n.tcp_listen_addr))
    .bind(null_listen(&n.udp_listen_addr))
    .bind(n.id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_online(
    pool: &SqlitePool,
    id: i64,
    version: Option<&str>,
    http: Option<i32>,
    tls: Option<i32>,
    socks: Option<i32>,
) -> AppResult<()> {
    let now = now_ms();
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("UPDATE node SET status = 1, updated_time = ");
    qb.push_bind(now);
    if let Some(version) = version {
        qb.push(", version = ");
        qb.push_bind(version);
    }
    if let Some(http) = http {
        qb.push(", http = ");
        qb.push_bind(http);
    }
    if let Some(tls) = tls {
        qb.push(", tls = ");
        qb.push_bind(tls);
    }
    if let Some(socks) = socks {
        qb.push(", socks = ");
        qb.push_bind(socks);
    }
    qb.push(" WHERE id = ");
    qb.push_bind(id);
    qb.build().execute(pool).await?;
    Ok(())
}

pub async fn update_status(pool: &SqlitePool, id: i64, status: i32) -> AppResult<()> {
    let now = now_ms();
    sqlx::query("UPDATE node SET status = ?, updated_time = ? WHERE id = ?")
        .bind(status)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let res = sqlx::query("DELETE FROM node WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::new("节点不存在"));
    }
    Ok(())
}

pub async fn get_vite_config(pool: &SqlitePool, name: &str) -> AppResult<String> {
    let value =
        sqlx::query_scalar::<_, String>("SELECT value FROM vite_config WHERE name = ? LIMIT 1")
            .bind(name)
            .fetch_optional(pool)
            .await?;
    Ok(value.unwrap_or_default())
}
