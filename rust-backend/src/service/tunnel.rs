use std::collections::{HashMap, HashSet};

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::error::{AppError, AppResult};
use crate::gost;
use crate::model::{ChainTunnel, Forward, ForwardPort, Node, Tunnel, UserTunnel};
use crate::repo::chain_tunnel as chain_repo;
use crate::repo::tunnel as tunnel_repo;
use crate::repo::user_tunnel as user_tunnel_repo;
use crate::response::now_ms;
use crate::service::forward::{self, DiagnosisResult};
use crate::service::tunnel_detach::delete_service;
use crate::state::AppState;

// ---- DTO ----

/// 前端传入的拓扑节点（不信任 id/port）
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ChainTunnelInput {
    #[serde(default, rename = "nodeId")]
    pub node_id: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TunnelCreateReq {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub r#type: i32,
    #[serde(default)]
    pub flow: i64,
    #[serde(default, rename = "trafficRatio")]
    pub traffic_ratio: f64,
    #[serde(default, rename = "inIp")]
    pub in_ip: String,
    #[serde(default, rename = "inNodeId")]
    pub in_node_id: Vec<ChainTunnelInput>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TunnelUpdateReq {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub flow: i64,
    #[serde(default, rename = "trafficRatio")]
    pub traffic_ratio: f64,
    #[serde(default, rename = "inIp")]
    pub in_ip: String,
    #[serde(default, rename = "inNodeId")]
    pub in_node_id: Vec<ChainTunnelInput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelDetailDto {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: i32,
    pub flow: i64,
    #[serde(serialize_with = "crate::model::serialize_f64_go")]
    pub traffic_ratio: f64,
    pub status: i32,
    pub created_time: i64,
    pub updated_time: i64,
    pub in_ip: Option<String>,
    pub in_node_id: Vec<ChainTunnel>,
}

// ---- create ----

pub async fn create(state: &AppState, req: TunnelCreateReq) -> AppResult<()> {
    if req.name.trim().is_empty() {
        return Err(AppError::new("隧道名称不能为空"));
    }
    if req.in_node_id.is_empty() {
        return Err(AppError::new("入口节点不能为空"));
    }
    if req.r#type != 1 {
        return Err(AppError::new("隧道类型不合法"));
    }
    if req.traffic_ratio <= 0.0 || req.traffic_ratio > 100.0 {
        return Err(AppError::new("流量倍率必须大于0.0且不超过100.0"));
    }

    let cnt = tunnel_repo::count_by_name(&state.db, &req.name).await?;
    if cnt > 0 {
        return Err(AppError::new("隧道名称重复"));
    }

    let mut nodes: HashMap<i64, Node> = HashMap::new();
    let mut chain_tunnels: Vec<ChainTunnel> = Vec::new();
    let mut node_ids: Vec<i64> = Vec::new();

    // 相关节点一次查回（原逐个 GetByID + 后续重复 ListByIDs，N+1）
    for input in &req.in_node_id {
        if input.node_id == 0 {
            return Err(AppError::new("节点不存在"));
        }
        node_ids.push(input.node_id);
    }

    let mut set: HashSet<i64> = HashSet::new();
    for id in &node_ids {
        set.insert(*id);
    }
    let uniq_node_ids: Vec<i64> = set.iter().copied().collect();
    let batch_nodes = load_nodes_by_ids(&state.db, &uniq_node_ids).await?;
    if batch_nodes.len() != set.len() {
        return Err(AppError::new("节点不存在"));
    }
    if set.len() != node_ids.len() {
        return Err(AppError::new("节点重复"));
    }
    for n in batch_nodes {
        nodes.insert(n.id, n);
    }
    // 全部在线
    for id in &set {
        let Some(n) = nodes.get(id) else {
            return Err(AppError::new("部分节点不存在"));
        };
        if n.status != 1 {
            return Err(AppError::new("部分节点不在线"));
        }
    }

    // 入口
    for input in &req.in_node_id {
        if !nodes.contains_key(&input.node_id) {
            return Err(AppError::new("节点不存在"));
        }
        chain_tunnels.push(chain_repo::new_chain_tunnel(
            0,
            1,
            input.node_id,
            None,
            None,
            None,
            None,
        ));
    }

    // in_ip
    let mut in_ip = req.in_ip.trim().to_string();
    if in_ip.is_empty() {
        let mut parts: Vec<String> = Vec::new();
        for input in &req.in_node_id {
            if let Some(n) = nodes.get(&input.node_id) {
                if !n.server_ip.is_empty() {
                    parts.push(n.server_ip.clone());
                }
            }
        }
        in_ip = parts.join(",");
    }

    let now = now_ms();
    let mut tunnel = Tunnel {
        name: req.name.clone(),
        traffic_ratio: req.traffic_ratio,
        r#type: 1,
        protocol: "tls".to_string(),
        flow: req.flow,
        created_time: now,
        updated_time: now,
        status: 1,
        in_ip: Some(in_ip),
        ..Default::default()
    };
    tunnel_repo::create(&state.db, &mut tunnel).await?;

    for ct in chain_tunnels.iter_mut() {
        ct.tunnel_id = tunnel.id;
    }

    if let Err(e) = chain_repo::insert_batch(&state.db, &mut chain_tunnels).await {
        let _ = tunnel_repo::delete(&state.db, tunnel.id).await;
        return Err(e);
    }

    Ok(())
}

// ---- list ----

pub async fn list(state: &AppState) -> AppResult<Vec<TunnelDetailDto>> {
    let tunnels = tunnel_repo::list_all(&state.db).await?;
    if tunnels.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<i64> = tunnels.iter().map(|t| t.id).collect();
    let all_ct = chain_repo::list_by_tunnel_ids(&state.db, &ids).await?;
    let mut by_tunnel: HashMap<i64, Vec<ChainTunnel>> = HashMap::new();
    for ct in all_ct {
        by_tunnel.entry(ct.tunnel_id).or_default().push(ct);
    }

    let mut out: Vec<TunnelDetailDto> = Vec::with_capacity(tunnels.len());
    for t in tunnels {
        let cts = by_tunnel.remove(&t.id).unwrap_or_default();
        let entries: Vec<ChainTunnel> = cts
            .into_iter()
            .filter(|ct| chain_repo::chain_type_of(ct) == 1)
            .collect();
        out.push(TunnelDetailDto {
            id: t.id,
            name: t.name,
            r#type: t.r#type,
            flow: t.flow,
            traffic_ratio: t.traffic_ratio,
            status: t.status,
            created_time: t.created_time,
            updated_time: t.updated_time,
            in_ip: t.in_ip,
            in_node_id: entries,
        });
    }
    Ok(out)
}

// ---- update ----

pub async fn update(state: &AppState, req: TunnelUpdateReq) -> AppResult<()> {
    if req.id == 0 {
        return Err(AppError::new("隧道ID不能为空"));
    }
    if req.name.trim().is_empty() {
        return Err(AppError::new("隧道名称不能为空"));
    }
    let Some(existing) = tunnel_repo::find_by_id(&state.db, req.id).await? else {
        return Err(AppError::new("隧道不存在"));
    };

    if !req.in_node_id.is_empty() {
        reconfigure_tunnel_nodes(state, &existing, &req).await?;
    }

    let mut in_ip = req.in_ip.trim().to_string();
    if in_ip.is_empty() {
        let entries = chain_repo::list_by_tunnel_id(&state.db, req.id).await?;
        let entry_node_ids: Vec<i64> = entries
            .iter()
            .filter(|ct| chain_repo::chain_type_of(ct) == 1)
            .map(|ct| ct.node_id)
            .collect();
        let entry_nodes = load_nodes_by_ids(&state.db, &entry_node_ids).await?;
        let entry_node_map: HashMap<i64, Node> =
            entry_nodes.into_iter().map(|n| (n.id, n)).collect();
        let mut parts: Vec<String> = Vec::new();
        for ct in &entries {
            if chain_repo::chain_type_of(ct) != 1 {
                continue;
            }
            let Some(n) = entry_node_map.get(&ct.node_id) else {
                return Err(AppError::new("隧道节点数据错误，部分节点不存在"));
            };
            parts.push(n.server_ip.clone());
        }
        in_ip = parts.join(",");
    }
    let now = now_ms();
    tunnel_repo::update_basic(
        &state.db,
        req.id,
        &req.name,
        req.flow,
        req.traffic_ratio,
        Some(&in_ip),
        now,
    )
    .await
}

async fn reconfigure_tunnel_nodes(
    state: &AppState,
    tunnel: &Tunnel,
    dto: &TunnelUpdateReq,
) -> AppResult<()> {
    let tunnel_id = tunnel.id;

    // 规范化
    let mut new_entries: Vec<ChainTunnel> = Vec::new();
    let mut node_ids: Vec<i64> = Vec::new();

    for input in &dto.in_node_id {
        if input.node_id == 0 {
            return Err(AppError::new("节点不存在"));
        }
        new_entries.push(chain_repo::new_chain_tunnel(
            tunnel_id,
            1,
            input.node_id,
            None,
            None,
            None,
            None,
        ));
        node_ids.push(input.node_id);
    }

    let mut set: HashSet<i64> = HashSet::new();
    for id in &node_ids {
        if !set.insert(*id) {
            return Err(AppError::new("节点重复"));
        }
    }

    let mut nodes: HashMap<i64, Node> = HashMap::new();
    let uniq_node_ids: Vec<i64> = set.iter().copied().collect();
    let node_list = load_nodes_by_ids(&state.db, &uniq_node_ids).await?;
    if node_list.len() != set.len() {
        return Err(AppError::new("部分节点不存在"));
    }
    for n in node_list {
        if n.status != 1 {
            return Err(AppError::new(format!(
                "节点 {} 不在线，无法下发配置",
                n.name
            )));
        }
        nodes.insert(n.id, n);
    }

    let old_records = chain_repo::list_by_tunnel_id(&state.db, tunnel_id).await?;

    let mut failures: Vec<String> = Vec::new();

    // DB 替换
    chain_repo::delete_by_tunnel_id(&state.db, tunnel_id).await?;
    let mut all_new: Vec<ChainTunnel> = Vec::new();
    all_new.extend(new_entries.iter().cloned());
    chain_repo::insert_batch(&state.db, &mut all_new).await?;

    // 入口变化 → 迁移转发服务
    let mut old_entry_ids: HashSet<i64> = HashSet::new();
    for old in &old_records {
        if chain_repo::chain_type_of(old) == 1 {
            old_entry_ids.insert(old.node_id);
        }
    }
    let mut new_entry_ids: HashSet<i64> = HashSet::new();
    for e in &new_entries {
        new_entry_ids.insert(e.node_id);
    }
    migrate_forward_services(
        state,
        tunnel,
        &old_entry_ids,
        &new_entry_ids,
        &nodes,
        &mut failures,
    )
    .await;

    if !failures.is_empty() {
        return Err(AppError::new(format!(
            "配置已保存，但部分下发失败: {}",
            failures.join("；")
        )));
    }
    Ok(())
}

async fn migrate_forward_services(
    state: &AppState,
    tunnel: &Tunnel,
    old_entry_ids: &HashSet<i64>,
    new_entry_ids: &HashSet<i64>,
    nodes: &HashMap<i64, Node>,
    failures: &mut Vec<String>,
) {
    let mut removed: Vec<i64> = old_entry_ids
        .difference(new_entry_ids)
        .copied()
        .collect();
    let mut added: Vec<i64> = new_entry_ids
        .difference(old_entry_ids)
        .copied()
        .collect();
    if removed.is_empty() && added.is_empty() {
        return;
    }
    removed.sort_unstable();
    added.sort_unstable();

    let forwards = match list_forwards_by_tunnel_id(&state.db, tunnel.id).await {
        Ok(v) => v,
        Err(e) => {
            failures.push(format!("查询转发失败: {}", e.0));
            return;
        }
    };
    if forwards.is_empty() {
        return;
    }

    // 归属权限、已有端口一次查回（原逐转发各 1 次查询）
    let fwd_ids: Vec<i64> = forwards.iter().map(|f| f.id).collect();
    let uts = user_tunnel_repo::list_by_tunnel_ids(&state.db, &[tunnel.id])
        .await
        .unwrap_or_default();
    let ut_by_user: HashMap<i64, UserTunnel> = uts.into_iter().map(|u| (u.user_id, u)).collect();
    let ports_map = list_forward_ports_by_forward_ids(&state.db, &fwd_ids)
        .await
        .unwrap_or_default();

    // 新增节点可用端口一次算好（原逐转发 GetAvailablePorts 各 3 次查询）
    let (avail_by_node, avail_err) = batch_available_ports(state, &added).await;

    // 本次已分配端口（串行分配时扣减，避免同批多条转发选到同一端口）
    let mut consumed: HashMap<i64, HashSet<i32>> = HashMap::new();

    // 移除：按节点批量删库，WS 删除并行尽力清理
    if !removed.is_empty() {
        for fw in &forwards {
            let ut_id = ut_by_user.get(&fw.user_id).map(|u| u.id).unwrap_or(0);
            let base = gost::forward_service_base(fw.id, fw.user_id, ut_id);
            for node_id in &removed {
                delete_service(
                    state,
                    *node_id,
                    vec![gost::forward_tcp(&base), gost::forward_udp(&base)],
                )
                .await;
            }
        }
        for node_id in &removed {
            let _ = delete_forward_ports_by_node_and_forward_ids(&state.db, *node_id, &fwd_ids).await;
        }
    }

    struct AddTask {
        fw_index: usize,
        base: String,
        limiter: Option<i64>,
        fp: ForwardPort,
        node: Node,
        paused: bool,
    }
    let mut add_tasks: Vec<AddTask> = Vec::new();

    for (fi, fw) in forwards.iter().enumerate() {
        let ut = ut_by_user.get(&fw.user_id);
        let (ut_id, limiter) = match ut {
            Some(u) => (u.id, u.speed_id),
            None => (0, None),
        };
        let base = gost::forward_service_base(fw.id, fw.user_id, ut_id);

        // preferred port
        let mut preferred_port: Option<i32> = None;
        if let Some(existing_ports) = ports_map.get(&fw.id) {
            for fp in existing_ports {
                let p = fp.port;
                if preferred_port.is_none() {
                    preferred_port = Some(p);
                }
                if !removed.contains(&fp.node_id) {
                    preferred_port = Some(p);
                    break;
                }
            }
        }

        for node_id in &added {
            let n = match nodes.get(node_id) {
                Some(n) => n.clone(),
                None => match load_node_by_id(&state.db, *node_id).await {
                    Ok(Some(n)) => n,
                    _ => {
                        failures.push(format!("转发[{}]节点不存在", fw.name));
                        continue;
                    }
                },
            };
            if let Some(msg) = avail_err.get(node_id) {
                failures.push(format!(
                    "转发[{}]在节点[{}]分配端口失败: {}",
                    fw.name, n.name, msg
                ));
                continue;
            }
            let mut available = avail_by_node.get(node_id).cloned().unwrap_or_default();
            if let Some(used) = consumed.get(node_id) {
                available.retain(|p| !used.contains(p));
            }
            if available.is_empty() {
                failures.push(format!(
                    "转发[{}]在节点[{}]分配端口失败: 节点端口已满，无可用端口",
                    fw.name, n.name
                ));
                continue;
            }
            let mut port = available[0];
            if let Some(pref) = preferred_port {
                if available.contains(&pref) {
                    port = pref;
                }
            }
            let mut fp = ForwardPort {
                id: 0,
                forward_id: fw.id,
                node_id: *node_id,
                port,
            };
            if let Err(e) = insert_forward_port(&state.db, &mut fp).await {
                failures.push(format!(
                    "转发[{}]在节点[{}]保存端口失败: {}",
                    fw.name, n.name, e.0
                ));
                continue;
            }
            consumed.entry(*node_id).or_default().insert(port);
            add_tasks.push(AddTask {
                fw_index: fi,
                base: base.clone(),
                limiter,
                fp,
                node: n,
                paused: fw.status == 0,
            });
        }
    }

    // 新增服务下发限并发并行（DB 已串行落库，端口无竞争）
    let mut add_fails: Vec<String> = Vec::new();
    for t in &add_tasks {
        let fw = &forwards[t.fw_index];
        let msg = forward::add_or_update_service(
            state,
            &t.base,
            t.limiter,
            &t.node,
            fw,
            &t.fp,
            tunnel,
            "AddService",
        )
        .await;
        if !gost::is_ok(&msg) {
            add_fails.push(format!(
                "转发[{}]在节点[{}]创建失败: {}",
                fw.name,
                t.node.name,
                gost::normalize_ok(&msg)
            ));
            continue;
        }
        if t.paused {
            let _ = state
                .hub
                .send_msg(
                    t.fp.node_id,
                    gost::pause_resume_payload(&t.base),
                    "PauseService",
                )
                .await;
        }
    }
    failures.extend(add_fails);
}

/// 一次算出多个节点的可用端口（对齐 GetAvailablePorts 逻辑）
/// 返回可用端口映射与计算失败的节点错误文案
async fn batch_available_ports(
    state: &AppState,
    node_ids: &[i64],
) -> (HashMap<i64, Vec<i32>>, HashMap<i64, String>) {
    let mut out: HashMap<i64, Vec<i32>> = HashMap::new();
    let mut failed: HashMap<i64, String> = HashMap::new();
    if node_ids.is_empty() {
        return (out, failed);
    }
    let nodes = match load_nodes_by_ids(&state.db, node_ids).await {
        Ok(v) => v,
        Err(e) => {
            for id in node_ids {
                failed.insert(*id, e.0.clone());
            }
            return (out, failed);
        }
    };
    let node_map: HashMap<i64, Node> = nodes.into_iter().map(|n| (n.id, n)).collect();
    let cts = chain_repo::list_by_node_ids(&state.db, node_ids)
        .await
        .unwrap_or_default();
    let mut used_by_chain: HashMap<i64, HashSet<i32>> = HashMap::new();
    for ct in cts {
        if let Some(p) = ct.port {
            used_by_chain.entry(ct.node_id).or_default().insert(p);
        }
    }
    let fps = list_forward_ports_by_node_ids(&state.db, node_ids)
        .await
        .unwrap_or_default();
    let mut used_by_forward: HashMap<i64, HashSet<i32>> = HashMap::new();
    for fp in fps {
        used_by_forward.entry(fp.node_id).or_default().insert(fp.port);
    }
    for id in node_ids {
        let Some(n) = node_map.get(id) else {
            failed.insert(*id, "节点不存在".to_string());
            continue;
        };
        let parsed = match parse_ports(&n.port) {
            Ok(v) => v,
            Err(e) => {
                failed.insert(*id, e.0);
                continue;
            }
        };
        let mut avail: Vec<i32> = Vec::new();
        for p in parsed {
            if used_by_chain.get(id).map(|s| s.contains(&p)).unwrap_or(false) {
                continue;
            }
            if used_by_forward
                .get(id)
                .map(|s| s.contains(&p))
                .unwrap_or(false)
            {
                continue;
            }
            avail.push(p);
        }
        out.insert(*id, avail);
    }
    (out, failed)
}

// ---- delete ----

pub async fn delete(state: &AppState, id: i64) -> AppResult<()> {
    if id == 0 {
        return Err(AppError::new("隧道ID不能为空"));
    }
    let Some(_tunnel) = tunnel_repo::find_by_id(&state.db, id).await? else {
        return Err(AppError::new("隧道不存在"));
    };

    // 禁止级联：有转发或用户权限时拒绝删除，需先手动清理
    let fw_count = count_forwards_by_tunnel_id(&state.db, id).await?;
    let ut_count = user_tunnel_repo::count_by_tunnel_id(&state.db, id).await?;
    if fw_count > 0 || ut_count > 0 {
        let mut parts: Vec<String> = Vec::new();
        if fw_count > 0 {
            parts.push(format!("{} 条转发", fw_count));
        }
        if ut_count > 0 {
            parts.push(format!("{} 条用户权限", ut_count));
        }
        return Err(AppError::new(format!(
            "无法删除隧道：仍有 {}，请先删除转发并取消用户隧道分配后再删隧道",
            parts.join("、")
        )));
    }

    // 无业务关联时，仅清 chain_tunnel + tunnel
    let _ = chain_repo::delete_by_tunnel_id(&state.db, id).await;
    tunnel_repo::delete(&state.db, id).await
}

// ---- mine (userTunnel) ----

pub async fn mine(state: &AppState, user_id: i64, role_id: i32) -> AppResult<Vec<Tunnel>> {
    if role_id == 0 {
        return tunnel_repo::list_enabled(&state.db).await;
    }
    let uts = user_tunnel_repo::list_by_user_id(&state.db, user_id).await?;
    if uts.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<i64> = uts.iter().map(|ut| ut.tunnel_id).collect();
    tunnel_repo::list_by_ids(&state.db, &ids).await
}

// ---- diagnose ----

struct PingTask {
    node: Node,
    target_ip: String,
    target_port: i32,
    description: String,
    from_chain_type: Option<i32>,
    from_inx: Option<i32>,
    to_chain_type: Option<i32>,
    to_inx: Option<i32>,
}

pub async fn diagnose(state: &AppState, tunnel_id: i64) -> AppResult<Value> {
    let Some(tunnel) = tunnel_repo::find_by_id(&state.db, tunnel_id).await? else {
        return Err(AppError::new("隧道不存在"));
    };
    let chain_tunnels = chain_repo::list_by_tunnel_id(&state.db, tunnel_id).await?;
    if chain_tunnels.is_empty() {
        return Err(AppError::new("隧道配置不完整"));
    }
    let entries: Vec<ChainTunnel> = chain_tunnels
        .into_iter()
        .filter(|ct| chain_repo::chain_type_of(ct) == 1)
        .collect();

    // 诊断涉及的节点一次查回（替代循环内逐个 GetByID）
    let diag_node_ids: Vec<i64> = {
        let mut set: HashSet<i64> = HashSet::new();
        for ct in &entries {
            set.insert(ct.node_id);
        }
        set.into_iter().collect()
    };
    let diag_nodes = load_nodes_by_ids(&state.db, &diag_node_ids)
        .await
        .unwrap_or_default();
    let diag_node_map: HashMap<i64, Node> = diag_nodes.into_iter().map(|n| (n.id, n)).collect();

    // 先按原顺序组装探测任务，再限并发执行 TcpPing（单次最长 10s，串行是主要耗时）
    let mut tasks: Vec<PingTask> = Vec::new();

    for in_node in &entries {
        let Some(n) = diag_node_map.get(&in_node.node_id) else {
            continue;
        };
        tasks.push(PingTask {
            node: n.clone(),
            target_ip: "www.google.com".to_string(),
            target_port: 443,
            description: format!("入口({})->外网", n.name),
            from_chain_type: Some(1),
            from_inx: None,
            to_chain_type: None,
            to_inx: None,
        });
    }

    let mut results: Vec<DiagnosisResult> = Vec::with_capacity(tasks.len());
    for chunk in tasks.chunks(8) {
        let futs = chunk.iter().map(|t| {
            forward::perform_tcp_ping(
                state,
                &t.node,
                &t.target_ip,
                t.target_port,
                &t.description,
            )
        });
        let pinged = join_all(futs).await;
        for (t, mut r) in chunk.iter().zip(pinged.into_iter()) {
            r.from_chain_type = t.from_chain_type;
            r.from_inx = t.from_inx;
            r.to_chain_type = t.to_chain_type;
            r.to_inx = t.to_inx;
            results.push(r);
        }
    }

    let type_label = "端口转发";
    Ok(json!({
        "tunnelId": tunnel_id,
        "tunnelName": tunnel.name,
        "tunnelType": type_label,
        "results": results,
        "timestamp": now_ms(),
    }))
}

// ---- ports ----


/// 解析节点端口配置："1000-2000,3000"
fn parse_ports(input: &str) -> AppResult<Vec<i32>> {
    if input.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut set: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.contains('-') {
            let mut range_parts = part.splitn(2, '-');
            let a = range_parts.next().unwrap_or("");
            let b = range_parts.next().unwrap_or("");
            let mut start = a
                .trim()
                .parse::<i32>()
                .map_err(|e| AppError::new(e.to_string()))?;
            let mut end = b
                .trim()
                .parse::<i32>()
                .map_err(|e| AppError::new(e.to_string()))?;
            if start > end {
                std::mem::swap(&mut start, &mut end);
            }
            for i in start..=end {
                set.insert(i);
            }
        } else {
            let p = part
                .parse::<i32>()
                .map_err(|e| AppError::new(e.to_string()))?;
            set.insert(p);
        }
    }
    Ok(set.into_iter().collect())
}


// ---- 跨域数据访问（forward / forward_port / node） ----

pub(crate) async fn load_node_by_id(pool: &SqlitePool, id: i64) -> AppResult<Option<Node>> {
    Ok(
        sqlx::query_as::<_, Node>("SELECT * FROM node WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

pub(crate) async fn load_nodes_by_ids(pool: &SqlitePool, ids: &[i64]) -> AppResult<Vec<Node>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT * FROM node WHERE id IN (");
    let mut sep = qb.separated(", ");
    for id in ids {
        sep.push_bind(*id);
    }
    sep.push_unseparated(")");
    Ok(qb.build_query_as::<Node>().fetch_all(pool).await?)
}

pub(crate) async fn list_forwards_by_tunnel_id(
    pool: &SqlitePool,
    tunnel_id: i64,
) -> AppResult<Vec<Forward>> {
    Ok(sqlx::query_as::<_, Forward>(
        "SELECT id, user_id, user_name, name, tunnel_id, remote_addr, strategy,
		in_flow, out_flow, created_time, updated_time, status, inx FROM forward WHERE tunnel_id=?",
    )
    .bind(tunnel_id)
    .fetch_all(pool)
    .await?)
}

pub(crate) async fn list_forwards_by_user_and_tunnel(
    pool: &SqlitePool,
    user_id: i64,
    tunnel_id: i64,
) -> AppResult<Vec<Forward>> {
    Ok(sqlx::query_as::<_, Forward>(
        "SELECT id, user_id, user_name, name, tunnel_id, remote_addr, strategy,
		in_flow, out_flow, created_time, updated_time, status, inx FROM forward WHERE user_id=? AND tunnel_id=?",
    )
    .bind(user_id)
    .bind(tunnel_id)
    .fetch_all(pool)
    .await?)
}

pub(crate) async fn count_forwards_by_tunnel_id(
    pool: &SqlitePool,
    tunnel_id: i64,
) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM forward WHERE tunnel_id=?")
            .bind(tunnel_id)
            .fetch_one(pool)
            .await?,
    )
}

pub(crate) async fn forward_ids_by_tunnel_id(
    pool: &SqlitePool,
    tunnel_id: i64,
) -> AppResult<Vec<i64>> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT id FROM forward WHERE tunnel_id = ?")
            .bind(tunnel_id)
            .fetch_all(pool)
            .await?,
    )
}

pub(crate) async fn list_forward_ports_by_forward_ids(
    pool: &SqlitePool,
    forward_ids: &[i64],
) -> AppResult<HashMap<i64, Vec<ForwardPort>>> {
    if forward_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, forward_id, node_id, port FROM forward_port WHERE forward_id IN (",
    );
    let mut sep = qb.separated(", ");
    for id in forward_ids {
        sep.push_bind(*id);
    }
    sep.push_unseparated(") ORDER BY id");
    let list = qb.build_query_as::<ForwardPort>().fetch_all(pool).await?;
    let mut by_fwd: HashMap<i64, Vec<ForwardPort>> = HashMap::new();
    for fp in list {
        by_fwd.entry(fp.forward_id).or_default().push(fp);
    }
    Ok(by_fwd)
}

pub(crate) async fn list_forward_ports_by_node_id(
    pool: &SqlitePool,
    node_id: i64,
) -> AppResult<Vec<ForwardPort>> {
    Ok(sqlx::query_as::<_, ForwardPort>(
        "SELECT id, forward_id, node_id, port FROM forward_port WHERE node_id=?",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await?)
}

pub(crate) async fn list_forward_ports_by_node_ids(
    pool: &SqlitePool,
    node_ids: &[i64],
) -> AppResult<Vec<ForwardPort>> {
    if node_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, forward_id, node_id, port FROM forward_port WHERE node_id IN (",
    );
    let mut sep = qb.separated(", ");
    for id in node_ids {
        sep.push_bind(*id);
    }
    sep.push_unseparated(")");
    Ok(qb.build_query_as::<ForwardPort>().fetch_all(pool).await?)
}

pub(crate) async fn insert_forward_port(
    pool: &SqlitePool,
    fp: &mut ForwardPort,
) -> AppResult<()> {
    let res = sqlx::query("INSERT INTO forward_port (forward_id, node_id, port) VALUES (?, ?, ?)")
        .bind(fp.forward_id)
        .bind(fp.node_id)
        .bind(fp.port)
        .execute(pool)
        .await?;
    fp.id = res.last_insert_rowid();
    Ok(())
}

pub(crate) async fn delete_forward_ports_by_node_and_forward_ids(
    pool: &SqlitePool,
    node_id: i64,
    forward_ids: &[i64],
) -> AppResult<()> {
    if forward_ids.is_empty() {
        return Ok(());
    }
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("DELETE FROM forward_port WHERE node_id=? AND forward_id IN (");
    qb.push_bind(node_id);
    let mut sep = qb.separated(", ");
    for id in forward_ids {
        sep.push_bind(*id);
    }
    sep.push_unseparated(")");
    qb.build().execute(pool).await?;
    Ok(())
}

pub(crate) async fn delete_forward_ports_by_node(
    pool: &SqlitePool,
    node_id: i64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM forward_port WHERE node_id = ?")
        .bind(node_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub(crate) async fn delete_forward_port_by_forward_id(
    pool: &SqlitePool,
    forward_id: i64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM forward_port WHERE forward_id=?")
        .bind(forward_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub(crate) async fn delete_forward_by_id(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM forward WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
