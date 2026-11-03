use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use std::collections::HashMap;

use crate::error::AppResult;
use crate::model::ForwardPort;

pub async fn list_by_forward_id(pool: &SqlitePool, forward_id: i64) -> AppResult<Vec<ForwardPort>> {
    Ok(sqlx::query_as::<_, ForwardPort>(
        "SELECT id, forward_id, node_id, port FROM forward_port WHERE forward_id=?",
    )
    .bind(forward_id)
    .fetch_all(pool)
    .await?)
}

/// 批量查询多条转发的端口记录（N+1 优化）
pub async fn list_by_forward_ids(
    pool: &SqlitePool,
    forward_ids: &[i64],
) -> AppResult<HashMap<i64, Vec<ForwardPort>>> {
    if forward_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, forward_id, node_id, port FROM forward_port WHERE forward_id IN (",
    );
    {
        let mut sep = qb.separated(", ");
        for id in forward_ids {
            sep.push_bind(*id);
        }
    }
    qb.push(") ORDER BY id");
    let list: Vec<ForwardPort> = qb.build_query_as().fetch_all(pool).await?;

    let mut by_fwd: HashMap<i64, Vec<ForwardPort>> = HashMap::with_capacity(forward_ids.len());
    for fp in list {
        by_fwd.entry(fp.forward_id).or_default().push(fp);
    }
    Ok(by_fwd)
}

pub async fn get_by_forward_and_node(
    pool: &SqlitePool,
    forward_id: i64,
    node_id: i64,
) -> AppResult<Option<ForwardPort>> {
    Ok(sqlx::query_as::<_, ForwardPort>(
        "SELECT id, forward_id, node_id, port FROM forward_port WHERE forward_id=? AND node_id=?",
    )
    .bind(forward_id)
    .bind(node_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn list_by_node_exclude_forward(
    pool: &SqlitePool,
    node_id: i64,
    exclude_forward_id: i64,
) -> AppResult<Vec<ForwardPort>> {
    Ok(sqlx::query_as::<_, ForwardPort>(
        "SELECT id, forward_id, node_id, port FROM forward_port WHERE node_id=? AND forward_id<>?",
    )
    .bind(node_id)
    .bind(exclude_forward_id)
    .fetch_all(pool)
    .await?)
}

pub async fn insert(pool: &SqlitePool, fp: &ForwardPort) -> AppResult<i64> {
    let res = sqlx::query("INSERT INTO forward_port (forward_id, node_id, port) VALUES (?, ?, ?)")
        .bind(fp.forward_id)
        .bind(fp.node_id)
        .bind(fp.port)
        .execute(pool)
        .await?;
    Ok(res.last_insert_rowid())
}

pub async fn update_port(pool: &SqlitePool, id: i64, port: i32) -> AppResult<()> {
    sqlx::query("UPDATE forward_port SET port=? WHERE id=?")
        .bind(port)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_by_forward_id(pool: &SqlitePool, forward_id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM forward_port WHERE forward_id=?")
        .bind(forward_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 批量删除多条转发的端口记录（分片单语句 IN）
pub async fn delete_by_forward_ids(pool: &SqlitePool, forward_ids: &[i64]) -> AppResult<()> {
    if forward_ids.is_empty() {
        return Ok(());
    }
    for chunk in forward_ids.chunks(500) {
        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("DELETE FROM forward_port WHERE forward_id IN (");
        {
            let mut sep = qb.separated(", ");
            for id in chunk {
                sep.push_bind(*id);
            }
        }
        qb.push(")");
        qb.build().execute(pool).await?;
    }
    Ok(())
}

pub async fn list_by_node_id(pool: &SqlitePool, node_id: i64) -> AppResult<Vec<ForwardPort>> {
    Ok(sqlx::query_as::<_, ForwardPort>(
        "SELECT id, forward_id, node_id, port FROM forward_port WHERE node_id=?",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await?)
}

/// 批量查询多个节点的转发端口（N+1 优化）
pub async fn list_by_node_ids(
    pool: &SqlitePool,
    node_ids: &[i64],
) -> AppResult<Vec<ForwardPort>> {
    if node_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, forward_id, node_id, port FROM forward_port WHERE node_id IN (",
    );
    {
        let mut sep = qb.separated(", ");
        for id in node_ids {
            sep.push_bind(*id);
        }
    }
    qb.push(")");
    Ok(qb.build_query_as().fetch_all(pool).await?)
}

pub async fn used_ports_on_node(pool: &SqlitePool, node_id: i64) -> AppResult<Vec<i32>> {
    Ok(sqlx::query_scalar::<_, i32>(
        "SELECT port FROM forward_port WHERE node_id=?",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await?)
}

pub async fn delete_by_forward_and_node(
    pool: &SqlitePool,
    forward_id: i64,
    node_id: i64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM forward_port WHERE forward_id=? AND node_id=?")
        .bind(forward_id)
        .bind(node_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_by_node_id(pool: &SqlitePool, node_id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM forward_port WHERE node_id=?")
        .bind(node_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_by_node_and_forward_ids(
    pool: &SqlitePool,
    node_id: i64,
    forward_ids: &[i64],
) -> AppResult<()> {
    if forward_ids.is_empty() {
        return Ok(());
    }
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("DELETE FROM forward_port WHERE node_id=");
    qb.push_bind(node_id);
    qb.push(" AND forward_id IN (");
    {
        let mut sep = qb.separated(", ");
        for id in forward_ids {
            sep.push_bind(*id);
        }
    }
    qb.push(")");
    qb.build().execute(pool).await?;
    Ok(())
}

pub async fn insert_one(pool: &SqlitePool, fp: &mut ForwardPort) -> AppResult<()> {
    let id = insert(pool, fp).await?;
    fp.id = id;
    Ok(())
}
