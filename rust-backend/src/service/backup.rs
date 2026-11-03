use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::model::{
    ChainTunnel, Forward, ForwardPort, Node, SpeedLimit, StatisticsFlow, Tunnel, User, UserTunnel,
    ViteConfig,
};
use crate::response::now_ms;
use crate::state::AppState;

/// 全量数据备份载荷，网页端导入导出使用。
/// 包含全部业务表（含用户密码哈希，可完整迁移恢复）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupData {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub exported_at: i64,
    #[serde(default)]
    pub users: Vec<User>,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub tunnels: Vec<Tunnel>,
    #[serde(default)]
    pub chain_tunnels: Vec<ChainTunnel>,
    #[serde(default)]
    pub forwards: Vec<Forward>,
    #[serde(default)]
    pub forward_ports: Vec<ForwardPort>,
    #[serde(default)]
    pub user_tunnels: Vec<UserTunnel>,
    #[serde(default)]
    pub speed_limits: Vec<SpeedLimit>,
    #[serde(default)]
    pub vite_configs: Vec<ViteConfig>,
    #[serde(default)]
    pub statistics_flows: Vec<StatisticsFlow>,
}

/// 导出全部数据（只读，无锁表，单连接串行下安全）
pub async fn export(state: &AppState) -> AppResult<BackupData> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM user ORDER BY id")
        .fetch_all(&state.db)
        .await?;
    let nodes = sqlx::query_as::<_, Node>("SELECT * FROM node ORDER BY id")
        .fetch_all(&state.db)
        .await?;
    let tunnels = sqlx::query_as::<_, Tunnel>("SELECT * FROM tunnel ORDER BY id")
        .fetch_all(&state.db)
        .await?;
    let chain_tunnels = sqlx::query_as::<_, ChainTunnel>("SELECT * FROM chain_tunnel ORDER BY id")
        .fetch_all(&state.db)
        .await?;
    let forwards = sqlx::query_as::<_, Forward>("SELECT * FROM forward ORDER BY id")
        .fetch_all(&state.db)
        .await?;
    let forward_ports = sqlx::query_as::<_, ForwardPort>("SELECT * FROM forward_port ORDER BY id")
        .fetch_all(&state.db)
        .await?;
    let user_tunnels = sqlx::query_as::<_, UserTunnel>("SELECT * FROM user_tunnel ORDER BY id")
        .fetch_all(&state.db)
        .await?;
    let speed_limits = sqlx::query_as::<_, SpeedLimit>("SELECT * FROM speed_limit ORDER BY id")
        .fetch_all(&state.db)
        .await?;
    let vite_configs = sqlx::query_as::<_, ViteConfig>("SELECT * FROM vite_config ORDER BY id")
        .fetch_all(&state.db)
        .await?;
    let statistics_flows =
        sqlx::query_as::<_, StatisticsFlow>("SELECT * FROM statistics_flow ORDER BY id")
            .fetch_all(&state.db)
            .await?;
    Ok(BackupData {
        version: "1".to_string(),
        exported_at: now_ms(),
        users,
        nodes,
        tunnels,
        chain_tunnels,
        forwards,
        forward_ports,
        user_tunnels,
        speed_limits,
        vite_configs,
        statistics_flows,
    })
}

/// 全量覆盖式恢复：先清空全部业务表，再按 ID 原样插入，保证外键关联（forward→user/tunnel、forward_port→forward/node）不变。
/// 调用方必须已做管理员鉴权与前端二次确认。
pub async fn import(state: &AppState, data: &BackupData) -> AppResult<()> {
    if data.version != "" && data.version != "1" && data.version != "1.0" {
        return Err(AppError::new("不支持的备份版本"));
    }
    // 必须包含至少一个管理员，防止恢复后无人可登录
    let has_admin = data.users.iter().any(|u| u.role_id == 0);
    if !has_admin {
        return Err(AppError::new(
            "备份文件中缺少管理员账号，为防止面板无法登录已拒绝导入",
        ));
    }

    let mut tx = state.db.begin().await?;

    // 清空（子表先清，避免外键约束问题；SQLite 默认无 FK，但保持顺序）
    for stmt in [
        "DELETE FROM forward_port",
        "DELETE FROM forward",
        "DELETE FROM user_tunnel",
        "DELETE FROM statistics_flow",
        "DELETE FROM chain_tunnel",
        "DELETE FROM speed_limit",
        "DELETE FROM tunnel",
        "DELETE FROM node",
        "DELETE FROM user",
        "DELETE FROM vite_config",
    ] {
        sqlx::query(stmt).execute(&mut *tx).await?;
    }

    // 插入（显式 ID，保证关联关系不变；仅写 schema.sql 真实存在的列）
    for u in &data.users {
        sqlx::query(
            "INSERT INTO user (id, user, pwd, role_id, exp_time, flow, in_flow, out_flow, flow_reset_time, num, created_time, updated_time, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(u.id)
        .bind(&u.user)
        .bind(&u.pwd)
        .bind(u.role_id)
        .bind(u.exp_time)
        .bind(u.flow)
        .bind(u.in_flow)
        .bind(u.out_flow)
        .bind(u.flow_reset_time)
        .bind(u.num)
        .bind(u.created_time)
        .bind(u.updated_time)
        .bind(u.status)
        .execute(&mut *tx)
        .await?;
    }
    for n in &data.nodes {
        sqlx::query(
            "INSERT INTO node (id, name, secret, server_ip, port, interface_name, version, http, tls, socks, created_time, updated_time, status, tcp_listen_addr, udp_listen_addr)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(n.id)
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
        .bind(&n.tcp_listen_addr)
        .bind(&n.udp_listen_addr)
        .execute(&mut *tx)
        .await?;
    }
    for t in &data.tunnels {
        sqlx::query(
            "INSERT INTO tunnel (id, name, traffic_ratio, type, protocol, flow, created_time, updated_time, status, in_ip)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(t.id)
        .bind(&t.name)
        .bind(t.traffic_ratio)
        .bind(t.r#type)
        .bind(&t.protocol)
        .bind(t.flow)
        .bind(t.created_time)
        .bind(t.updated_time)
        .bind(t.status)
        .bind(&t.in_ip)
        .execute(&mut *tx)
        .await?;
    }
    for ct in &data.chain_tunnels {
        sqlx::query(
            "INSERT INTO chain_tunnel (id, tunnel_id, chain_type, node_id, port, strategy, inx, protocol, exit_node_ids)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(ct.id)
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
    }
    for f in &data.forwards {
        sqlx::query(
            "INSERT INTO forward (id, user_id, user_name, name, tunnel_id, remote_addr, strategy, in_flow, out_flow, created_time, updated_time, status, inx)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(f.id)
        .bind(f.user_id)
        .bind(&f.user_name)
        .bind(&f.name)
        .bind(f.tunnel_id)
        .bind(&f.remote_addr)
        .bind(&f.strategy)
        .bind(f.in_flow)
        .bind(f.out_flow)
        .bind(f.created_time)
        .bind(f.updated_time)
        .bind(f.status)
        .bind(f.inx)
        .execute(&mut *tx)
        .await?;
    }
    for fp in &data.forward_ports {
        sqlx::query("INSERT INTO forward_port (id, forward_id, node_id, port) VALUES (?, ?, ?, ?)")
            .bind(fp.id)
            .bind(fp.forward_id)
            .bind(fp.node_id)
            .bind(fp.port)
            .execute(&mut *tx)
            .await?;
    }
    for ut in &data.user_tunnels {
        sqlx::query(
            "INSERT INTO user_tunnel (id, user_id, tunnel_id, speed_id, num, flow, in_flow, out_flow, flow_reset_time, exp_time, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(ut.id)
        .bind(ut.user_id)
        .bind(ut.tunnel_id)
        .bind(ut.speed_id)
        .bind(ut.num)
        .bind(ut.flow)
        .bind(ut.in_flow)
        .bind(ut.out_flow)
        .bind(ut.flow_reset_time)
        .bind(ut.exp_time)
        .bind(ut.status)
        .execute(&mut *tx)
        .await?;
    }
    for sl in &data.speed_limits {
        sqlx::query(
            "INSERT INTO speed_limit (id, name, speed, tunnel_id, tunnel_name, created_time, updated_time, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(sl.id)
        .bind(&sl.name)
        .bind(sl.speed)
        .bind(sl.tunnel_id)
        .bind(&sl.tunnel_name)
        .bind(sl.created_time)
        .bind(sl.updated_time)
        .bind(sl.status)
        .execute(&mut *tx)
        .await?;
    }
    for vc in &data.vite_configs {
        sqlx::query("INSERT INTO vite_config (id, name, value, time) VALUES (?, ?, ?, ?)")
            .bind(vc.id)
            .bind(&vc.name)
            .bind(&vc.value)
            .bind(vc.time)
            .execute(&mut *tx)
            .await?;
    }
    for sf in &data.statistics_flows {
        sqlx::query(
            "INSERT INTO statistics_flow (id, user_id, flow, total_flow, time, created_time) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(sf.id)
        .bind(sf.user_id)
        .bind(sf.flow)
        .bind(sf.total_flow)
        .bind(&sf.time)
        .bind(sf.created_time)
        .execute(&mut *tx)
        .await?;
    }

    // 修正自增序列，避免后续插入 ID 冲突
    for tbl in [
        "user",
        "node",
        "tunnel",
        "chain_tunnel",
        "forward",
        "forward_port",
        "user_tunnel",
        "speed_limit",
        "vite_config",
        "statistics_flow",
    ] {
        let stmt = format!(
            "UPDATE sqlite_sequence SET seq = (SELECT COALESCE(MAX(id),0) FROM {}) WHERE name = ?",
            tbl
        );
        let _ = sqlx::query(&stmt).bind(tbl).execute(&mut *tx).await;
    }

    tx.commit().await?;
    Ok(())
}
