use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::gost::{self, ChainNodeInput};
use crate::model::{ChainTunnel, Node, Tunnel};
use crate::repo::chain_tunnel as chain_repo;
use crate::repo::tunnel as tunnel_repo;
use crate::response::now_ms;
use crate::service::tunnel::{
    delete_forward_ports_by_node, delete_forward_ports_by_node_and_forward_ids,
    forward_ids_by_tunnel_id, list_forwards_by_tunnel_id, load_node_by_id, load_nodes_map,
    run_node_jobs, PushJob, PushJobKind,
};
use crate::state::AppState;
use crate::ws::GostResult;

fn chain_type_of(ct: &ChainTunnel) -> i32 {
    chain_repo::parse_chain_type(ct.chain_type.trim())
}

// DetachNodeFromTunnels 从所有隧道拓扑中剔除节点，不删除隧道本身。
// 对齐 Java TunnelServiceImpl.detachNodeFromTunnels。
// 失败返回 error（中文文案）；成功可能仅记警告日志。
pub async fn detach_node_from_tunnels(state: &AppState, node_id: i64) -> AppResult<()> {
    if node_id == 0 {
        return Err(AppError::new("节点ID不能为空"));
    }

    let node_links = chain_repo::list_by_node_id(&state.db, node_id).await?;
    if node_links.is_empty() {
        let _ = delete_forward_ports_by_node(&state.db, node_id).await;
        return Ok(());
    }

    let mut tunnel_id_set: HashSet<i64> = HashSet::new();
    let mut tunnel_ids: Vec<i64> = Vec::new();
    for ct in &node_links {
        if tunnel_id_set.insert(ct.tunnel_id) {
            tunnel_ids.push(ct.tunnel_id);
        }
    }

    let mut ctx_map: HashMap<i64, (Tunnel, Vec<ChainTunnel>)> = HashMap::new();
    let mut blockers: Vec<String> = Vec::new();

    for tunnel_id in &tunnel_ids {
        let Some(t) = tunnel_repo::find_by_id(&state.db, *tunnel_id).await? else {
            continue;
        };
        let all = chain_repo::list_by_tunnel_id(&state.db, *tunnel_id).await?;

        let mut remain_entry = 0;
        let mut remain_exit = 0;
        for ct in &all {
            if ct.node_id == node_id {
                continue;
            }
            match chain_type_of(ct) {
                1 => remain_entry += 1,
                3 => remain_exit += 1,
                _ => {}
            }
        }
        if remain_entry == 0 {
            blockers.push(format!("隧道[{}]剔除后将没有入口节点", t.name));
        }
        if t.r#type == 2 && remain_exit == 0 {
            blockers.push(format!("隧道[{}]剔除后将没有出口节点", t.name));
        }
        ctx_map.insert(*tunnel_id, (t, all));
    }

    if !blockers.is_empty() {
        return Err(AppError::new(format!(
            "无法删除节点：{}。请先在隧道编辑中更换入口/出口，或删除相关隧道后再删节点。",
            blockers.join("；")
        )));
    }

    let mut warnings: Vec<String> = Vec::new();

    for tunnel_id in &tunnel_ids {
        let Some((tunnel, all)) = ctx_map.get(tunnel_id) else {
            continue;
        };

        let mut was_entry = false;
        let mut was_exit = false;
        for ct in all {
            if ct.node_id != node_id {
                continue;
            }
            match chain_type_of(ct) {
                1 => was_entry = true,
                3 => was_exit = true,
                _ => {}
            }
        }

        tracing::info!(
            "删除节点时从隧道剔除 nodeId={} tunnelId={} tunnelName={} entry={} exit={}",
            node_id,
            tunnel_id,
            tunnel.name,
            was_entry,
            was_exit
        );

        // 1) 清理该节点上的 gost 配置
        if was_entry {
            let forwards = list_forwards_by_tunnel_id(&state.db, *tunnel_id)
                .await
                .unwrap_or_default();
            for fw in &forwards {
                let ut_id: i64 = sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM user_tunnel WHERE user_id = ? AND tunnel_id = ? LIMIT 1",
                )
                .bind(fw.user_id)
                .bind(*tunnel_id)
                .fetch_optional(&state.db)
                .await
                .unwrap_or(None)
                .unwrap_or(0);
                let base = gost::forward_service_base(fw.id, fw.user_id, ut_id);
                delete_service(
                    state,
                    node_id,
                    vec![gost::forward_tcp(&base), gost::forward_udp(&base)],
                )
                .await;
            }
            if tunnel.r#type == 2 {
                delete_chains(state, node_id, &gost::chain_name(*tunnel_id)).await;
            }
        }
        if was_exit {
            delete_service(state, node_id, vec![gost::tunnel_tls(*tunnel_id)]).await;
        }

        // 2) 删 DB 引用
        chain_repo::delete_by_tunnel_and_node(&state.db, *tunnel_id, node_id).await?;
        if was_entry {
            let forward_ids = forward_ids_by_tunnel_id(&state.db, *tunnel_id)
                .await
                .unwrap_or_default();
            if !forward_ids.is_empty() {
                let _ = delete_forward_ports_by_node_and_forward_ids(
                    &state.db,
                    node_id,
                    &forward_ids,
                )
                .await;
            }
        }

        // 3) 剩余拓扑
        let remaining = chain_repo::list_by_tunnel_id(&state.db, *tunnel_id).await?;
        let mut entries: Vec<ChainTunnel> = Vec::new();
        let mut outs: Vec<ChainTunnel> = Vec::new();
        for ct in remaining {
            match chain_type_of(&ct) {
                1 => entries.push(ct),
                3 => outs.push(ct),
                _ => {}
            }
        }

        // 4) 刷新 in_ip
        refresh_tunnel_in_ip(state, tunnel, &entries).await;

        // 5) type=2 重推（按节点分组并行，同节点 chain 先于 service）
        if tunnel.r#type == 2 {
            let mut node_id_set: HashSet<i64> = HashSet::new();
            for ct in &entries {
                node_id_set.insert(ct.node_id);
            }
            for ct in &outs {
                node_id_set.insert(ct.node_id);
            }
            let node_ids: Vec<i64> = node_id_set.into_iter().collect();
            let nodes = load_nodes_map(&state.db, &node_ids).await;

            let mut jobs: Vec<PushJob> = Vec::new();
            for entry in &entries {
                let n = nodes.get(&entry.node_id);
                if n.map(|n| n.status != 1).unwrap_or(true) {
                    warnings.push(format!("隧道[{}]入口节点离线或缺失，跳过链重推", tunnel.name));
                    continue;
                }
                let target = outs.clone();
                if target.is_empty() {
                    warnings.push(format!("隧道[{}]入口后无下一跳，跳过链重推", tunnel.name));
                    continue;
                }
                jobs.push(PushJob {
                    node_id: entry.node_id,
                    kind: PushJobKind::DetachEntry {
                        tunnel_name: tunnel.name.clone(),
                        target,
                    },
                });
            }

            for ct in &outs {
                let n = nodes.get(&ct.node_id);
                if n.map(|n| n.status != 1).unwrap_or(true) {
                    warnings.push(format!(
                        "隧道[{}]出口节点离线或缺失，跳过服务重推",
                        tunnel.name
                    ));
                    continue;
                }
                jobs.push(PushJob {
                    node_id: ct.node_id,
                    kind: PushJobKind::DetachOut {
                        tunnel_name: tunnel.name.clone(),
                        ct: ct.clone(),
                    },
                });
            }
            warnings.extend(run_node_jobs(state, &nodes, jobs).await);
        }
    }

    // 兜底清理 forward_port
    let _ = delete_forward_ports_by_node(&state.db, node_id).await;

    if !warnings.is_empty() {
        tracing::warn!(
            "删除节点时部分隧道重推/清理有警告 nodeId={} warnings={:?}",
            node_id,
            warnings
        );
        // DB 已剔除成功，不阻断删除
    }
    Ok(())
}

pub(crate) async fn delete_service(state: &AppState, node_id: i64, names: Vec<String>) {
    // 清理类删除无需等待节点确认，避免慢响应时被 10s 超时拖住
    state
        .hub
        .send_no_wait(node_id, gost::delete_service_payload(names), "DeleteService")
        .await;
}

pub(crate) async fn delete_chains(state: &AppState, node_id: i64, name: &str) {
    state
        .hub
        .send_no_wait(node_id, json!({ "chain": name }), "DeleteChains")
        .await;
}

async fn refresh_tunnel_in_ip(state: &AppState, tunnel: &Tunnel, entries: &[ChainTunnel]) {
    let mut parts: Vec<String> = Vec::new();
    for entry in entries {
        match load_node_by_id(&state.db, entry.node_id).await {
            Ok(Some(n)) if !n.server_ip.is_empty() => parts.push(n.server_ip),
            _ => continue,
        }
    }
    let in_ip = parts.join(",");
    let now = now_ms();
    let _ = tunnel_repo::update_in_ip(&state.db, tunnel.id, &in_ip, now).await;
}

pub async fn push_chains(
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
    let name = data.get("name").cloned().unwrap_or_default();
    // Update 优先
    let req = json!({
        "chain": name,
        "data": data,
    });
    let mut res = state.hub.send_msg(node_id, req, "UpdateChains").await;
    // 空节点 Update 返回 not found，必须 Add（IsOK 不再吞掉 not found）
    if !gost::is_ok(&res.msg) {
        res = state.hub.send_msg(node_id, data, "AddChains").await;
    }
    res.msg = gost::normalize_ok(&res.msg);
    res
}

pub async fn push_chain_service(
    state: &AppState,
    node_id: i64,
    ct: &ChainTunnel,
    nodes: &HashMap<i64, Node>,
) -> GostResult {
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
        chain_type_of(ct),
        &proto,
        &tcp_listen,
        port,
        &iface,
        src_has,
    );
    let mut res = state
        .hub
        .send_msg(node_id, json!(services.clone()), "UpdateService")
        .await;
    if !gost::is_ok(&res.msg) {
        res = state
            .hub
            .send_msg(node_id, json!(services), "AddService")
            .await;
    }
    res.msg = gost::normalize_ok(&res.msg);
    res
}
