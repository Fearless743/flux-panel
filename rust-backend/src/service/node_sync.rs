use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

use serde_json::json;
use sqlx::{QueryBuilder, Sqlite};

use crate::gost;
use crate::model;
use crate::repo::chain_tunnel as chain_repo;
use crate::repo::forward as forward_repo;
use crate::repo::forward_port as port_repo;
use crate::repo::node as node_repo;
use crate::repo::tunnel as tunnel_repo;
use crate::repo::user_tunnel as user_tunnel_repo;
use crate::service::forward;
use crate::service::speed_limit::convert_bits_to_mbps;
use crate::service::tunnel;
use crate::state::AppState;

type NodeSyncLock = Arc<tokio::sync::Mutex<()>>;

static NODE_SYNC_LOCKS: LazyLock<Mutex<HashMap<i64, NodeSyncLock>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn lock_node_sync(node_id: i64) -> NodeSyncLock {
    let mut locks = NODE_SYNC_LOCKS.lock().unwrap();
    locks
        .entry(node_id)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

#[derive(Default)]
struct SyncRecorder {
    ok: i32,
    fail: i32,
}

impl SyncRecorder {
    fn record(&mut self, node_id: i64, label: &str, msg: &str) {
        if gost::is_ok(msg) {
            self.ok += 1;
            return;
        }
        self.fail += 1;
        tracing::warn!(
            nodeId = node_id,
            item = label,
            msg = msg,
            "节点配置同步项失败"
        );
    }
}

/// 将 DB 中该节点应有配置通过现有增量命令重放到节点。
/// 设计为上线后异步调用；单条失败记日志并继续。
pub async fn sync_node_config(state: &AppState, node_id: i64) {
    if node_id == 0 {
        return;
    }
    let lock = lock_node_sync(node_id);
    let _guard = lock.lock().await;

    if !state.hub.is_node_online(node_id) {
        tracing::info!(nodeId = node_id, "节点配置同步跳过：不在线");
        return;
    }

    let node = match node_repo::find_by_id(&state.db, node_id).await {
        Ok(Some(n)) => n,
        Ok(None) => {
            tracing::warn!(
                nodeId = node_id,
                err = "节点不存在",
                "节点配置同步：加载节点失败"
            );
            return;
        }
        Err(e) => {
            tracing::warn!(
                nodeId = node_id,
                err = %e,
                "节点配置同步：加载节点失败"
            );
            return;
        }
    };

    let links = match chain_repo::list_by_node_id(&state.db, node_id).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                nodeId = node_id,
                err = %e,
                "节点配置同步：查询 chain_tunnel 失败"
            );
            return;
        }
    };

    let mut tunnel_ids: Vec<i64> = Vec::new();
    let mut tunnel_id_set: HashSet<i64> = HashSet::new();
    for ct in &links {
        if tunnel_id_set.insert(ct.tunnel_id) {
            tunnel_ids.push(ct.tunnel_id);
        }
    }

    let mut rec = SyncRecorder::default();

    // 1) Limiters：多限速器下发
    if !tunnel_ids.is_empty() {
        match list_speed_limits_by_tunnel_ids(state, &tunnel_ids).await {
            Ok(limiters) => {
                for sl in &limiters {
                    let msg = ensure_limiter(state, node_id, sl).await;
                    rec.record(node_id, &format!("limiter:{}", sl.id), &msg);
                }
            }
            Err(e) => {
                tracing::warn!(
                    nodeId = node_id,
                    err = %e,
                    "节点配置同步：查询限速失败"
                );
            }
        }
    }

    // 2-3) 按隧道重放 chain + tunnel service
    for tid in &tunnel_ids {
        let tid = *tid;
        let tunnel = match tunnel_repo::find_by_id(&state.db, tid).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    nodeId = node_id,
                    tunnelId = tid,
                    err = %e,
                    "节点配置同步：隧道不存在"
                );
                continue;
            }
        };
        let Some(t) = tunnel else {
            tracing::warn!(nodeId = node_id, tunnelId = tid, "节点配置同步：隧道不存在");
            continue;
        };
        let all = match chain_repo::list_by_tunnel_id(&state.db, tid).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    nodeId = node_id,
                    tunnelId = tid,
                    err = %e,
                    "节点配置同步：加载拓扑失败"
                );
                continue;
            }
        };
        let mut nodes = match load_nodes_for_chains(state, &all).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    nodeId = node_id,
                    tunnelId = tid,
                    err = %e,
                    "节点配置同步：加载节点失败"
                );
                continue;
            }
        };
        // 确保本节点在 map 中
        nodes.insert(node_id, node.clone());

        if t.r#type == 2 {
            sync_tunnel_topology(state, node_id, &all, &nodes, &mut rec).await;
        }
    }

    // 4) Forwards on this node (入口端口)
    match port_repo::list_by_node_id(&state.db, node_id).await {
        Ok(ports) => {
            for fp in &ports {
                let Ok(Some(fw)) = forward_repo::get_by_id(&state.db, fp.forward_id).await else {
                    continue;
                };
                let Ok(Some(tunnel)) = tunnel_repo::find_by_id(&state.db, fw.tunnel_id).await
                else {
                    continue;
                };
                let mut ut: Option<model::UserTunnel> = None;
                let mut limiter: Option<i64> = None;
                if fw.user_id != 0 {
                    if let Ok(Some(u)) = user_tunnel_repo::find_by_user_and_tunnel(
                        &state.db,
                        fw.user_id,
                        fw.tunnel_id,
                    )
                    .await
                    {
                        limiter = u.speed_id;
                        ut = Some(u);
                    }
                }
                let base = forward::build_service_name(fw.id, fw.user_id, ut.as_ref());
                // 空节点无本地 gost.json：Update 不存在时回落 Add
                let mut msg = forward::add_or_update_service(
                    state,
                    &base,
                    limiter,
                    &node,
                    &fw,
                    fp,
                    &tunnel,
                    "UpdateService",
                )
                .await;
                if !gost::is_ok(&msg) {
                    msg = forward::add_or_update_service(
                        state,
                        &base,
                        limiter,
                        &node,
                        &fw,
                        fp,
                        &tunnel,
                        "AddService",
                    )
                    .await;
                }
                // exists 也算成功
                rec.record(node_id, &format!("forward:{}", base), &msg);
                if fw.status == 0 {
                    let res = state
                        .hub
                        .send_msg(node_id, gost::pause_resume_payload(&base), "PauseService")
                        .await;
                    rec.record(
                        node_id,
                        &format!("pause:{}", base),
                        &gost::normalize_ok(&res.msg),
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                nodeId = node_id,
                err = %e,
                "节点配置同步：查询 forward_port 失败"
            );
        }
    }

    tracing::info!(
        nodeId = node_id,
        ok = rec.ok,
        fail = rec.fail,
        "节点配置同步完成"
    );
}

enum TopoTask {
    Chains(Vec<model::ChainTunnel>),
    Service(model::ChainTunnel),
}

async fn sync_tunnel_topology(
    state: &AppState,
    node_id: i64,
    all: &[model::ChainTunnel],
    nodes: &HashMap<i64, model::Node>,
    rec: &mut SyncRecorder,
) {
    let (entries, chain_groups, outs) = tunnel::split_topology(all.to_vec());

    let mut tasks: Vec<(String, TopoTask)> =
        Vec::with_capacity(entries.len() + chain_groups.len() + outs.len());

    // 本节点作为入口：需要 chain 指向下一跳
    for entry in &entries {
        if entry.node_id != node_id {
            continue;
        }
        let target: Vec<model::ChainTunnel> = if chain_groups.is_empty() {
            outs.clone()
        } else {
            chain_groups[0].clone()
        };
        if target.is_empty() {
            continue;
        }
        tasks.push((
            format!("chain:entry:tunnel={}", entry.tunnel_id),
            TopoTask::Chains(target),
        ));
    }

    // 本节点作为转发链
    for i in 0..chain_groups.len() {
        let target: Vec<model::ChainTunnel> = if i + 1 < chain_groups.len() {
            chain_groups[i + 1].clone()
        } else {
            outs.clone()
        };
        for ct in &chain_groups[i] {
            if ct.node_id != node_id {
                continue;
            }
            if !target.is_empty() {
                tasks.push((
                    format!("chain:hop:tunnel={}", ct.tunnel_id),
                    TopoTask::Chains(target.clone()),
                ));
            }
            tasks.push((
                format!("service:tls:tunnel={}", ct.tunnel_id),
                TopoTask::Service(ct.clone()),
            ));
        }
    }

    // 本节点作为出口
    for ct in &outs {
        if ct.node_id != node_id {
            continue;
        }
        tasks.push((
            format!("service:out:tunnel={}", ct.tunnel_id),
            TopoTask::Service(ct.clone()),
        ));
    }

    for (label, task) in tasks {
        let msg = match &task {
            TopoTask::Chains(target) => {
                tunnel::push_chains(state, node_id, target, nodes).await.msg
            }
            TopoTask::Service(ct) => {
                tunnel::push_chain_service(state, node_id, ct, nodes)
                    .await
                    .msg
            }
        };
        rec.record(node_id, &label, &msg);
    }
}

async fn ensure_limiter(state: &AppState, node_id: i64, sl: &model::SpeedLimit) -> String {
    let speed = convert_bits_to_mbps(sl.speed);
    let data = gost::limiter_data(sl.id, &speed);
    let payload = json!({
        "limiter": sl.id.to_string(),
        "data": data,
    });
    let res = state.hub.send_msg(node_id, payload, "UpdateLimiters").await;
    if gost::is_ok(&res.msg) {
        return gost::normalize_ok(&res.msg);
    }
    if res.msg.contains("not found") {
        let res = state.hub.send_msg(node_id, data, "AddLimiters").await;
        return gost::normalize_ok(&res.msg);
    }
    // Update 失败时尝试 Add（空节点常见）
    let res2 = state.hub.send_msg(node_id, data, "AddLimiters").await;
    if gost::is_ok(&res2.msg) {
        return gost::normalize_ok(&res2.msg);
    }
    res.msg
}

// ---- DB 读取 ----

async fn list_speed_limits_by_tunnel_ids(
    state: &AppState,
    tunnel_ids: &[i64],
) -> Result<Vec<model::SpeedLimit>, sqlx::Error> {
    if tunnel_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, name, speed, tunnel_id, tunnel_name, created_time, updated_time, status
         FROM speed_limit WHERE tunnel_id IN (",
    );
    let mut sep = qb.separated(", ");
    for id in tunnel_ids {
        sep.push_bind(id);
    }
    sep.push_unseparated(")");
    qb.build_query_as::<model::SpeedLimit>()
        .fetch_all(&state.db)
        .await
}

async fn load_nodes_for_chains(
    state: &AppState,
    cts: &[model::ChainTunnel],
) -> crate::error::AppResult<HashMap<i64, model::Node>> {
    let mut ids: Vec<i64> = Vec::with_capacity(cts.len());
    let mut seen: HashSet<i64> = HashSet::new();
    for ct in cts {
        if seen.insert(ct.node_id) {
            ids.push(ct.node_id);
        }
    }
    let mut out: HashMap<i64, model::Node> = HashMap::with_capacity(ids.len());
    if ids.is_empty() {
        return Ok(out);
    }
    let list = node_repo::list_by_ids(&state.db, &ids).await?;
    for n in list {
        out.insert(n.id, n);
    }
    Ok(out)
}
