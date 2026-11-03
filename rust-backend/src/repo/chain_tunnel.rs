use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::error::AppResult;
use crate::model::ChainTunnel;

// chainType 在 schema 中为 VARCHAR，业务用 "1"/"2"/"3"
pub fn chain_type_str(t: i32) -> String {
    t.to_string()
}

pub fn parse_chain_type(s: &str) -> i32 {
    s.parse::<i32>().unwrap_or(0)
}

pub fn chain_type_of(ct: &ChainTunnel) -> i32 {
    parse_chain_type(&ct.chain_type)
}

pub async fn list_by_tunnel_id(pool: &SqlitePool, tunnel_id: i64) -> AppResult<Vec<ChainTunnel>> {
    Ok(sqlx::query_as::<_, ChainTunnel>(
        "SELECT * FROM chain_tunnel WHERE tunnel_id = ? ORDER BY id",
    )
    .bind(tunnel_id)
    .fetch_all(pool)
    .await?)
}

pub async fn list_entries_by_tunnel_id(
    pool: &SqlitePool,
    tunnel_id: i64,
) -> AppResult<Vec<ChainTunnel>> {
    Ok(sqlx::query_as::<_, ChainTunnel>(
        "SELECT * FROM chain_tunnel WHERE tunnel_id = ? AND chain_type = '1' ORDER BY id",
    )
    .bind(tunnel_id)
    .fetch_all(pool)
    .await?)
}

pub async fn update_exit_node_ids(
    pool: &SqlitePool,
    id: i64,
    exit_node_ids: Option<&str>,
) -> AppResult<()> {
    sqlx::query("UPDATE chain_tunnel SET exit_node_ids = ? WHERE id = ?")
        .bind(exit_node_ids)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_by_tunnel_ids(
    pool: &SqlitePool,
    tunnel_ids: &[i64],
) -> AppResult<Vec<ChainTunnel>> {
    if tunnel_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT * FROM chain_tunnel WHERE tunnel_id IN (");
    let mut sep = qb.separated(", ");
    for id in tunnel_ids {
        sep.push_bind(*id);
    }
    sep.push_unseparated(") ORDER BY id");
    Ok(qb.build_query_as::<ChainTunnel>().fetch_all(pool).await?)
}

pub async fn list_by_node_id(pool: &SqlitePool, node_id: i64) -> AppResult<Vec<ChainTunnel>> {
    Ok(
        sqlx::query_as::<_, ChainTunnel>("SELECT * FROM chain_tunnel WHERE node_id = ?")
            .bind(node_id)
            .fetch_all(pool)
            .await?,
    )
}

// ListEntriesByTunnelIDs 批量查询多个隧道的入口节点（批量操作预取，避免 N+1）
pub async fn list_entries_by_tunnel_ids(
    pool: &SqlitePool,
    tunnel_ids: &[i64],
) -> AppResult<Vec<ChainTunnel>> {
    if tunnel_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT * FROM chain_tunnel WHERE tunnel_id IN (",
    );
    let mut sep = qb.separated(", ");
    for id in tunnel_ids {
        sep.push_bind(*id);
    }
    sep.push_unseparated(") AND chain_type = '1' ORDER BY id");
    Ok(qb.build_query_as::<ChainTunnel>().fetch_all(pool).await?)
}

// ListByNodeIDs 批量查询多个节点的链隧道（N+1 优化）
pub async fn list_by_node_ids(
    pool: &SqlitePool,
    node_ids: &[i64],
) -> AppResult<Vec<ChainTunnel>> {
    if node_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT * FROM chain_tunnel WHERE node_id IN (");
    let mut sep = qb.separated(", ");
    for id in node_ids {
        sep.push_bind(*id);
    }
    sep.push_unseparated(")");
    Ok(qb.build_query_as::<ChainTunnel>().fetch_all(pool).await?)
}

pub async fn list_by_node_id_with_port(
    pool: &SqlitePool,
    node_id: i64,
) -> AppResult<Vec<ChainTunnel>> {
    Ok(sqlx::query_as::<_, ChainTunnel>(
        "SELECT * FROM chain_tunnel WHERE node_id = ? AND port IS NOT NULL",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await?)
}

pub async fn insert_batch(pool: &SqlitePool, items: &mut [ChainTunnel]) -> AppResult<()> {
    if items.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    for ct in items.iter_mut() {
        let res = sqlx::query(
            "
		INSERT INTO chain_tunnel (tunnel_id, chain_type, node_id, port, strategy, inx, protocol, exit_node_ids)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(ct.tunnel_id)
        .bind(&ct.chain_type)
        .bind(ct.node_id)
        .bind(ct.port)
        .bind(&ct.strategy)
        .bind(ct.inx)
        .bind(&ct.protocol)
        .bind(&ct.exit_node_ids)
        .execute(&mut *tx)
        .await?;
        ct.id = res.last_insert_rowid();
    }
    tx.commit().await?;
    Ok(())
}

pub async fn delete_by_tunnel_id(pool: &SqlitePool, tunnel_id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM chain_tunnel WHERE tunnel_id = ?")
        .bind(tunnel_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_by_tunnel_and_node(
    pool: &SqlitePool,
    tunnel_id: i64,
    node_id: i64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM chain_tunnel WHERE tunnel_id = ? AND node_id = ?")
        .bind(tunnel_id)
        .bind(node_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn used_ports_on_node(pool: &SqlitePool, node_id: i64) -> AppResult<Vec<i32>> {
    let ports = sqlx::query_scalar::<_, Option<i32>>(
        "SELECT port FROM chain_tunnel WHERE node_id = ? AND port IS NOT NULL",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await?;
    Ok(ports.into_iter().flatten().collect())
}

pub fn ensure_chain_type(ct: &mut ChainTunnel, typ: i32) {
    ct.chain_type = chain_type_str(typ);
}

#[allow(clippy::too_many_arguments)]
pub fn new_chain_tunnel(
    tunnel_id: i64,
    chain_type: i32,
    node_id: i64,
    port: Option<i32>,
    strategy: Option<String>,
    protocol: Option<String>,
    inx: Option<i32>,
) -> ChainTunnel {
    ChainTunnel {
        id: 0,
        tunnel_id,
        chain_type: chain_type_str(chain_type),
        node_id,
        port,
        strategy,
        inx,
        protocol,
        brutal: false,
        exit_node_ids: None,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn new_chain_tunnel_with_exit_binding(
    tunnel_id: i64,
    chain_type: i32,
    node_id: i64,
    port: Option<i32>,
    strategy: Option<String>,
    protocol: Option<String>,
    inx: Option<i32>,
    exit_node_ids: Option<String>,
) -> ChainTunnel {
    ChainTunnel {
        id: 0,
        tunnel_id,
        chain_type: chain_type_str(chain_type),
        node_id,
        port,
        strategy,
        inx,
        protocol,
        brutal: false,
        exit_node_ids,
    }
}

pub fn int_ptr(v: i32) -> Option<i32> {
    Some(v)
}

pub fn string_ptr(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

pub fn must_int(s: &str) -> i32 {
    s.parse::<i32>().unwrap_or(0)
}

pub fn format_ids(ids: &[i64]) -> String {
    // 对齐 Go fmt.Sprintf("%v", ids) 输出：[1 2 3]
    let body = ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    format!("[{}]", body)
}
