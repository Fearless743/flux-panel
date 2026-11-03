use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};
use std::collections::HashMap;

use crate::error::{AppError, AppResult};
use crate::model;

/// 列表项（对齐 Go ForwardWithTunnel / ForwardWithTunnelDto）
#[derive(Debug, Clone, Default, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ForwardWithTunnel {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub tunnel_id: i64,
    pub remote_addr: String,
    pub status: i32,
    pub created_time: i64,
    pub updated_time: i64,
    pub user_name: String,
    pub in_flow: i64,
    pub out_flow: i64,
    pub strategy: String,
    pub inx: i32,
    pub tunnel_name: Option<String>,
    #[sqlx(rename = "type")]
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<i32>,
    // 运行时填充
    #[sqlx(default)]
    pub in_ip: String,
    #[sqlx(default)]
    pub in_port: Option<i32>,
}

/// 分页列表的服务端过滤条件（高性能分页：WHERE 下推 + LIMIT/OFFSET）。
/// user_id 为 None 表示管理员（不过滤归属）；keyword 匹配 name/remote_addr/user_name。
#[derive(Debug, Clone, Default)]
pub struct ForwardListFilter {
    pub user_id: Option<i64>,
    pub tunnel_id: Option<i64>,
    pub status: Option<i32>,
    pub keyword: String,
}

/// Keyword 中的 LIKE 通配符（% _ \）做转义，查询统一加 ESCAPE '\'。
fn push_filtered_where(qb: &mut QueryBuilder<Sqlite>, f: &ForwardListFilter) {
    qb.push("1=1");
    if let Some(uid) = f.user_id {
        qb.push(" AND f.user_id = ").push_bind(uid);
    }
    if let Some(tid) = f.tunnel_id {
        qb.push(" AND f.tunnel_id = ").push_bind(tid);
    }
    if let Some(status) = f.status {
        qb.push(" AND f.status = ").push_bind(status);
    }
    let kw = f.keyword.trim();
    if !kw.is_empty() {
        let mut esc = String::with_capacity(kw.len() + 8);
        for c in kw.chars() {
            match c {
                '\\' => esc.push_str("\\\\"),
                '%' => esc.push_str("\\%"),
                '_' => esc.push_str("\\_"),
                other => esc.push(other),
            }
        }
        let like = format!("%{}%", esc);
        qb.push(" AND (f.name LIKE ");
        qb.push_bind(like.clone());
        qb.push(" ESCAPE '\\' OR f.remote_addr LIKE ");
        qb.push_bind(like.clone());
        qb.push(" ESCAPE '\\' OR f.user_name LIKE ");
        qb.push_bind(like);
        qb.push(" ESCAPE '\\')");
    }
}

pub async fn get_by_id(pool: &SqlitePool, id: i64) -> AppResult<Option<model::Forward>> {
    Ok(sqlx::query_as::<_, model::Forward>(
        "SELECT id, user_id, user_name, name, tunnel_id, remote_addr, strategy,
		in_flow, out_flow, created_time, updated_time, status, inx FROM forward WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn list_all(pool: &SqlitePool) -> AppResult<Vec<ForwardWithTunnel>> {
    Ok(sqlx::query_as::<_, ForwardWithTunnel>(
        "SELECT f.id, f.user_id, f.name, f.tunnel_id, f.remote_addr, f.status,
			f.created_time, f.updated_time, f.user_name, f.in_flow, f.strategy,
			f.out_flow, f.inx, t.name AS tunnel_name, t.type
		FROM forward f
		LEFT JOIN tunnel t ON f.tunnel_id = t.id
		ORDER BY f.created_time DESC",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn list_by_user_id(
    pool: &SqlitePool,
    user_id: i64,
) -> AppResult<Vec<ForwardWithTunnel>> {
    Ok(sqlx::query_as::<_, ForwardWithTunnel>(
        "SELECT f.id, f.user_id, f.name, f.tunnel_id, f.remote_addr, f.status,
			f.created_time, f.updated_time, f.user_name, f.in_flow, f.strategy,
			f.out_flow, f.inx, t.name AS tunnel_name, t.type
		FROM forward f
		LEFT JOIN tunnel t ON f.tunnel_id = t.id
		WHERE f.user_id = ?
		ORDER BY f.created_time DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

/// 分页总数（与 list_filtered 同一 WHERE，保证 total 准确）。
pub async fn count_filtered(pool: &SqlitePool, f: &ForwardListFilter) -> AppResult<i64> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT COUNT(1) FROM forward f WHERE ");
    push_filtered_where(&mut qb, f);
    Ok(qb.build_query_scalar::<i64>().fetch_one(pool).await?)
}

/// 分页取一页（只查一页行 + JOIN 隧道名，inIp/inPort 由上层 fill 按页填充）。
pub async fn list_filtered(
    pool: &SqlitePool,
    f: &ForwardListFilter,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<ForwardWithTunnel>> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT f.id, f.user_id, f.name, f.tunnel_id, f.remote_addr, f.status,
			f.created_time, f.updated_time, f.user_name, f.in_flow, f.strategy,
			f.out_flow, f.inx, t.name AS tunnel_name, t.type
		FROM forward f
		LEFT JOIN tunnel t ON f.tunnel_id = t.id
		WHERE ",
    );
    push_filtered_where(&mut qb, f);
    qb.push(" ORDER BY f.inx ASC, f.created_time DESC, f.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);
    Ok(qb.build_query_as().fetch_all(pool).await?)
}

pub async fn insert(pool: &SqlitePool, f: &model::Forward) -> AppResult<i64> {
    let res = sqlx::query(
        "INSERT INTO forward (user_id, user_name, name, tunnel_id, remote_addr, strategy,
			in_flow, out_flow, created_time, updated_time, status, inx)
		VALUES (?, ?, ?, ?, ?, ?, 0, 0, ?, ?, ?, ?)",
    )
    .bind(f.user_id)
    .bind(&f.user_name)
    .bind(&f.name)
    .bind(f.tunnel_id)
    .bind(&f.remote_addr)
    .bind(&f.strategy)
    .bind(f.created_time)
    .bind(f.updated_time)
    .bind(f.status)
    .bind(f.inx)
    .execute(pool)
    .await?;
    Ok(res.last_insert_rowid())
}

pub async fn update(pool: &SqlitePool, f: &model::Forward) -> AppResult<()> {
    sqlx::query(
        "UPDATE forward SET name=?, tunnel_id=?, remote_addr=?, strategy=?,
			status=?, updated_time=?, inx=? WHERE id=?",
    )
    .bind(&f.name)
    .bind(f.tunnel_id)
    .bind(&f.remote_addr)
    .bind(&f.strategy)
    .bind(f.status)
    .bind(f.updated_time)
    .bind(f.inx)
    .bind(f.id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_status(
    pool: &SqlitePool,
    id: i64,
    status: i32,
    updated_time: i64,
) -> AppResult<()> {
    sqlx::query("UPDATE forward SET status=?, updated_time=? WHERE id=?")
        .bind(status)
        .bind(updated_time)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_inx(pool: &SqlitePool, id: i64, inx: i32) -> AppResult<()> {
    sqlx::query("UPDATE forward SET inx=? WHERE id=?")
        .bind(inx)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM forward WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// ListByIDs 批量按 ID 查询转发（批量操作预取，避免 N+1）
pub async fn list_by_ids(pool: &SqlitePool, ids: &[i64]) -> AppResult<Vec<model::Forward>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for chunk in ids.chunks(500) {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT id, user_id, user_name, name, tunnel_id, remote_addr, strategy,
			in_flow, out_flow, created_time, updated_time, status, inx FROM forward WHERE id IN (",
        );
        {
            let mut sep = qb.separated(", ");
            for id in chunk {
                sep.push_bind(*id);
            }
        }
        qb.push(")");
        let list: Vec<model::Forward> = qb.build_query_as().fetch_all(pool).await?;
        out.extend(list);
    }
    Ok(out)
}

/// ListTunnelsByIDs 批量查询隧道（用于 N+1 优化）
pub async fn list_tunnels_by_ids(
    pool: &SqlitePool,
    ids: &[i64],
) -> AppResult<Vec<model::Tunnel>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, name, traffic_ratio, type, protocol, flow,
		created_time, updated_time, status, in_ip FROM tunnel WHERE id IN (",
    );
    {
        let mut sep = qb.separated(", ");
        for id in ids {
            sep.push_bind(*id);
        }
    }
    qb.push(")");
    Ok(qb.build_query_as().fetch_all(pool).await?)
}

/// ListNodesByIDs 批量查询节点（用于 N+1 优化）
pub async fn list_nodes_by_ids(pool: &SqlitePool, ids: &[i64]) -> AppResult<Vec<model::Node>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, name, secret, server_ip, port, interface_name, version,
		http, tls, socks, created_time, updated_time, status, tcp_listen_addr, udp_listen_addr
		FROM node WHERE id IN (",
    );
    {
        let mut sep = qb.separated(", ");
        for id in ids {
            sep.push_bind(*id);
        }
    }
    qb.push(")");
    Ok(qb.build_query_as().fetch_all(pool).await?)
}

/// DeleteByIDs 批量删除转发（分片单语句 IN，替代逐条 delete）
pub async fn delete_by_ids(pool: &SqlitePool, ids: &[i64]) -> AppResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    for chunk in ids.chunks(500) {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("DELETE FROM forward WHERE id IN (");
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

/// UpdateStatusByIDs 批量更新转发状态（分片单语句 IN）
pub async fn update_status_by_ids(
    pool: &SqlitePool,
    ids: &[i64],
    status: i32,
    updated_time: i64,
) -> AppResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    for chunk in ids.chunks(500) {
        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("UPDATE forward SET status=");
        qb.push_bind(status);
        qb.push(", updated_time=");
        qb.push_bind(updated_time);
        qb.push(" WHERE id IN (");
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

/// UpdateInxBatch 单语句 CASE WHEN 批量更新排序（替代逐条 update_inx）
pub async fn update_inx_batch(
    pool: &SqlitePool,
    inx_by_id: &HashMap<i64, i32>,
) -> AppResult<()> {
    if inx_by_id.is_empty() {
        return Ok(());
    }
    let mut ids: Vec<i64> = inx_by_id.keys().copied().collect();
    ids.sort_unstable();
    // 每行占 3 个变量（WHEN/THEN/IN），分片 250 → 750 变量 < 999 上限
    for chunk in ids.chunks(250) {
        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("UPDATE forward SET inx = CASE id ");
        for id in chunk {
            qb.push("WHEN ").push_bind(*id).push(" THEN ");
            qb.push_bind(inx_by_id.get(id).copied().unwrap_or(0));
            qb.push(" ");
        }
        qb.push("END WHERE id IN (");
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

pub async fn list_by_tunnel_id(
    pool: &SqlitePool,
    tunnel_id: i64,
) -> AppResult<Vec<model::Forward>> {
    Ok(sqlx::query_as::<_, model::Forward>(
        "SELECT id, user_id, user_name, name, tunnel_id, remote_addr, strategy,
		in_flow, out_flow, created_time, updated_time, status, inx FROM forward WHERE tunnel_id=?",
    )
    .bind(tunnel_id)
    .fetch_all(pool)
    .await?)
}

pub async fn count_by_tunnel_id(pool: &SqlitePool, tunnel_id: i64) -> AppResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM forward WHERE tunnel_id=?")
        .bind(tunnel_id)
        .fetch_one(pool)
        .await?)
}

pub async fn list_by_user_and_tunnel(
    pool: &SqlitePool,
    user_id: i64,
    tunnel_id: i64,
) -> AppResult<Vec<model::Forward>> {
    Ok(sqlx::query_as::<_, model::Forward>(
        "SELECT id, user_id, user_name, name, tunnel_id, remote_addr, strategy,
		in_flow, out_flow, created_time, updated_time, status, inx FROM forward WHERE user_id=? AND tunnel_id=?",
    )
    .bind(user_id)
    .bind(tunnel_id)
    .fetch_all(pool)
    .await?)
}

pub async fn delete_by_tunnel_id(pool: &SqlitePool, tunnel_id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM forward WHERE tunnel_id=?")
        .bind(tunnel_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn count_by_user(pool: &SqlitePool, user_id: i64) -> AppResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM forward WHERE user_id=?")
        .bind(user_id)
        .fetch_one(pool)
        .await?)
}

pub async fn count_by_user_tunnel(
    pool: &SqlitePool,
    user_id: i64,
    tunnel_id: i64,
    exclude_forward_id: Option<i64>,
) -> AppResult<i64> {
    let n = if let Some(exclude) = exclude_forward_id {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(1) FROM forward WHERE user_id=? AND tunnel_id=? AND id<>?",
        )
        .bind(user_id)
        .bind(tunnel_id)
        .bind(exclude)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(1) FROM forward WHERE user_id=? AND tunnel_id=?",
        )
        .bind(user_id)
        .bind(tunnel_id)
        .fetch_one(pool)
        .await?
    };
    Ok(n)
}

pub async fn count_by_ids_and_user(
    pool: &SqlitePool,
    ids: &[i64],
    user_id: i64,
) -> AppResult<i64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT COUNT(1) FROM forward WHERE id IN (");
    {
        let mut sep = qb.separated(", ");
        for id in ids {
            sep.push_bind(*id);
        }
    }
    qb.push(") AND user_id=").push_bind(user_id);
    Ok(qb.build_query_scalar::<i64>().fetch_one(pool).await?)
}

// ---- 关联实体查询 ----

pub async fn get_tunnel(pool: &SqlitePool, id: i64) -> AppResult<Option<model::Tunnel>> {
    Ok(sqlx::query_as::<_, model::Tunnel>(
        "SELECT id, name, traffic_ratio, type, protocol, flow,
		created_time, updated_time, status, in_ip FROM tunnel WHERE id=?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn get_node(pool: &SqlitePool, id: i64) -> AppResult<Option<model::Node>> {
    Ok(sqlx::query_as::<_, model::Node>(
        "SELECT id, name, secret, server_ip, port, interface_name, version,
		http, tls, socks, created_time, updated_time, status, tcp_listen_addr, udp_listen_addr
		FROM node WHERE id=?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn get_user(pool: &SqlitePool, id: i64) -> AppResult<Option<model::User>> {
    Ok(sqlx::query_as::<_, model::User>(
        "SELECT id, user, pwd, role_id, exp_time, flow, in_flow, out_flow,
		flow_reset_time, num, created_time, updated_time, status FROM user WHERE id=?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn get_user_tunnel(
    pool: &SqlitePool,
    user_id: i64,
    tunnel_id: i64,
) -> AppResult<Option<model::UserTunnel>> {
    Ok(sqlx::query_as::<_, model::UserTunnel>(
        "SELECT id, user_id, tunnel_id, speed_id, num, flow, in_flow, out_flow,
		flow_reset_time, exp_time, status FROM user_tunnel WHERE user_id=? AND tunnel_id=?",
    )
    .bind(user_id)
    .bind(tunnel_id)
    .fetch_optional(pool)
    .await?)
}

// ---- chain_tunnel 关联查询 ----
// 注意：Go 只 select 8 列，而 model::ChainTunnel 还含 brutal/exit_node_ids，
// 这里用行结构体承接后再映射，保证 SQL 与 Go 逐字一致。

#[derive(Debug, Clone, FromRow)]
struct ChainTunnelRow {
    id: i64,
    tunnel_id: i64,
    chain_type: String,
    node_id: i64,
    port: Option<i32>,
    strategy: Option<String>,
    inx: Option<i32>,
    protocol: Option<String>,
}

impl From<ChainTunnelRow> for model::ChainTunnel {
    fn from(r: ChainTunnelRow) -> Self {
        model::ChainTunnel {
            id: r.id,
            tunnel_id: r.tunnel_id,
            chain_type: r.chain_type,
            node_id: r.node_id,
            port: r.port,
            strategy: r.strategy,
            inx: r.inx,
            protocol: r.protocol,
            brutal: false,
            exit_node_ids: None,
        }
    }
}

/// 按 tunnel_id 查询；chain_type 为空则全部，否则按 chain_type 过滤（"1"/"2"/"3"）
pub async fn list_chain_tunnels_by_tunnel(
    pool: &SqlitePool,
    tunnel_id: i64,
    chain_type: &str,
) -> AppResult<Vec<model::ChainTunnel>> {
    let rows: Vec<ChainTunnelRow> = if chain_type.is_empty() {
        sqlx::query_as::<_, ChainTunnelRow>(
            "SELECT id, tunnel_id, chain_type, node_id, port, strategy, inx, protocol
			FROM chain_tunnel WHERE tunnel_id=?",
        )
        .bind(tunnel_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, ChainTunnelRow>(
            "SELECT id, tunnel_id, chain_type, node_id, port, strategy, inx, protocol
			FROM chain_tunnel WHERE tunnel_id=? AND chain_type=?",
        )
        .bind(tunnel_id)
        .bind(chain_type)
        .fetch_all(pool)
        .await?
    };
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn list_chain_tunnels_by_node(
    pool: &SqlitePool,
    node_id: i64,
) -> AppResult<Vec<model::ChainTunnel>> {
    let rows = sqlx::query_as::<_, ChainTunnelRow>(
        "SELECT id, tunnel_id, chain_type, node_id, port, strategy, inx, protocol
		FROM chain_tunnel WHERE node_id=?",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// ListEntriesByTunnelIDs 批量查询多个隧道的入口节点（批量操作预取，避免 N+1）
pub async fn list_entries_by_tunnel_ids(
    pool: &SqlitePool,
    tunnel_ids: &[i64],
) -> AppResult<Vec<model::ChainTunnel>> {
    if tunnel_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, tunnel_id, chain_type, node_id, port, strategy, inx, protocol
		FROM chain_tunnel WHERE tunnel_id IN (",
    );
    {
        let mut sep = qb.separated(", ");
        for id in tunnel_ids {
            sep.push_bind(*id);
        }
    }
    qb.push(") AND chain_type = '1' ORDER BY id");
    let rows: Vec<ChainTunnelRow> = qb.build_query_as().fetch_all(pool).await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

// ---- user_tunnel 批量查询（Go 在 user_tunnel.go，这里用于 forward 批量优化） ----

/// ListByTunnelIDs 批量查询多个隧道下的用户权限（批量操作预取，避免 N+1）
pub async fn list_user_tunnels_by_tunnel_ids(
    pool: &SqlitePool,
    tunnel_ids: &[i64],
) -> AppResult<Vec<model::UserTunnel>> {
    if tunnel_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for chunk in tunnel_ids.chunks(500) {
        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT * FROM user_tunnel WHERE tunnel_id IN (");
        {
            let mut sep = qb.separated(", ");
            for id in chunk {
                sep.push_bind(*id);
            }
        }
        qb.push(")");
        let list: Vec<model::UserTunnel> = qb.build_query_as().fetch_all(pool).await?;
        out.extend(list);
    }
    Ok(out)
}

/// ListByUserAndTunnelIDs 批量查询某用户在多个隧道上的权限（批量操作预取）
pub async fn list_user_tunnels_by_user_and_tunnel_ids(
    pool: &SqlitePool,
    user_id: i64,
    tunnel_ids: &[i64],
) -> AppResult<Vec<model::UserTunnel>> {
    if tunnel_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for chunk in tunnel_ids.chunks(500) {
        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT * FROM user_tunnel WHERE user_id = ");
        qb.push_bind(user_id);
        qb.push(" AND tunnel_id IN (");
        {
            let mut sep = qb.separated(", ");
            for id in chunk {
                sep.push_bind(*id);
            }
        }
        qb.push(")");
        let list: Vec<model::UserTunnel> = qb.build_query_as().fetch_all(pool).await?;
        out.extend(list);
    }
    Ok(out)
}

// ---- port 解析 ----

/// 解析节点端口配置："1000-2000,3000"
pub fn parse_ports(input: &str) -> AppResult<Vec<i32>> {
    use std::collections::BTreeSet;

    let mut set: BTreeSet<i32> = BTreeSet::new();
    if input.trim().is_empty() {
        return Ok(Vec::new());
    }
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.contains('-') {
            let range_parts: Vec<&str> = part.splitn(2, '-').collect();
            if range_parts.len() != 2 {
                return Err(AppError::new(format!("invalid port range: {}", part)));
            }
            let mut start = scan_int_prefix(range_parts[0].trim())
                .ok_or_else(|| AppError::new(format!("invalid port: {}", part)))?;
            let mut end = scan_int_prefix(range_parts[1].trim())
                .ok_or_else(|| AppError::new(format!("invalid port: {}", part)))?;
            if start > end {
                std::mem::swap(&mut start, &mut end);
            }
            for i in start..=end {
                set.insert(i);
            }
        } else {
            let p = scan_int_prefix(part)
                .ok_or_else(|| AppError::new(format!("invalid port: {}", part)))?;
            set.insert(p);
        }
    }
    Ok(set.into_iter().collect())
}

/// 模拟 Go `fmt.Sscanf(s, "%d", &n)`：读取可选符号 + 十进制前缀，忽略尾部残余字符。
fn scan_int_prefix(s: &str) -> Option<i32> {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut i = 0usize;
    let mut neg = false;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        neg = bytes[i] == b'-';
        i += 1;
    }
    let start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == start {
        return None;
    }
    let mut val: i64 = 0;
    for b in &bytes[start..i] {
        val = val.checked_mul(10)?.checked_add((b - b'0') as i64)?;
        if val > i32::MAX as i64 + 1 {
            return None;
        }
    }
    if neg {
        val = -val;
    }
    i32::try_from(val).ok()
}
