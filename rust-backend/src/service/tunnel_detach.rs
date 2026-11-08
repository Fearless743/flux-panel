use std::collections::{HashMap, HashSet};

use crate::error::{AppError, AppResult};
use crate::gost;
use crate::model::{ChainTunnel, Tunnel};
use crate::repo::chain_tunnel as chain_repo;
use crate::repo::tunnel as tunnel_repo;
use crate::response::now_ms;
use crate::service::tunnel::{
    delete_forward_ports_by_node, delete_forward_ports_by_node_and_forward_ids,
    forward_ids_by_tunnel_id, list_forwards_by_tunnel_id, load_node_by_id,
};
use crate::state::AppState;

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
        for ct in &all {
            if ct.node_id == node_id {
                continue;
            }
            if chain_type_of(ct) == 1 {
                remain_entry += 1;
            }
        }
        if remain_entry == 0 {
            blockers.push(format!("隧道[{}]剔除后将没有入口节点", t.name));
        }
        ctx_map.insert(*tunnel_id, (t, all));
    }

    if !blockers.is_empty() {
        return Err(AppError::new(format!(
            "无法删除节点：{}。请先在隧道编辑中更换入口，或删除相关隧道后再删节点。",
            blockers.join("；")
        )));
    }

    let warnings: Vec<String> = Vec::new();

    for tunnel_id in &tunnel_ids {
        let Some((tunnel, all)) = ctx_map.get(tunnel_id) else {
            continue;
        };

        let mut was_entry = false;
        for ct in all {
            if ct.node_id != node_id {
                continue;
            }
            if chain_type_of(ct) == 1 {
                was_entry = true;
            }
        }

        tracing::info!(
            "删除节点时从隧道剔除 nodeId={} tunnelId={} tunnelName={} entry={}",
            node_id,
            tunnel_id,
            tunnel.name,
            was_entry
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
        let entries: Vec<ChainTunnel> = remaining
            .into_iter()
            .filter(|ct| chain_type_of(ct) == 1)
            .collect();

        // 4) 刷新 in_ip
        refresh_tunnel_in_ip(state, tunnel, &entries).await;
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



