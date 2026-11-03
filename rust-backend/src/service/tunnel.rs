use std::collections::{BTreeMap, HashMap, HashSet};

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::error::{AppError, AppResult};
use crate::gost::{self, ChainNodeInput};
use crate::model::{ChainTunnel, Forward, ForwardPort, Node, Tunnel, UserTunnel};
use crate::repo::chain_tunnel as chain_repo;
use crate::repo::tunnel as tunnel_repo;
use crate::repo::user_tunnel as user_tunnel_repo;
use crate::response::now_ms;
use crate::service::forward::{self, DiagnosisResult};
pub use crate::service::tunnel_detach::{push_chain_service, push_chains};
use crate::service::tunnel_detach::{delete_chains, delete_service};
use crate::state::AppState;
use crate::ws::GostResult;

// ---- DTO ----

/// 前端传入的拓扑节点（不信任 id/port）
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ChainTunnelInput {
    #[serde(default, rename = "nodeId")]
    pub node_id: i64,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub strategy: Option<String>,
    /// 入口节点使用的出口节点 ID 列表，支持数组或 JSON 字符串
    #[serde(default, rename = "exitNodeIds")]
    pub exit_node_ids: Option<Value>,
}

impl ChainTunnelInput {
    /// 解析 exitNodeIds，支持数组和 JSON 字符串两种格式
    pub fn get_exit_node_ids(&self) -> Vec<i64> {
        let Some(v) = self.exit_node_ids.as_ref() else {
            return Vec::new();
        };
        match v {
            Value::Array(arr) => {
                let mut ids = Vec::with_capacity(arr.len());
                for item in arr {
                    if let Some(n) = item.as_i64() {
                        ids.push(n);
                    } else if let Some(f) = item.as_f64() {
                        ids.push(f as i64);
                    }
                }
                ids
            }
            Value::String(s) => {
                if s.is_empty() || s == "null" {
                    return Vec::new();
                }
                serde_json::from_str::<Vec<i64>>(s).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }
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
    #[serde(default, deserialize_with = "null_default", rename = "inNodeId")]
    pub in_node_id: Vec<ChainTunnelInput>,
    #[serde(default, deserialize_with = "null_default", rename = "chainNodes")]
    pub chain_nodes: Vec<Vec<ChainTunnelInput>>,
    #[serde(default, deserialize_with = "null_default", rename = "outNodeId")]
    pub out_node_id: Vec<ChainTunnelInput>,
}

/// Go 的 JSON 反序列化接受 null 切片（视为空），serde 需显式转换
fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
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
    #[serde(default, deserialize_with = "null_default", rename = "inNodeId")]
    pub in_node_id: Vec<ChainTunnelInput>,
    #[serde(default, deserialize_with = "null_default", rename = "chainNodes")]
    pub chain_nodes: Vec<Vec<ChainTunnelInput>>,
    #[serde(default, deserialize_with = "null_default", rename = "outNodeId")]
    pub out_node_id: Vec<ChainTunnelInput>,
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
    pub chain_nodes: Vec<Vec<ChainTunnel>>,
    pub out_node_id: Vec<ChainTunnel>,
}

// ---- create ----

pub async fn create(state: &AppState, req: TunnelCreateReq) -> AppResult<()> {
    if req.name.trim().is_empty() {
        return Err(AppError::new("隧道名称不能为空"));
    }
    if req.in_node_id.is_empty() {
        return Err(AppError::new("入口节点不能为空"));
    }
    if req.r#type != 1 && req.r#type != 2 {
        return Err(AppError::new("隧道类型不合法"));
    }
    if req.r#type == 2 && req.out_node_id.is_empty() {
        return Err(AppError::new("出口不能为空"));
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
    if req.r#type == 2 {
        for group in &req.chain_nodes {
            for c in group {
                if c.node_id == 0 {
                    return Err(AppError::new("节点不存在"));
                }
                node_ids.push(c.node_id);
            }
        }
        for out in &req.out_node_id {
            if out.node_id == 0 {
                return Err(AppError::new("节点不存在"));
            }
            node_ids.push(out.node_id);
        }
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
        let exit_ids = input.get_exit_node_ids();
        let mut exit_node_ids: Option<String> = None;
        if !exit_ids.is_empty() {
            let s = serde_json::to_string(&exit_ids)
                .map_err(|e| AppError::new(format!("序列化出口节点 ID 失败: {}", e)))?;
            exit_node_ids = Some(s);
        }
        chain_tunnels.push(chain_repo::new_chain_tunnel_with_exit_binding(
            0,
            1,
            input.node_id,
            None,
            None,
            None,
            None,
            exit_node_ids,
        ));
    }

    // type2: 转发链 + 出口
    let mut chain_groups: Vec<Vec<ChainTunnel>> = Vec::new();
    let mut out_nodes: Vec<ChainTunnel> = Vec::new();
    if req.r#type == 2 {
        let mut inx = 1;
        for group in &req.chain_nodes {
            let mut new_group: Vec<ChainTunnel> = Vec::new();
            for c in group {
                if !nodes.contains_key(&c.node_id) {
                    return Err(AppError::new("节点不存在"));
                }
                // 端口分配保持串行：分配结果依赖库中实时占用，并发会选出重复端口
                let port = get_node_port(state, c.node_id).await?;
                let ct = chain_repo::new_chain_tunnel(
                    0,
                    2,
                    c.node_id,
                    Some(port),
                    c.strategy.clone(),
                    c.protocol.clone(),
                    Some(inx),
                );
                new_group.push(ct.clone());
                chain_tunnels.push(ct);
            }
            if !new_group.is_empty() {
                chain_groups.push(new_group);
                inx += 1;
            }
        }
        for out in &req.out_node_id {
            if !nodes.contains_key(&out.node_id) {
                return Err(AppError::new("节点不存在"));
            }
            let port = get_node_port(state, out.node_id).await?;
            let ct = chain_repo::new_chain_tunnel(
                0,
                3,
                out.node_id,
                Some(port),
                out.strategy.clone(),
                out.protocol.clone(),
                None,
            );
            out_nodes.push(ct.clone());
            chain_tunnels.push(ct);
        }
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
        r#type: req.r#type,
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
    for group in chain_groups.iter_mut() {
        for ct in group.iter_mut() {
            ct.tunnel_id = tunnel.id;
        }
    }
    for ct in out_nodes.iter_mut() {
        ct.tunnel_id = tunnel.id;
    }
    // 入口列表（用于下发）
    let entries: Vec<ChainTunnel> = chain_tunnels
        .iter()
        .filter(|ct| chain_repo::chain_type_of(ct) == 1)
        .cloned()
        .collect();

    if let Err(e) = chain_repo::insert_batch(&state.db, &mut chain_tunnels).await {
        let _ = tunnel_repo::delete(&state.db, tunnel.id).await;
        return Err(e);
    }

    // type2 下发：按节点分组并行（同节点 chain 先于 service），任一下发失败整体回滚
    if tunnel.r#type == 2 {
        let mut jobs: Vec<PushJob> = Vec::new();
        // 入口 → 第一跳 / 出口
        for entry in &entries {
            let target = if chain_groups.is_empty() {
                filter_exits_by_entry(&out_nodes, entry)
            } else {
                chain_groups[0].clone()
            };
            jobs.push(PushJob {
                node_id: entry.node_id,
                kind: PushJobKind::CreateEntry {
                    target,
                    tolerate_failure: chain_groups.is_empty(),
                },
            });
        }

        // 转发链
        for i in 0..chain_groups.len() {
            let target = if i + 1 < chain_groups.len() {
                chain_groups[i + 1].clone()
            } else {
                out_nodes.clone()
            };
            for ct in &chain_groups[i] {
                jobs.push(PushJob {
                    node_id: ct.node_id,
                    kind: PushJobKind::CreateChain {
                        ct: ct.clone(),
                        target: target.clone(),
                    },
                });
            }
        }

        // 出口
        for ct in &out_nodes {
            jobs.push(PushJob {
                node_id: ct.node_id,
                kind: PushJobKind::CreateOut { ct: ct.clone() },
            });
        }

        let fails = run_node_jobs(state, &nodes, jobs).await;
        if let Some(first) = fails.first() {
            // 回滚
            let _ = tunnel_repo::delete(&state.db, tunnel.id).await;
            let _ = chain_repo::delete_by_tunnel_id(&state.db, tunnel.id).await;
            for entry in &entries {
                delete_chains(state, entry.node_id, &gost::chain_name(tunnel.id)).await;
            }
            for group in &chain_groups {
                for ct in group {
                    delete_chains(state, ct.node_id, &gost::chain_name(tunnel.id)).await;
                    delete_service(state, ct.node_id, vec![gost::tunnel_tls(tunnel.id)]).await;
                }
            }
            for ct in &out_nodes {
                delete_service(state, ct.node_id, vec![gost::tunnel_tls(tunnel.id)]).await;
            }
            return Err(AppError::new(first.clone()));
        }
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
        let (entries, chains, outs) = split_topology(cts);
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
            chain_nodes: chains,
            out_node_id: outs,
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
    let is_tunnel_forward = tunnel.r#type == 2;

    let chain_groups_req = &dto.chain_nodes;
    let out_nodes_req = &dto.out_node_id;
    if is_tunnel_forward && out_nodes_req.is_empty() {
        return Err(AppError::new("出口不能为空"));
    }

    // 规范化
    let mut new_entries: Vec<ChainTunnel> = Vec::new();
    let mut new_chains: Vec<Vec<ChainTunnel>> = Vec::new();
    let mut new_outs: Vec<ChainTunnel> = Vec::new();
    let mut node_ids: Vec<i64> = Vec::new();

    for input in &dto.in_node_id {
        if input.node_id == 0 {
            return Err(AppError::new("节点不存在"));
        }
        let exit_ids = input.get_exit_node_ids();
        let mut exit_node_ids: Option<String> = None;
        if !exit_ids.is_empty() {
            let s = serde_json::to_string(&exit_ids)
                .map_err(|e| AppError::new(format!("序列化出口节点 ID 失败: {}", e)))?;
            exit_node_ids = Some(s);
        }
        new_entries.push(chain_repo::new_chain_tunnel_with_exit_binding(
            tunnel_id,
            1,
            input.node_id,
            None,
            None,
            None,
            None,
            exit_node_ids,
        ));
        node_ids.push(input.node_id);
    }
    let mut inx = 1;
    for group in chain_groups_req {
        let mut new_group: Vec<ChainTunnel> = Vec::new();
        for c in group {
            if c.node_id == 0 {
                return Err(AppError::new("节点不存在"));
            }
            let ct = chain_repo::new_chain_tunnel(
                tunnel_id,
                2,
                c.node_id,
                None,
                c.strategy.clone(),
                c.protocol.clone(),
                Some(inx),
            );
            new_group.push(ct);
            node_ids.push(c.node_id);
        }
        if !new_group.is_empty() {
            new_chains.push(new_group);
            inx += 1;
        }
    }
    for out in out_nodes_req {
        if out.node_id == 0 {
            return Err(AppError::new("节点不存在"));
        }
        let ct = chain_repo::new_chain_tunnel(
            tunnel_id,
            3,
            out.node_id,
            None,
            out.strategy.clone(),
            out.protocol.clone(),
            None,
        );
        new_outs.push(ct);
        node_ids.push(out.node_id);
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
    let mut old_ports: HashMap<i64, i32> = HashMap::new();
    for old in &old_records {
        if let Some(p) = old.port {
            old_ports.insert(old.node_id, p);
        }
    }

    // 端口沿用
    for group in new_chains.iter_mut() {
        for ct in group.iter_mut() {
            if let Some(p) = old_ports.get(&ct.node_id) {
                ct.port = Some(*p);
            } else {
                ct.port = Some(get_node_port(state, ct.node_id).await?);
            }
        }
    }
    for ct in new_outs.iter_mut() {
        if let Some(p) = old_ports.get(&ct.node_id) {
            ct.port = Some(*p);
        } else {
            ct.port = Some(get_node_port(state, ct.node_id).await?);
        }
    }

    let mut failures: Vec<String> = Vec::new();

    // 清理旧 gost
    if is_tunnel_forward {
        let mut chain_needed: HashSet<i64> = HashSet::new();
        let mut service_needed: HashSet<i64> = HashSet::new();
        for ct in &new_entries {
            chain_needed.insert(ct.node_id);
        }
        for group in &new_chains {
            for ct in group {
                chain_needed.insert(ct.node_id);
                service_needed.insert(ct.node_id);
            }
        }
        for ct in &new_outs {
            service_needed.insert(ct.node_id);
        }
        let mut chain_cleaned: HashSet<i64> = HashSet::new();
        let mut service_cleaned: HashSet<i64> = HashSet::new();
        for old in &old_records {
            let ct = chain_repo::chain_type_of(old);
            let had_chain = ct == 1 || ct == 2;
            let had_service = ct == 2 || ct == 3;
            if had_chain
                && !chain_needed.contains(&old.node_id)
                && chain_cleaned.insert(old.node_id)
            {
                delete_chains(state, old.node_id, &gost::chain_name(tunnel_id)).await;
            }
            if had_service
                && !service_needed.contains(&old.node_id)
                && service_cleaned.insert(old.node_id)
            {
                delete_service(state, old.node_id, vec![gost::tunnel_tls(tunnel_id)]).await;
            }
        }
    }

    // DB 替换
    chain_repo::delete_by_tunnel_id(&state.db, tunnel_id).await?;
    let mut all_new: Vec<ChainTunnel> = Vec::new();
    all_new.extend(new_entries.iter().cloned());
    for g in &new_chains {
        all_new.extend(g.iter().cloned());
    }
    all_new.extend(new_outs.iter().cloned());
    chain_repo::insert_batch(&state.db, &mut all_new).await?;

    // 下发：按节点分组并行（同节点上 chain 必须先于 service）
    if is_tunnel_forward {
        let mut jobs: Vec<PushJob> = Vec::new();
        for entry in &new_entries {
            let target = if new_chains.is_empty() {
                filter_exits_by_entry(&new_outs, entry)
            } else {
                new_chains[0].clone()
            };
            jobs.push(PushJob {
                node_id: entry.node_id,
                kind: PushJobKind::UpdateEntry { target },
            });
        }
        for i in 0..new_chains.len() {
            let target = if i + 1 < new_chains.len() {
                new_chains[i + 1].clone()
            } else {
                new_outs.clone()
            };
            for ct in &new_chains[i] {
                jobs.push(PushJob {
                    node_id: ct.node_id,
                    kind: PushJobKind::UpdateChain {
                        ct: ct.clone(),
                        target: target.clone(),
                    },
                });
            }
        }
        for ct in &new_outs {
            jobs.push(PushJob {
                node_id: ct.node_id,
                kind: PushJobKind::UpdateOut { ct: ct.clone() },
            });
        }
        failures.extend(run_node_jobs(state, &nodes, jobs).await);
    }

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

    // 无业务关联时，仅清 gost 拓扑 + chain_tunnel + tunnel
    let chain_tunnels = chain_repo::list_by_tunnel_id(&state.db, id)
        .await
        .unwrap_or_default();
    for ct in &chain_tunnels {
        match chain_repo::chain_type_of(ct) {
            1 => {
                delete_chains(state, ct.node_id, &gost::chain_name(ct.tunnel_id)).await;
            }
            2 => {
                delete_chains(state, ct.node_id, &gost::chain_name(ct.tunnel_id)).await;
                delete_service(state, ct.node_id, vec![gost::tunnel_tls(ct.tunnel_id)]).await;
            }
            _ => {
                delete_service(state, ct.node_id, vec![gost::tunnel_tls(ct.tunnel_id)]).await;
            }
        }
    }
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
    let (entries, chains, outs) = split_topology(chain_tunnels.clone());

    // 诊断涉及的节点一次查回（替代循环内逐个 GetByID）
    let diag_node_ids: Vec<i64> = {
        let mut set: HashSet<i64> = HashSet::new();
        for ct in &chain_tunnels {
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

    if tunnel.r#type == 1 {
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
    } else if tunnel.r#type == 2 {
        for in_node in &entries {
            let Some(from) = diag_node_map.get(&in_node.node_id) else {
                continue;
            };
            if !chains.is_empty() {
                for first in &chains[0] {
                    let Some(to) = diag_node_map.get(&first.node_id) else {
                        continue;
                    };
                    let port = first.port.unwrap_or(0);
                    tasks.push(PingTask {
                        node: from.clone(),
                        target_ip: to.server_ip.clone(),
                        target_port: port,
                        description: format!("入口({})->第1跳({})", from.name, to.name),
                        from_chain_type: Some(1),
                        from_inx: None,
                        to_chain_type: Some(2),
                        to_inx: first.inx,
                    });
                }
            } else {
                // 根据入口的 ExitNodeIDs 过滤出口节点
                let target_outs = filter_exits_by_entry(&outs, in_node);
                for out in &target_outs {
                    let Some(to) = diag_node_map.get(&out.node_id) else {
                        continue;
                    };
                    let port = out.port.unwrap_or(0);
                    tasks.push(PingTask {
                        node: from.clone(),
                        target_ip: to.server_ip.clone(),
                        target_port: port,
                        description: format!("入口({})->出口({})", from.name, to.name),
                        from_chain_type: Some(1),
                        from_inx: None,
                        to_chain_type: Some(3),
                        to_inx: None,
                    });
                }
            }
        }

        for i in 0..chains.len() {
            for current in &chains[i] {
                let Some(from) = diag_node_map.get(&current.node_id) else {
                    continue;
                };
                if i + 1 < chains.len() {
                    for next in &chains[i + 1] {
                        let Some(to) = diag_node_map.get(&next.node_id) else {
                            continue;
                        };
                        let port = next.port.unwrap_or(0);
                        tasks.push(PingTask {
                            node: from.clone(),
                            target_ip: to.server_ip.clone(),
                            target_port: port,
                            description: format!(
                                "第{}跳({})->第{}跳({})",
                                i + 1,
                                from.name,
                                i + 2,
                                to.name
                            ),
                            from_chain_type: Some(2),
                            from_inx: current.inx,
                            to_chain_type: Some(2),
                            to_inx: next.inx,
                        });
                    }
                } else {
                    for out in &outs {
                        let Some(to) = diag_node_map.get(&out.node_id) else {
                            continue;
                        };
                        let port = out.port.unwrap_or(0);
                        tasks.push(PingTask {
                            node: from.clone(),
                            target_ip: to.server_ip.clone(),
                            target_port: port,
                            description: format!(
                                "第{}跳({})->出口({})",
                                i + 1,
                                from.name,
                                to.name
                            ),
                            from_chain_type: Some(2),
                            from_inx: current.inx,
                            to_chain_type: Some(3),
                            to_inx: None,
                        });
                    }
                }
            }
        }

        for out in &outs {
            let Some(n) = diag_node_map.get(&out.node_id) else {
                continue;
            };
            tasks.push(PingTask {
                node: n.clone(),
                target_ip: "www.google.com".to_string(),
                target_port: 443,
                description: format!("出口({})->外网", n.name),
                from_chain_type: Some(3),
                from_inx: None,
                to_chain_type: None,
                to_inx: None,
            });
        }
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

    let type_label = if tunnel.r#type == 2 {
        "隧道转发"
    } else {
        "端口转发"
    };
    Ok(json!({
        "tunnelId": tunnel_id,
        "tunnelName": tunnel.name,
        "tunnelType": type_label,
        "results": results,
        "timestamp": now_ms(),
    }))
}

// ---- ports ----

async fn get_node_port(state: &AppState, node_id: i64) -> AppResult<i32> {
    let ports = get_available_ports(state, node_id).await?;
    ports
        .first()
        .copied()
        .ok_or_else(|| AppError::new("节点端口已满，无可用端口"))
}

async fn get_available_ports(state: &AppState, node_id: i64) -> AppResult<Vec<i32>> {
    let Some(node) = load_node_by_id(&state.db, node_id).await? else {
        return Err(AppError::new("节点不存在"));
    };
    let mut used: HashSet<i32> = HashSet::new();
    let cts = chain_repo::list_by_node_id(&state.db, node_id).await?;
    for ct in cts {
        if let Some(p) = ct.port {
            used.insert(p);
        }
    }
    let fps = list_forward_ports_by_node_id(&state.db, node_id).await?;
    for fp in fps {
        used.insert(fp.port);
    }
    let parsed = parse_ports(&node.port)?;
    Ok(parsed.into_iter().filter(|p| !used.contains(p)).collect())
}

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

// ---- gost helpers (create path: Add*) ----

async fn add_chains(
    state: &AppState,
    node_id: i64,
    target: &[ChainTunnel],
    nodes: &HashMap<i64, Node>,
) -> GostResult {
    if target.is_empty() {
        return GostResult {
            msg: "节点不在线".to_string(),
            data: None,
        };
    }
    let tunnel_id = target[0].tunnel_id;
    let src = nodes.get(&node_id);
    let iface = src
        .and_then(|n| n.interface_name.clone())
        .unwrap_or_default();
    let strategy = target[0].strategy.clone().unwrap_or_default();
    let mut inputs: Vec<ChainNodeInput> = Vec::with_capacity(target.len());
    for ct in target {
        let Some(n) = nodes.get(&ct.node_id) else {
            continue;
        };
        inputs.push(ChainNodeInput {
            protocol: ct.protocol.clone().unwrap_or_default(),
            server_ip: n.server_ip.clone(),
            port: ct.port.unwrap_or(0),
        });
    }
    let data = gost::build_chain_data(tunnel_id, node_id, &iface, &strategy, &inputs);
    // 节点未连接时跳过下发，仅保存数据库配置
    if !state.hub.is_node_online(node_id) {
        return GostResult {
            msg: "OK".to_string(),
            data: None,
        };
    }
    let mut res = state.hub.send_msg(node_id, data, "AddChains").await;
    res.msg = gost::normalize_ok(&res.msg);
    res
}

async fn add_chain_service(
    state: &AppState,
    node_id: i64,
    ct: &ChainTunnel,
    nodes: &HashMap<i64, Node>,
) -> GostResult {
    // 节点未连接时跳过下发，仅保存数据库配置
    if !state.hub.is_node_online(node_id) {
        return GostResult {
            msg: "OK".to_string(),
            data: None,
        };
    }
    let Some(n) = nodes.get(&ct.node_id) else {
        return GostResult {
            msg: "节点不存在".to_string(),
            data: None,
        };
    };
    let src = nodes.get(&node_id);
    let iface = src
        .and_then(|s| s.interface_name.clone())
        .unwrap_or_default();
    let src_has = src
        .and_then(|s| s.interface_name.as_deref())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let proto = ct
        .protocol
        .clone()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "tls".to_string());
    let port = ct.port.unwrap_or(0);
    let mut tcp_listen = n.tcp_listen_addr.clone();
    if tcp_listen.is_empty() {
        tcp_listen = "[::]".to_string();
    }
    let services = gost::build_chain_service(
        ct.tunnel_id,
        chain_repo::chain_type_of(ct),
        &proto,
        &tcp_listen,
        port,
        &iface,
        src_has,
    );
    let mut res = state
        .hub
        .send_msg(node_id, json!(services), "AddService")
        .await;
    res.msg = gost::normalize_ok(&res.msg);
    res
}

// ---- job runner ----

pub(crate) enum PushJobKind {
    CreateEntry {
        target: Vec<ChainTunnel>,
        tolerate_failure: bool,
    },
    CreateChain {
        ct: ChainTunnel,
        target: Vec<ChainTunnel>,
    },
    CreateOut {
        ct: ChainTunnel,
    },
    UpdateEntry {
        target: Vec<ChainTunnel>,
    },
    UpdateChain {
        ct: ChainTunnel,
        target: Vec<ChainTunnel>,
    },
    UpdateOut {
        ct: ChainTunnel,
    },
    DetachEntry {
        tunnel_name: String,
        target: Vec<ChainTunnel>,
    },
    DetachChain {
        tunnel_name: String,
        ct: ChainTunnel,
        target: Vec<ChainTunnel>,
    },
    DetachOut {
        tunnel_name: String,
        ct: ChainTunnel,
    },
}

pub(crate) struct PushJob {
    pub(crate) node_id: i64,
    pub(crate) kind: PushJobKind,
}

async fn run_job(
    state: &AppState,
    node_id: i64,
    kind: &PushJobKind,
    nodes: &HashMap<i64, Node>,
) -> Option<String> {
    match kind {
        PushJobKind::CreateEntry {
            target,
            tolerate_failure,
        } => {
            let res = add_chains(state, node_id, target, nodes).await;
            if !gost::is_ok(&res.msg) {
                if *tolerate_failure {
                    return None;
                }
                return Some(gost::normalize_ok(&res.msg));
            }
            None
        }
        PushJobKind::CreateChain { ct, target } => {
            let mut msgs: Vec<String> = Vec::new();
            let res = add_chains(state, node_id, target, nodes).await;
            if !gost::is_ok(&res.msg) {
                msgs.push(gost::normalize_ok(&res.msg));
            }
            let res = add_chain_service(state, node_id, ct, nodes).await;
            if !gost::is_ok(&res.msg) {
                msgs.push(gost::normalize_ok(&res.msg));
            }
            join_msgs(msgs)
        }
        PushJobKind::CreateOut { ct } => {
            let res = add_chain_service(state, node_id, ct, nodes).await;
            if !gost::is_ok(&res.msg) {
                return Some(gost::normalize_ok(&res.msg));
            }
            None
        }
        PushJobKind::UpdateEntry { target } => {
            let res = push_chains(state, node_id, target, nodes).await;
            if !gost::is_ok(&res.msg) {
                return Some(format!(
                    "入口[{}]链下发失败: {}",
                    node_name(nodes, node_id),
                    res.msg
                ));
            }
            None
        }
        PushJobKind::UpdateChain { ct, target } => {
            let mut msgs: Vec<String> = Vec::new();
            let res = push_chains(state, node_id, target, nodes).await;
            if !gost::is_ok(&res.msg) {
                msgs.push(format!(
                    "转发链[{}]链下发失败: {}",
                    node_name(nodes, node_id),
                    res.msg
                ));
            }
            let res = push_chain_service(state, node_id, ct, nodes).await;
            if !gost::is_ok(&res.msg) {
                msgs.push(format!(
                    "转发链[{}]服务下发失败: {}",
                    node_name(nodes, node_id),
                    res.msg
                ));
            }
            join_msgs(msgs)
        }
        PushJobKind::UpdateOut { ct } => {
            let res = push_chain_service(state, node_id, ct, nodes).await;
            if !gost::is_ok(&res.msg) {
                return Some(format!(
                    "出口[{}]服务下发失败: {}",
                    node_name(nodes, node_id),
                    res.msg
                ));
            }
            None
        }
        PushJobKind::DetachEntry {
            tunnel_name,
            target,
        } => {
            let res = push_chains(state, node_id, target, nodes).await;
            if !gost::is_ok(&res.msg) {
                return Some(format!("隧道[{}]入口链重推失败: {}", tunnel_name, res.msg));
            }
            None
        }
        PushJobKind::DetachChain {
            tunnel_name,
            ct,
            target,
        } => {
            let mut msgs: Vec<String> = Vec::new();
            if !target.is_empty() {
                let res = push_chains(state, node_id, target, nodes).await;
                if !gost::is_ok(&res.msg) {
                    msgs.push(format!(
                        "隧道[{}]转发链重推失败: {}",
                        tunnel_name, res.msg
                    ));
                }
            }
            let res = push_chain_service(state, node_id, ct, nodes).await;
            if !gost::is_ok(&res.msg) {
                msgs.push(format!(
                    "隧道[{}]转发链服务重推失败: {}",
                    tunnel_name, res.msg
                ));
            }
            join_msgs(msgs)
        }
        PushJobKind::DetachOut { tunnel_name, ct } => {
            let res = push_chain_service(state, node_id, ct, nodes).await;
            if !gost::is_ok(&res.msg) {
                return Some(format!(
                    "隧道[{}]出口服务重推失败: {}",
                    tunnel_name, res.msg
                ));
            }
            None
        }
    }
}

fn join_msgs(msgs: Vec<String>) -> Option<String> {
    if msgs.is_empty() {
        None
    } else {
        Some(msgs.join("；"))
    }
}

/// 按 nodeID 分组后顺序执行，返回按节点首次出现顺序、组内顺序收集的非空失败文案。
pub(crate) async fn run_node_jobs(
    state: &AppState,
    nodes: &HashMap<i64, Node>,
    jobs: Vec<PushJob>,
) -> Vec<String> {
    if jobs.is_empty() {
        return Vec::new();
    }
    let mut groups: Vec<(i64, Vec<PushJobKind>)> = Vec::new();
    for job in jobs {
        if let Some(group) = groups.iter_mut().find(|(id, _)| *id == job.node_id) {
            group.1.push(job.kind);
        } else {
            groups.push((job.node_id, vec![job.kind]));
        }
    }

    let mut out: Vec<String> = Vec::new();
    for (node_id, kinds) in groups {
        for kind in &kinds {
            if let Some(msg) = run_job(state, node_id, kind, nodes).await {
                out.push(msg);
            }
        }
    }
    out
}

// ---- topology helpers ----

pub fn split_topology(
    cts: Vec<ChainTunnel>,
) -> (
    Vec<ChainTunnel>,
    Vec<Vec<ChainTunnel>>,
    Vec<ChainTunnel>,
) {
    let mut entries: Vec<ChainTunnel> = Vec::new();
    let mut outs: Vec<ChainTunnel> = Vec::new();
    let mut groups: BTreeMap<i32, Vec<ChainTunnel>> = BTreeMap::new();
    for ct in cts {
        match chain_repo::chain_type_of(&ct) {
            1 => entries.push(ct),
            2 => {
                let inx = ct.inx.unwrap_or(0);
                groups.entry(inx).or_default().push(ct);
            }
            3 => outs.push(ct),
            _ => {}
        }
    }
    let chains: Vec<Vec<ChainTunnel>> = groups.into_values().collect();
    (entries, chains, outs)
}

pub(crate) fn node_name(nodes: &HashMap<i64, Node>, id: i64) -> String {
    match nodes.get(&id) {
        Some(n) => n.name.clone(),
        None => id.to_string(),
    }
}

/// 根据入口节点的 ExitNodeIDs 过滤出口节点
/// 如果入口没有指定 ExitNodeIDs，则返回所有出口节点
fn filter_exits_by_entry(out_nodes: &[ChainTunnel], entry: &ChainTunnel) -> Vec<ChainTunnel> {
    let Some(raw) = entry.exit_node_ids.as_deref() else {
        return out_nodes.to_vec();
    };
    if raw.is_empty() {
        return out_nodes.to_vec();
    }
    let exit_ids: Vec<i64> = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return out_nodes.to_vec(),
    };
    if exit_ids.is_empty() {
        return out_nodes.to_vec();
    }
    let exit_id_set: HashSet<i64> = exit_ids.into_iter().collect();
    let filtered: Vec<ChainTunnel> = out_nodes
        .iter()
        .filter(|node| exit_id_set.contains(&node.node_id))
        .cloned()
        .collect();
    if filtered.is_empty() {
        return out_nodes.to_vec();
    }
    filtered
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

pub(crate) async fn load_nodes_map(
    pool: &SqlitePool,
    ids: &[i64],
) -> HashMap<i64, Node> {
    match load_nodes_by_ids(pool, ids).await {
        Ok(list) => list.into_iter().map(|n| (n.id, n)).collect(),
        Err(_) => HashMap::new(),
    }
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
