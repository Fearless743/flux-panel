use std::collections::{HashMap, HashSet};

use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::crypto;
use crate::error::AppResult;
use crate::gost;
use crate::model::{self, ChainTunnel, Forward, Node, User, UserTunnel};
use crate::state::AppState;

const BYTES_TO_GB: i64 = 1024 * 1024 * 1024;
const BATCH_CONCURRENCY: usize = 16;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlowDto {
    #[serde(default)]
    pub n: String,
    #[serde(default)]
    pub u: i64,
    #[serde(default)]
    pub d: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigItem {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GostConfigDto {
    #[serde(default)]
    pub limiters: Vec<ConfigItem>,
    #[serde(default)]
    pub services: Vec<ConfigItem>,
}

#[derive(Debug, Default, Deserialize)]
struct EncryptedMessage {
    #[serde(default)]
    encrypted: bool,
    #[serde(default)]
    data: String,
    #[serde(default)]
    #[allow(dead_code)]
    timestamp: i64,
}

/// 解密节点上报 body；非加密则原样返回
pub fn decrypt_if_needed(raw: &[u8], secret: &str) -> AppResult<Vec<u8>> {
    if raw.is_empty() {
        return Ok(raw.to_vec());
    }
    let Ok(wrap) = serde_json::from_slice::<EncryptedMessage>(raw) else {
        return Ok(raw.to_vec());
    };
    if !wrap.encrypted || wrap.data.is_empty() {
        return Ok(raw.to_vec());
    }
    let Some(c) = crypto::get_or_create(secret) else {
        return Ok(raw.to_vec());
    };
    c.decrypt(&wrap.data)
}

/// 严格解密：/flow/* 必须使用加密信封，明文直接拒绝（防嗅探重放）
pub fn decrypt_strict(raw: &[u8], secret: &str) -> AppResult<Vec<u8>> {
    if raw.is_empty() {
        return Err(crate::error::AppError::new("空上报"));
    }
    // 兼容 agent 加密失败回退的明文 JSON：仅允许解密器缺失时的极小窗口，
    // 正常持有 secret 的节点一律走加密；此处要求信封存在
    let wrap: EncryptedMessage = serde_json::from_slice(raw)
        .map_err(|_| crate::error::AppError::new("上报需加密"))?;
    if !wrap.encrypted || wrap.data.is_empty() {
        return Err(crate::error::AppError::new("上报需加密"));
    }
    let Some(c) = crypto::get_or_create(secret) else {
        return Err(crate::error::AppError::new("解密失败"));
    };
    c.decrypt(&wrap.data)
}

pub async fn find_node_by_secret(state: &AppState, secret: &str) -> AppResult<Option<Node>> {
    let node = sqlx::query_as::<_, Node>("SELECT * FROM node WHERE secret = ? LIMIT 1")
        .bind(secret)
        .fetch_optional(&state.db)
        .await?;
    Ok(node)
}

pub async fn is_valid_node(state: &AppState, secret: &str) -> bool {
    let cnt = sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM node WHERE secret = ?")
        .bind(secret)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    cnt > 0
}

fn placeholders(n: usize) -> String {
    let mut s = String::with_capacity(n * 2);
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push('?');
    }
    s
}

/// Upload 处理流量上报数组：同批按解析结果聚合，转发/隧道一次查回算倍率，
/// 同一转发/用户/权限的流量合并为一次原子更新，超限检测每类只做一次。
/// reporting_node_id 用于校验节点-隧道绑定：仅入口节点可上报其隧道的流量。
pub async fn upload(state: &AppState, reporting_node_id: i64, items: Vec<FlowDto>) {
    struct Parsed {
        forward_id: i64,
        user_id: i64,
        ut_id: i64,
        d: i64,
        u: i64,
    }

    let mut parsed_items: Vec<Parsed> = Vec::with_capacity(items.len());
    let mut fwd_set: HashSet<i64> = HashSet::new();
    // 单次上报上限 + 单值上限，防恶意/异常数据刷爆配额
    const MAX_ITEMS: usize = 2000;
    const MAX_SINGLE: i64 = 100 * 1024 * 1024 * 1024; // 100GiB
    for item in items.iter().take(MAX_ITEMS) {
        if item.n.is_empty() || item.n == "web_api" {
            continue;
        }
        if item.n.len() > 128 {
            continue;
        }
        if item.d.abs() > MAX_SINGLE || item.u.abs() > MAX_SINGLE {
            continue;
        }
        if item.d < 0 || item.u < 0 {
            continue;
        }
        let Some((f, u, ut)) = parse_flow_name(&item.n) else {
            continue;
        };
        parsed_items.push(Parsed {
            forward_id: f,
            user_id: u,
            ut_id: ut,
            d: item.d,
            u: item.u,
        });
        fwd_set.insert(f);
    }
    if parsed_items.is_empty() {
        return;
    }

    // 批量查转发所属隧道与隧道倍率（查不到时倍率为 1，与逐条路径一致）
    let mut tunnel_of_forward: HashMap<i64, i64> = HashMap::new();
    if !fwd_set.is_empty() {
        let fwd_ids: Vec<i64> = fwd_set.iter().copied().collect();
        let q = format!(
            "SELECT id, tunnel_id FROM forward WHERE id IN ({})",
            placeholders(fwd_ids.len())
        );
        let mut query = sqlx::query_as::<_, (i64, i64)>(&q);
        for id in &fwd_ids {
            query = query.bind(id);
        }
        if let Ok(rows) = query.fetch_all(&state.db).await {
            for (id, tunnel_id) in rows {
                tunnel_of_forward.insert(id, tunnel_id);
            }
        }
    }
    let tun_set: HashSet<i64> = tunnel_of_forward.values().copied().collect();
    // 节点-隧道绑定：仅当上报节点是该隧道入口链节点时才接受该条流量，
    // 否则丢弃（防任一节点伪造全网流量刷爆配额）。
    let allowed_tunnels: HashSet<i64> = if tun_set.is_empty() {
        HashSet::new()
    } else {
        let tun_ids: Vec<i64> = tun_set.iter().copied().collect();
        let q = format!(
            "SELECT tunnel_id, node_id FROM chain_tunnel WHERE tunnel_id IN ({}) AND (chain_type = '1' OR chain_type = 1)",
            placeholders(tun_ids.len())
        );
        let mut query = sqlx::query_as::<_, (i64, i64)>(&q);
        for id in &tun_ids {
            query = query.bind(id);
        }
        let mut s = HashSet::new();
        if let Ok(rows) = query.fetch_all(&state.db).await {
            for (tid, nid) in rows {
                if nid == reporting_node_id {
                    s.insert(tid);
                }
            }
        }
        s
    };
    // 过滤掉上报节点无权上报的转发
    let tunnel_of_forward: HashMap<i64, i64> = tunnel_of_forward
        .into_iter()
        .filter(|(_, tid)| allowed_tunnels.contains(tid))
        .collect();
    if tunnel_of_forward.is_empty() {
        return;
    }
    let mut ratio_of_tunnel: HashMap<i64, f64> = HashMap::new();
    if !allowed_tunnels.is_empty() {
        let tun_ids: Vec<i64> = allowed_tunnels.iter().copied().collect();
        let q = format!(
            "SELECT id, traffic_ratio, flow FROM tunnel WHERE id IN ({})",
            placeholders(tun_ids.len())
        );
        let mut query = sqlx::query_as::<_, (i64, f64, i64)>(&q);
        for id in &tun_ids {
            query = query.bind(id);
        }
        if let Ok(rows) = query.fetch_all(&state.db).await {
            for (id, traffic_ratio, flow) in rows {
                let mut ratio = traffic_ratio;
                if ratio <= 0.0 {
                    ratio = 1.0;
                }
                let mut tf = flow as f64;
                if tf <= 0.0 {
                    tf = 1.0;
                }
                ratio_of_tunnel.insert(id, ratio * tf);
            }
        }
    }

    let mut fwd_agg: HashMap<i64, (i64, i64)> = HashMap::new();
    let mut user_agg: HashMap<i64, (i64, i64)> = HashMap::new();
    let mut ut_agg: HashMap<i64, (i64, i64)> = HashMap::new();
    let mut check_users: HashSet<i64> = HashSet::new();
    let mut ut_users: HashMap<i64, i64> = HashMap::new();
    for p in &parsed_items {
        // 非上报节点隧道的流量直接丢弃
        let tid = match tunnel_of_forward.get(&p.forward_id) {
            Some(t) => *t,
            None => continue,
        };
        let mut mult = 1.0;
        if let Some(m) = ratio_of_tunnel.get(&tid) {
            mult = *m;
        }
        let d = (p.d as f64 * mult) as i64;
        let u = (p.u as f64 * mult) as i64;
        add_agg(&mut fwd_agg, p.forward_id, d, u);
        add_agg(&mut user_agg, p.user_id, d, u);
        // 超限检测（管理员 userTunnelId=0 跳过，与逐条路径一致）
        if p.ut_id != 0 {
            add_agg(&mut ut_agg, p.ut_id, d, u);
            check_users.insert(p.user_id);
            ut_users.entry(p.ut_id).or_insert(p.user_id);
        }
    }

    for (id, (d, u)) in &fwd_agg {
        update_forward_flow(state, *id, *d, *u).await;
    }
    for (id, (d, u)) in &user_agg {
        update_user_flow(state, *id, *d, *u).await;
    }
    for (id, (d, u)) in &ut_agg {
        update_user_tunnel_flow(state, *id, *d, *u).await;
    }
    for uid in &check_users {
        check_user_related_limits(state, *uid).await;
    }
    for (ut_id, uid) in &ut_users {
        check_user_tunnel_related_limits(state, *ut_id, *uid).await;
    }
}

fn add_agg(m: &mut HashMap<i64, (i64, i64)>, id: i64, d: i64, u: i64) {
    let e = m.entry(id).or_insert((0, 0));
    e.0 += d;
    e.1 += u;
}

/// parseFlowName 解析 "{forwardId}_{userId}_{userTunnelId}[_tcp|_udp]" 服务名
pub fn parse_flow_name(n: &str) -> Option<(i64, i64, i64)> {
    if let Some(v) = gost::parse_forward_service_name(n) {
        return Some(v);
    }
    // 兼容 Java split("_") 前三段
    let parts: Vec<&str> = n.split('_').collect();
    if parts.len() < 3 {
        return None;
    }
    let forward_id = parts[0].parse::<i64>().ok()?;
    let user_id = parts[1].parse::<i64>().ok()?;
    let user_tunnel_id = parts[2].parse::<i64>().ok()?;
    Some((forward_id, user_id, user_tunnel_id))
}

pub async fn update_forward_flow(state: &AppState, forward_id: i64, d: i64, u: i64) {
    let _ = sqlx::query("UPDATE forward SET in_flow = in_flow + ?, out_flow = out_flow + ? WHERE id = ?")
        .bind(d)
        .bind(u)
        .bind(forward_id)
        .execute(&state.db)
        .await;
}

pub async fn update_user_flow(state: &AppState, user_id: i64, d: i64, u: i64) {
    let _ = sqlx::query("UPDATE user SET in_flow = in_flow + ?, out_flow = out_flow + ? WHERE id = ?")
        .bind(d)
        .bind(u)
        .bind(user_id)
        .execute(&state.db)
        .await;
}

pub async fn update_user_tunnel_flow(state: &AppState, user_tunnel_id: i64, d: i64, u: i64) {
    let _ = sqlx::query(
        "UPDATE user_tunnel SET in_flow = in_flow + ?, out_flow = out_flow + ? WHERE id = ?",
    )
    .bind(d)
    .bind(u)
    .bind(user_tunnel_id)
    .execute(&state.db)
    .await;
}

pub async fn check_user_related_limits(state: &AppState, user_id: i64) {
    let user = match sqlx::query_as::<_, User>("SELECT * FROM user WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
    {
        Ok(Some(u)) => u,
        _ => return,
    };
    let now = crate::response::now_ms();
    // 无限流量：跳过配额检查，仅检查到期/停用
    if !model::is_unlimited_flow(user.flow) {
        let limit = user.flow * BYTES_TO_GB;
        let current = user.in_flow + user.out_flow;
        if limit < current {
            tracing::info!(
                scope = "user",
                user = user_id,
                quotaGB = user.flow,
                used = current,
                expTime = user.exp_time,
                status = user.status,
                "流量上报自动暂停"
            );
            pause_all_user_services(state, user_id).await;
            return;
        }
    }
    if (user.exp_time > 0 && user.exp_time <= now) || user.status != 1 {
        tracing::info!(
            scope = "user",
            user = user_id,
            quotaGB = user.flow,
            used = user.in_flow + user.out_flow,
            expTime = user.exp_time,
            status = user.status,
            "流量上报自动暂停"
        );
        pause_all_user_services(state, user_id).await;
    }
}

pub async fn check_user_tunnel_related_limits(state: &AppState, user_tunnel_id: i64, user_id: i64) {
    let ut = match sqlx::query_as::<_, UserTunnel>("SELECT * FROM user_tunnel WHERE id = ?")
        .bind(user_tunnel_id)
        .fetch_optional(&state.db)
        .await
    {
        Ok(Some(ut)) => ut,
        _ => return,
    };
    let flow = ut.in_flow + ut.out_flow;
    let now = crate::response::now_ms();
    // 无限流量：跳过配额检查，仅检查到期/停用
    if !model::is_unlimited_flow(ut.flow) {
        if flow >= ut.flow * BYTES_TO_GB {
            tracing::info!(
                scope = "user_tunnel",
                ut = user_tunnel_id,
                user = user_id,
                quotaGB = ut.flow,
                used = flow,
                expTime = ut.exp_time,
                status = ut.status,
                "流量上报自动暂停"
            );
            pause_specific_forward(state, ut.tunnel_id, user_id).await;
            return;
        }
    }
    if (ut.exp_time > 0 && ut.exp_time <= now) || ut.status != 1 {
        tracing::info!(
            scope = "user_tunnel",
            ut = user_tunnel_id,
            user = user_id,
            quotaGB = ut.flow,
            used = flow,
            expTime = ut.exp_time,
            status = ut.status,
            "流量上报自动暂停"
        );
        pause_specific_forward(state, ut.tunnel_id, user_id).await;
    }
}

pub async fn pause_all_user_services(state: &AppState, user_id: i64) {
    // 只处理运行中的，已暂停的不再重复下发
    let forwards = match sqlx::query_as::<_, Forward>(
        "SELECT * FROM forward WHERE user_id = ? AND status = 1",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    {
        Ok(v) => v,
        Err(_) => return,
    };
    if !forwards.is_empty() {
        tracing::info!(
            scope = "user",
            user = user_id,
            count = forwards.len(),
            "执行自动暂停"
        );
    }
    pause_forwards(state, &forwards).await;
}

pub async fn pause_specific_forward(state: &AppState, tunnel_id: i64, user_id: i64) {
    let forwards = match sqlx::query_as::<_, Forward>(
        "SELECT * FROM forward WHERE tunnel_id = ? AND user_id = ? AND status = 1",
    )
    .bind(tunnel_id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    {
        Ok(v) => v,
        Err(_) => return,
    };
    if !forwards.is_empty() {
        tracing::info!(
            scope = "user_tunnel",
            tunnel = tunnel_id,
            user = user_id,
            count = forwards.len(),
            "执行自动暂停"
        );
    }
    pause_forwards(state, &forwards).await;
}

/// 对每条 forward 用正确 serviceName 暂停：归属权限与入口链一次查回，
/// WS 限并发并行，状态一次批量写。
pub async fn pause_forwards(state: &AppState, forwards: &[Forward]) {
    if forwards.is_empty() {
        return;
    }
    let tun_set: HashSet<i64> = forwards.iter().map(|f| f.tunnel_id).collect();
    let user_set: HashSet<i64> = forwards.iter().map(|f| f.user_id).collect();
    let tun_ids: Vec<i64> = tun_set.iter().copied().collect();
    let user_ids: Vec<i64> = user_set.iter().copied().collect();

    // 归属权限一次查回（超集 + 内存精确匹配）
    let ut_by_pair: HashMap<(i64, i64), i64> = {
        let q = format!(
            "SELECT * FROM user_tunnel WHERE tunnel_id IN ({}) AND user_id IN ({})",
            placeholders(tun_ids.len()),
            placeholders(user_ids.len())
        );
        let mut query = sqlx::query_as::<_, UserTunnel>(&q);
        for id in &tun_ids {
            query = query.bind(id);
        }
        for id in &user_ids {
            query = query.bind(id);
        }
        match query.fetch_all(&state.db).await {
            Ok(uts) => {
                let mut m = HashMap::new();
                for ut in uts {
                    if !tun_set.contains(&ut.tunnel_id) {
                        continue;
                    }
                    if !user_set.contains(&ut.user_id) {
                        continue;
                    }
                    m.insert((ut.user_id, ut.tunnel_id), ut.id);
                }
                m
            }
            Err(_) => {
                pause_forwards_legacy(state, forwards).await;
                return;
            }
        }
    };

    // 入口链一次查回
    let chains_by_tunnel: HashMap<i64, Vec<ChainTunnel>> = {
        let q = format!(
            "SELECT * FROM chain_tunnel WHERE tunnel_id IN ({}) AND (chain_type = '1' OR chain_type = 1)",
            placeholders(tun_ids.len())
        );
        let mut query = sqlx::query_as::<_, ChainTunnel>(&q);
        for id in &tun_ids {
            query = query.bind(id);
        }
        match query.fetch_all(&state.db).await {
            Ok(chains) => {
                let mut m: HashMap<i64, Vec<ChainTunnel>> = HashMap::new();
                for ct in chains {
                    m.entry(ct.tunnel_id).or_default().push(ct);
                }
                m
            }
            Err(_) => {
                pause_forwards_legacy(state, forwards).await;
                return;
            }
        }
    };

    struct PauseTask {
        base_name: String,
        node_ids: Vec<i64>,
    }
    let mut tasks: Vec<PauseTask> = Vec::with_capacity(forwards.len());
    let mut ids: Vec<i64> = Vec::with_capacity(forwards.len());
    for fw in forwards {
        let ut_id = ut_by_pair
            .get(&(fw.user_id, fw.tunnel_id))
            .copied()
            .unwrap_or(0);
        let base = gost::forward_service_base(fw.id, fw.user_id, ut_id);
        let node_ids: Vec<i64> = chains_by_tunnel
            .get(&fw.tunnel_id)
            .map(|v| v.iter().map(|ct| ct.node_id).collect())
            .unwrap_or_default();
        tasks.push(PauseTask {
            base_name: base,
            node_ids,
        });
        ids.push(fw.id);
    }

    let hub = state.hub.clone();
    stream::iter(tasks.into_iter().map(move |t| {
        let hub = hub.clone();
        async move {
            let payload = gost::pause_resume_payload(&t.base_name);
            for node_id in &t.node_ids {
                let res = hub.send_msg(*node_id, payload.clone(), "PauseService").await;
                if !gost::is_ok(&res.msg) {
                    tracing::info!(
                        node = *node_id,
                        name = %t.base_name,
                        msg = %res.msg,
                        "PauseService"
                    );
                }
            }
        }
    }))
    .buffer_unordered(BATCH_CONCURRENCY)
    .for_each(|_| async {})
    .await;

    // 状态一次批量写（分片单语句 IN；原逐条 UPDATE）
    for chunk in ids.chunks(500) {
        let q = format!(
            "UPDATE forward SET status = 0 WHERE id IN ({})",
            placeholders(chunk.len())
        );
        let mut query = sqlx::query(&q);
        for id in chunk {
            query = query.bind(id);
        }
        let _ = query.execute(&state.db).await;
    }
}

/// 逐条暂停（批量预取失败时的回落路径，与原语义一致）
pub async fn pause_forwards_legacy(state: &AppState, forwards: &[Forward]) {
    for fw in forwards {
        let service_name = service_name_for_forward(state, fw).await;
        if service_name.is_empty() {
            // 仍更新状态
            let _ = sqlx::query("UPDATE forward SET status = 0 WHERE id = ?")
                .bind(fw.id)
                .execute(&state.db)
                .await;
            continue;
        }
        pause_service_on_nodes(state, fw.tunnel_id, &service_name).await;
        let _ = sqlx::query("UPDATE forward SET status = 0 WHERE id = ?")
            .bind(fw.id)
            .execute(&state.db)
            .await;
    }
}

async fn service_name_for_forward(state: &AppState, fw: &Forward) -> String {
    match sqlx::query_as::<_, UserTunnel>(
        "SELECT * FROM user_tunnel WHERE user_id = ? AND tunnel_id = ? LIMIT 1",
    )
    .bind(fw.user_id)
    .bind(fw.tunnel_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(ut)) => gost::forward_service_base(fw.id, fw.user_id, ut.id),
        // 管理员可能无 user_tunnel，用 0
        _ => gost::forward_service_base(fw.id, fw.user_id, 0),
    }
}

async fn pause_service_on_nodes(state: &AppState, tunnel_id: i64, base_name: &str) {
    // chain_type 可能是 "1" 或 1
    let chains = match sqlx::query_as::<_, ChainTunnel>(
        "SELECT * FROM chain_tunnel WHERE tunnel_id = ? AND (chain_type = '1' OR chain_type = 1)",
    )
    .bind(tunnel_id)
    .fetch_all(&state.db)
    .await
    {
        Ok(v) => v,
        Err(_) => return,
    };
    let payload = gost::pause_resume_payload(base_name);
    for ct in chains {
        let res = state
            .hub
            .send_msg(ct.node_id, payload.clone(), "PauseService")
            .await;
        if !gost::is_ok(&res.msg) {
            tracing::info!(
                node = ct.node_id,
                name = %base_name,
                msg = %res.msg,
                "PauseService"
            );
        }
    }
}

/// 异步清理节点孤立配置
pub async fn clean_node_configs(state: &AppState, node_id: i64, cfg: GostConfigDto) {
    // 上报条目上限，防超大配置拖慢 DB
    const MAX_CFG_ITEMS: usize = 5000;
    let services: Vec<ConfigItem> = cfg.services.into_iter().take(MAX_CFG_ITEMS).collect();
    let limiters: Vec<ConfigItem> = cfg.limiters.into_iter().take(MAX_CFG_ITEMS).collect();
    let node = match sqlx::query_as::<_, Node>("SELECT * FROM node WHERE id = ?")
        .bind(node_id)
        .fetch_optional(&state.db)
        .await
    {
        Ok(Some(n)) => n,
        _ => return,
    };
    clean_orphaned_services(state, &services, &node).await;
    clean_orphaned_limiters(state, &limiters, &node).await;
}

async fn clean_orphaned_services(state: &AppState, items: &[ConfigItem], node: &Node) {
    for svc in items {
        let name = &svc.name;
        if name.is_empty() || name == "web_api" {
            continue;
        }
        let parts: Vec<&str> = name.split('_').collect();
        if parts.len() < 2 {
            continue;
        }
        let last = parts[parts.len() - 1];
        match last {
            "tls" => {
                // {tunnelId}_tls
                let Ok(tunnel_id) = parts[0].parse::<i64>() else {
                    continue;
                };
                let cnt = sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM tunnel WHERE id = ?")
                    .bind(tunnel_id)
                    .fetch_one(&state.db)
                    .await
                    .unwrap_or(0);
                if cnt == 0 {
                    delete_services(state, node.id, vec![gost::tunnel_tls(tunnel_id)]).await;
                    tracing::info!(name = %name, node = node.id, "删除孤立的服务");
                }
            }
            "tcp" => {
                // {forwardId}_{userId}_{userTunnelId}_tcp
                if parts.len() < 4 {
                    continue;
                }
                let Ok(forward_id) = parts[0].parse::<i64>() else {
                    continue;
                };
                let cnt = sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM forward WHERE id = ?")
                    .bind(forward_id)
                    .fetch_one(&state.db)
                    .await
                    .unwrap_or(0);
                if cnt == 0 {
                    let base = name.strip_suffix("_tcp").unwrap_or(name);
                    delete_services(
                        state,
                        node.id,
                        vec![format!("{}_tcp", base), format!("{}_udp", base)],
                    )
                    .await;
                    tracing::info!(name = %name, node = node.id, "删除孤立的服务");
                }
            }
            _ => {}
        }
    }
}

async fn clean_orphaned_limiters(state: &AppState, items: &[ConfigItem], node: &Node) {
    for lim in items {
        let Ok(id) = lim.name.parse::<i64>() else {
            continue;
        };
        let cnt = sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM speed_limit WHERE id = ?")
            .bind(id)
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);
        if cnt == 0 {
            delete_limiters(state, node.id, id).await;
            tracing::info!(name = %lim.name, node = node.id, "删除孤立的限流器");
        }
    }
}

async fn delete_services(state: &AppState, node_id: i64, names: Vec<String>) {
    if names.is_empty() {
        return;
    }
    // 孤儿清理无需等待节点确认
    state
        .hub
        .send_no_wait(node_id, gost::delete_service_payload(names), "DeleteService")
        .await;
}

async fn delete_limiters(state: &AppState, node_id: i64, name: i64) {
    state
        .hub
        .send_no_wait(
            node_id,
            json!({ "limiter": name.to_string() }),
            "DeleteLimiters",
        )
        .await;
}

/// 供定时任务调用：按用户/隧道到期暂停，使用正确 serviceName
pub async fn pause_forward_by_ids(state: &AppState, forwards: &[Forward]) {
    pause_forwards(state, forwards).await;
}
