use std::collections::{HashMap, HashSet};
use std::future::Future;

use futures_util::stream::StreamExt;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::gost;
use crate::model;
use crate::repo::forward as repo;
use crate::repo::forward_port as port_repo;
use crate::response::now_ms;
use crate::service::batchutil::{
    dedup_int64s, parallel_each, parallel_each_indexed, BATCH_CONCURRENCY,
};
use crate::state::AppState;

const FORWARD_BYTES_TO_GB: i64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub user_id: i64,
    pub role_id: i32,
    pub user_name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ForwardCreateReq {
    pub name: String,
    pub tunnel_id: i64,
    pub remote_addr: String,
    pub strategy: String,
    pub in_port: Option<i32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ForwardUpdateReq {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub tunnel_id: Option<i64>,
    pub remote_addr: String,
    pub strategy: String,
    pub in_port: Option<i32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BatchForwardReq {
    pub ids: Vec<i64>,
    pub tunnel_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OrderItem {
    pub id: i64,
    pub inx: i32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UpdateOrderReq {
    pub forwards: Vec<OrderItem>,
}

struct UserPermission {
    limiter: Option<i64>,
    user_tunnel: Option<model::UserTunnel>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisResult {
    pub node_id: i64,
    pub node_name: String,
    pub target_ip: String,
    pub target_port: i32,
    pub description: String,
    pub success: bool,
    pub message: String,
    #[serde(serialize_with = "crate::model::serialize_f64_go")]
    pub average_time: f64,
    #[serde(serialize_with = "crate::model::serialize_f64_go")]
    pub packet_loss: f64,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_chain_type: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_inx: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_chain_type: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_inx: Option<i32>,
}

// ---- list ----

pub async fn list(
    state: &AppState,
    user: &CurrentUser,
) -> AppResult<Vec<repo::ForwardWithTunnel>> {
    let mut list = if user.role_id != 0 {
        repo::list_by_user_id(&state.db, user.user_id).await?
    } else {
        repo::list_all(&state.db).await?
    };
    if !list.is_empty() {
        fill_in_ip_batch(state, &mut list).await?;
    }
    Ok(list)
}

/// ForwardListReq 分页列表请求（全部可选；全空时 handler 走 legacy 全量数组返回）。
/// page 从 1 开始；pageSize 默认 20、上限 500（导出循环取页，避免单次大 LIMIT）。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ForwardListReq {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub tunnel_id: Option<i64>,
    pub status: Option<i32>,
    pub keyword: String,
}

/// 分页结果：list 只含当前页（已填充 inIp/inPort），total 为过滤后总数。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardPageResult {
    pub list: Vec<repo::ForwardWithTunnel>,
    pub total: i64,
    pub page: i32,
    pub page_size: i32,
}

impl ForwardListReq {
    /// 是否为分页请求（任一分页/过滤字段出现即按分页语义处理）
    pub fn is_paged(&self) -> bool {
        self.page.is_some()
            || self.page_size.is_some()
            || self.tunnel_id.is_some()
            || self.status.is_some()
            || !self.keyword.trim().is_empty()
    }
}

pub async fn list_paged(
    state: &AppState,
    user: &CurrentUser,
    req: ForwardListReq,
) -> AppResult<ForwardPageResult> {
    let mut page = 1;
    if let Some(p) = req.page {
        if p > 0 {
            page = p;
        }
    }
    let mut page_size = 20;
    if let Some(p) = req.page_size {
        if p > 0 {
            page_size = p;
        }
    }
    if page_size > 500 {
        page_size = 500;
    }

    let mut filter = repo::ForwardListFilter {
        tunnel_id: req.tunnel_id,
        status: req.status,
        keyword: req.keyword.trim().to_string(),
        ..Default::default()
    };
    if user.role_id != 0 {
        filter.user_id = Some(user.user_id);
    }

    let total = repo::count_filtered(&state.db, &filter).await?;
    if total == 0 {
        return Ok(ForwardPageResult {
            list: Vec::new(),
            total: 0,
            page,
            page_size,
        });
    }
    let offset = (page as i64 - 1) * page_size as i64;
    let mut list = repo::list_filtered(&state.db, &filter, page_size as i64, offset).await?;
    if !list.is_empty() {
        fill_in_ip_batch(state, &mut list).await?;
    }
    Ok(ForwardPageResult {
        list,
        total,
        page,
        page_size,
    })
}

/// 批量填充转发列表的 inIp / inPort。
/// 固定 3 条语句（隧道/端口/节点各一次），内存 map 组装。预取失败时回落逐条路径。
async fn fill_in_ip_batch(
    state: &AppState,
    list: &mut [repo::ForwardWithTunnel],
) -> AppResult<()> {
    let mut tunnel_id_set: HashSet<i64> = HashSet::with_capacity(list.len());
    let mut forward_ids: Vec<i64> = Vec::with_capacity(list.len());
    for f in list.iter() {
        tunnel_id_set.insert(f.tunnel_id);
        forward_ids.push(f.id);
    }
    let tunnel_ids: Vec<i64> = tunnel_id_set.into_iter().collect();

    let tunnels = match repo::list_tunnels_by_ids(&state.db, &tunnel_ids).await {
        Ok(v) => v,
        Err(_) => {
            fill_in_ip_legacy(state, list).await;
            return Ok(());
        }
    };
    let ports_map = match port_repo::list_by_forward_ids(&state.db, &forward_ids).await {
        Ok(v) => v,
        Err(_) => {
            fill_in_ip_legacy(state, list).await;
            return Ok(());
        }
    };
    let mut node_id_set: HashSet<i64> = HashSet::new();
    for fps in ports_map.values() {
        for fp in fps {
            node_id_set.insert(fp.node_id);
        }
    }
    let node_ids: Vec<i64> = node_id_set.into_iter().collect();
    let nodes = match repo::list_nodes_by_ids(&state.db, &node_ids).await {
        Ok(v) => v,
        Err(_) => {
            fill_in_ip_legacy(state, list).await;
            return Ok(());
        }
    };

    let tunnel_map: HashMap<i64, model::Tunnel> =
        tunnels.into_iter().map(|t| (t.id, t)).collect();
    let node_map: HashMap<i64, model::Node> = nodes.into_iter().map(|n| (n.id, n)).collect();

    for f in list.iter_mut() {
        fill_in_ip_local(f, &tunnel_map, &ports_map, &node_map);
    }
    Ok(())
}

/// 逐条回填（预取失败时的回落路径，与原 List 行为一致）
async fn fill_in_ip_legacy(state: &AppState, list: &mut [repo::ForwardWithTunnel]) {
    for f in list.iter_mut() {
        fill_in_ip(state, f).await;
    }
}

fn fill_in_ip_local(
    f: &mut repo::ForwardWithTunnel,
    tunnel_map: &HashMap<i64, model::Tunnel>,
    ports_map: &HashMap<i64, Vec<model::ForwardPort>>,
    node_map: &HashMap<i64, model::Node>,
) {
    let Some(tunnel) = tunnel_map.get(&f.tunnel_id) else {
        return;
    };
    let Some(ports) = ports_map.get(&f.id) else {
        return;
    };
    if ports.is_empty() {
        return;
    }

    let use_tunnel_in_ip = tunnel
        .in_ip
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let mut ip_port_set: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    if use_tunnel_in_ip {
        let in_ip = tunnel.in_ip.clone().unwrap_or_default();
        let ip_list: Vec<String> = in_ip
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        let uniq_ip = unique_strings(&ip_list);
        let uniq_ports = unique_ints(&ports_to_ints(ports));
        for ip in &uniq_ip {
            for p in &uniq_ports {
                let key = format!("{}:{}", ip, p);
                if seen.insert(key.clone()) {
                    ip_port_set.push(key);
                }
            }
        }
        if let Some(p) = uniq_ports.first() {
            f.in_port = Some(*p);
        }
    } else {
        for fp in ports {
            let Some(node) = node_map.get(&fp.node_id) else {
                continue;
            };
            if node.server_ip.is_empty() {
                continue;
            }
            let key = format!("{}:{}", node.server_ip, fp.port);
            if seen.insert(key.clone()) {
                ip_port_set.push(key);
            }
        }
        if let Some(fp) = ports.first() {
            f.in_port = Some(fp.port);
        }
    }
    if !ip_port_set.is_empty() {
        f.in_ip = ip_port_set.join(",");
    }
}

async fn fill_in_ip(state: &AppState, f: &mut repo::ForwardWithTunnel) {
    let tunnel = match repo::get_tunnel(&state.db, f.tunnel_id).await {
        Ok(Some(t)) => t,
        _ => return,
    };
    let ports = match port_repo::list_by_forward_id(&state.db, f.id).await {
        Ok(v) => v,
        _ => return,
    };
    if ports.is_empty() {
        return;
    }

    let use_tunnel_in_ip = tunnel
        .in_ip
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let mut ip_port_set: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    if use_tunnel_in_ip {
        let in_ip = tunnel.in_ip.clone().unwrap_or_default();
        let ip_list: Vec<String> = in_ip
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        let uniq_ip = unique_strings(&ip_list);
        let uniq_ports = unique_ints(&ports_to_ints(&ports));
        for ip in &uniq_ip {
            for p in &uniq_ports {
                let key = format!("{}:{}", ip, p);
                if seen.insert(key.clone()) {
                    ip_port_set.push(key);
                }
            }
        }
        if let Some(p) = uniq_ports.first() {
            f.in_port = Some(*p);
        }
    } else {
        for fp in &ports {
            let node = match repo::get_node(&state.db, fp.node_id).await {
                Ok(Some(n)) if !n.server_ip.is_empty() => n,
                _ => continue,
            };
            let key = format!("{}:{}", node.server_ip, fp.port);
            if seen.insert(key.clone()) {
                ip_port_set.push(key);
            }
        }
        if let Some(fp) = ports.first() {
            f.in_port = Some(fp.port);
        }
    }
    if !ip_port_set.is_empty() {
        f.in_ip = ip_port_set.join(",");
    }
}

// ---- create ----

pub async fn create(
    state: &AppState,
    user: CurrentUser,
    req: ForwardCreateReq,
) -> AppResult<()> {
    if req.name.trim().is_empty() {
        return Err(AppError::new("转发名称不能为空"));
    }
    if req.name.chars().count() > 64 {
        return Err(AppError::new("转发名称过长"));
    }
    if req.tunnel_id == 0 {
        return Err(AppError::new("隧道ID不能为空"));
    }
    validate_remote_addr(&req.remote_addr)?;

    let Some(tunnel) = repo::get_tunnel(&state.db, req.tunnel_id).await? else {
        return Err(AppError::new("隧道不存在"));
    };
    if tunnel.status != 1 {
        return Err(AppError::new("隧道已禁用，无法创建转发"));
    }

    let perm = check_user_permissions(state, &user, &tunnel, None).await?;

    let now = now_ms();
    let mut forward = model::Forward {
        user_id: user.user_id,
        user_name: user.user_name.clone(),
        name: req.name.clone(),
        tunnel_id: req.tunnel_id,
        remote_addr: req.remote_addr.clone(),
        strategy: req.strategy.clone(),
        status: 1,
        created_time: now,
        updated_time: now,
        inx: 0,
        ..Default::default()
    };

    let mut chain_tunnels = repo::list_chain_tunnels_by_tunnel(&state.db, tunnel.id, "1").await?;
    if chain_tunnels.is_empty() {
        return Err(AppError::new("隧道没有入口节点"));
    }
    chain_tunnels = get_port(state, chain_tunnels, req.in_port, 0).await?;

    let id = repo::insert(&state.db, &forward).await?;
    forward.id = id;

    let mut success: Vec<(i64, String)> = Vec::new();
    type CreateTask = (model::Node, model::ForwardPort, String);
    let mut tasks: Vec<CreateTask> = Vec::with_capacity(chain_tunnels.len());
    for ct in &chain_tunnels {
        let mut fp = model::ForwardPort {
            forward_id: forward.id,
            node_id: ct.node_id,
            port: deref_int(ct.port),
            ..Default::default()
        };
        match port_repo::insert(&state.db, &fp).await {
            Ok(fpid) => fp.id = fpid,
            Err(e) => {
                rollback_create(state, forward.id, &success).await;
                return Err(e);
            }
        }
        let service_name = build_service_name(forward.id, forward.user_id, perm.user_tunnel.as_ref());
        let node = match repo::get_node(&state.db, ct.node_id).await {
            Ok(Some(n)) => n,
            Ok(None) => {
                rollback_create(state, forward.id, &success).await;
                return Err(AppError::new("部分节点不存在"));
            }
            Err(e) => {
                rollback_create(state, forward.id, &success).await;
                return Err(e);
            }
        };
        tasks.push((node, fp, service_name));
    }

    let state_ref = state;
    let forward_ref = &forward;
    let tunnel_ref = &tunnel;
    let tasks_ref = &tasks;
    let limiter = perm.limiter;
    let errs = parallel_each_indexed(tasks.len(), BATCH_CONCURRENCY, move |i| {
        let (node, fp, name) = &tasks_ref[i];
        async move {
            let msg = add_or_update_service(
                state_ref,
                name,
                limiter,
                node,
                forward_ref,
                fp,
                tunnel_ref,
                "AddService",
            )
            .await;
            if !gost::is_ok(&msg) {
                gost::normalize_ok(&msg)
            } else {
                String::new()
            }
        }
    })
    .await;

    let failed: HashSet<usize> = errs.iter().map(|(i, _)| *i).collect();
    for (i, (node, _, name)) in tasks.iter().enumerate() {
        if !failed.contains(&i) {
            success.push((node.id, name.clone()));
        }
    }
    if let Some((_, msg)) = errs.first() {
        rollback_create(state, forward.id, &success).await;
        return Err(AppError::new(msg.clone()));
    }
    Ok(())
}

async fn rollback_create(state: &AppState, forward_id: i64, items: &[(i64, String)]) {
    let _ = repo::delete(&state.db, forward_id).await;
    let _ = port_repo::delete_by_forward_id(&state.db, forward_id).await;
    let state_ref = state;
    parallel_each(items.len(), 8, move |i| {
        let (node_id, name) = &items[i];
        let node_id = *node_id;
        let names = vec![format!("{}_tcp", name), format!("{}_udp", name)];
        async move {
            let res = state_ref
                .hub
                .send_msg(node_id, gost::delete_service_payload(names), "DeleteService")
                .await;
            let _ = gost::normalize_ok(&res.msg);
            String::new()
        }
    })
    .await;
}

// ---- update ----

pub async fn update(
    state: &AppState,
    user: CurrentUser,
    req: ForwardUpdateReq,
) -> AppResult<()> {
    if req.id == 0 {
        return Err(AppError::new("ID不能为空"));
    }
    if req.name.trim().is_empty() {
        return Err(AppError::new("转发名称不能为空"));
    }
    if req.name.chars().count() > 64 {
        return Err(AppError::new("转发名称过长"));
    }
    validate_remote_addr(&req.remote_addr)?;

    let Some(mut exist) = validate_forward_exists(state, req.id, &user).await? else {
        return Err(AppError::new("转发不存在"));
    };

    let Some(tunnel) = repo::get_tunnel(&state.db, exist.tunnel_id).await? else {
        return Err(AppError::new("隧道不存在"));
    };

    // 换隧道
    if let Some(new_tid) = req.tunnel_id {
        if new_tid != exist.tunnel_id {
            return change_forward_tunnel(state, &user, exist, tunnel, req).await;
        }
    }

    let perm = check_user_permissions(state, &user, &tunnel, None).await?;

    let user_tunnel = if user.role_id != 0 {
        match repo::get_user_tunnel(&state.db, user.user_id, exist.tunnel_id).await? {
            Some(ut) => Some(ut),
            None => return Err(AppError::new("你没有该隧道权限")),
        }
    } else {
        repo::get_user_tunnel(&state.db, exist.user_id, exist.tunnel_id)
            .await
            .unwrap_or(None)
    };

    exist.remote_addr = req.remote_addr.clone();
    exist.name = req.name.clone();
    exist.strategy = req.strategy.clone();
    exist.status = 1;
    exist.updated_time = now_ms();
    repo::update(&state.db, &exist).await?;

    let mut chain_tunnels =
        repo::list_chain_tunnels_by_tunnel(&state.db, tunnel.id, "1").await?;
    chain_tunnels = get_port(state, chain_tunnels, req.in_port, exist.id).await?;

    type UpdTask = (model::Node, model::ForwardPort, String);
    let mut tasks: Vec<UpdTask> = Vec::with_capacity(chain_tunnels.len());
    for ct in &chain_tunnels {
        let service_name = build_service_name(exist.id, exist.user_id, user_tunnel.as_ref());
        let Some(node) = repo::get_node(&state.db, ct.node_id).await? else {
            return Err(AppError::new("部分节点不存在"));
        };
        let Some(mut fp) = port_repo::get_by_forward_and_node(&state.db, exist.id, node.id).await?
        else {
            return Err(AppError::new("部分节点不存在1"));
        };
        fp.port = deref_int(ct.port);
        port_repo::update_port(&state.db, fp.id, fp.port).await?;
        tasks.push((node, fp, service_name));
    }

    let state_ref = state;
    let exist_ref = &exist;
    let tunnel_ref = &tunnel;
    let tasks_ref = &tasks;
    let limiter = perm.limiter;
    let errs = parallel_each_indexed(tasks.len(), BATCH_CONCURRENCY, move |i| {
        let (node, fp, name) = &tasks_ref[i];
        async move {
            let msg = add_or_update_service(
                state_ref,
                name,
                limiter,
                node,
                exist_ref,
                fp,
                tunnel_ref,
                "UpdateService",
            )
            .await;
            if !gost::is_ok(&msg) {
                gost::normalize_ok(&msg)
            } else {
                String::new()
            }
        }
    })
    .await;
    if let Some((_, msg)) = errs.first() {
        return Err(AppError::new(msg.clone()));
    }
    Ok(())
}

// ---- change tunnel ----

async fn change_forward_tunnel(
    state: &AppState,
    user: &CurrentUser,
    exist: model::Forward,
    old_tunnel: model::Tunnel,
    req: ForwardUpdateReq,
) -> AppResult<()> {
    let Some(new_tunnel) = repo::get_tunnel(&state.db, req.tunnel_id.unwrap_or(0)).await? else {
        return Err(AppError::new("目标隧道不存在"));
    };
    if new_tunnel.status != 1 {
        return Err(AppError::new("目标隧道已禁用"));
    }

    check_user_permissions(state, user, &new_tunnel, Some(exist.id)).await?;

    let new_user_tunnel = repo::get_user_tunnel(&state.db, exist.user_id, new_tunnel.id)
        .await
        .unwrap_or(None);
    let owner = repo::get_user(&state.db, exist.user_id).await.unwrap_or(None);
    if let Some(owner) = &owner {
        if owner.role_id != 0 && new_user_tunnel.is_none() {
            return Err(AppError::new("该转发所属用户没有目标隧道权限"));
        }
    }

    let mut new_entry_nodes =
        repo::list_chain_tunnels_by_tunnel(&state.db, new_tunnel.id, "1").await?;
    if new_entry_nodes.is_empty() {
        return Err(AppError::new("目标隧道没有入口节点"));
    }

    // 换隧道时保持原入口端口：优先请求指定，否则取原 forward_port
    let in_port = resolve_keep_in_port(state, req.in_port, exist.id).await?;
    new_entry_nodes = get_port(state, new_entry_nodes, in_port, exist.id).await?;

    // 删旧服务（多入口并行）
    let old_user_tunnel = repo::get_user_tunnel(&state.db, exist.user_id, old_tunnel.id)
        .await
        .unwrap_or(None);
    let old_service_name = build_service_name(exist.id, exist.user_id, old_user_tunnel.as_ref());
    let old_entry_nodes = repo::list_chain_tunnels_by_tunnel(&state.db, old_tunnel.id, "1")
        .await
        .unwrap_or_default();
    let names = vec![
        format!("{}_tcp", old_service_name),
        format!("{}_udp", old_service_name),
    ];
    let _ = for_each_chain_node_parallel(state, &old_entry_nodes, move |node| {
        let names = names.clone();
        async move {
            let res = state
                .hub
                .send_msg(node.id, gost::delete_service_payload(names), "DeleteService")
                .await;
            let _ = gost::normalize_ok(&res.msg);
            Ok(())
        }
    })
    .await;
    let _ = port_repo::delete_by_forward_id(&state.db, exist.id).await;

    // 更新转发
    let mut exist = exist;
    exist.tunnel_id = req.tunnel_id.unwrap_or(0);
    exist.name = req.name.clone();
    exist.remote_addr = req.remote_addr.clone();
    exist.strategy = req.strategy.clone();
    exist.status = 1;
    exist.updated_time = now_ms();
    repo::update(&state.db, &exist).await?;

    // 新服务：先串行写库，再并行下发 AddService
    let new_service_name = build_service_name(exist.id, exist.user_id, new_user_tunnel.as_ref());
    let limiter = new_user_tunnel.as_ref().and_then(|ut| ut.speed_id);

    type EntryWork = (model::Node, model::ForwardPort);
    let mut works: Vec<EntryWork> = Vec::with_capacity(new_entry_nodes.len());
    for ct in &new_entry_nodes {
        let Some(node) = repo::get_node(&state.db, ct.node_id).await? else {
            return Err(AppError::new("目标隧道部分入口节点不存在"));
        };
        let mut fp = model::ForwardPort {
            forward_id: exist.id,
            node_id: ct.node_id,
            port: deref_int(ct.port),
            ..Default::default()
        };
        let fpid = port_repo::insert(&state.db, &fp).await?;
        fp.id = fpid;
        works.push((node, fp));
    }

    if works.len() == 1 {
        let (node, fp) = &works[0];
        let msg = add_or_update_service(
            state,
            &new_service_name,
            limiter,
            node,
            &exist,
            fp,
            &new_tunnel,
            "AddService",
        )
        .await;
        if !gost::is_ok(&msg) {
            return Err(AppError::new(format!(
                "节点[{}]服务创建失败: {}",
                node.name,
                gost::normalize_ok(&msg)
            )));
        }
        return Ok(());
    }

    let state_ref = state;
    let exist_ref = &exist;
    let tunnel_ref = &new_tunnel;
    let name_ref = &new_service_name;
    let works_ref = &works;
    let errs = parallel_each_indexed(works.len(), BATCH_CONCURRENCY, move |i| {
        let (node, fp) = &works_ref[i];
        async move {
            let msg = add_or_update_service(
                state_ref,
                name_ref,
                limiter,
                node,
                exist_ref,
                fp,
                tunnel_ref,
                "AddService",
            )
            .await;
            if !gost::is_ok(&msg) {
                format!(
                    "节点[{}]服务创建失败: {}",
                    node.name,
                    gost::normalize_ok(&msg)
                )
            } else {
                String::new()
            }
        }
    })
    .await;
    if let Some((_, msg)) = errs.first() {
        return Err(AppError::new(msg.clone()));
    }
    Ok(())
}

// ---- delete ----

pub async fn delete(state: &AppState, user: CurrentUser, id: i64) -> AppResult<()> {
    let Some(forward) = validate_forward_exists(state, id, &user).await? else {
        return Err(AppError::new("转发不存在"));
    };
    let Some(tunnel) = repo::get_tunnel(&state.db, forward.tunnel_id).await? else {
        return Err(AppError::new("隧道不存在"));
    };

    check_user_permissions(state, &user, &tunnel, None).await?;

    let user_tunnel: Option<model::UserTunnel> = if user.role_id != 0 {
        match repo::get_user_tunnel(&state.db, user.user_id, forward.tunnel_id)
            .await
            .unwrap_or(None)
        {
            Some(ut) => Some(ut),
            None => return Err(AppError::new("你没有该隧道权限")),
        }
    } else {
        repo::get_user_tunnel(&state.db, forward.user_id, forward.tunnel_id)
            .await
            .unwrap_or(None)
    };

    let chain_tunnels =
        repo::list_chain_tunnels_by_tunnel(&state.db, tunnel.id, "1").await?;
    let service_name = build_service_name(forward.id, forward.user_id, user_tunnel.as_ref());
    let names = vec![
        format!("{}_tcp", service_name),
        format!("{}_udp", service_name),
    ];
    // 多入口节点并行下发删除（单条失败不阻塞其余）
    let _ = for_each_chain_node_parallel(state, &chain_tunnels, move |node| {
        let names = names.clone();
        async move {
            let res = state
                .hub
                .send_msg(node.id, gost::delete_service_payload(names), "DeleteService")
                .await;
            let _ = gost::normalize_ok(&res.msg);
            Ok(())
        }
    })
    .await;
    let _ = port_repo::delete_by_forward_id(&state.db, id).await;
    repo::delete(&state.db, id).await
}

pub async fn force_delete(state: &AppState, user: CurrentUser, id: i64) -> AppResult<()> {
    let Some(forward) = validate_forward_exists(state, id, &user).await? else {
        return Err(AppError::new("端口转发不存在"));
    };
    // force-delete 同样尽力清理节点侧残留服务，避免孤儿端口继续可连
    if let Ok(Some(tunnel)) = repo::get_tunnel(&state.db, forward.tunnel_id).await {
        if let Ok(user_tunnel) =
            repo::get_user_tunnel(&state.db, forward.user_id, forward.tunnel_id).await
        {
            let service_name =
                build_service_name(forward.id, forward.user_id, user_tunnel.as_ref());
            let names = vec![
                format!("{}_tcp", service_name),
                format!("{}_udp", service_name),
            ];
            if let Ok(chain_tunnels) =
                repo::list_chain_tunnels_by_tunnel(&state.db, tunnel.id, "1").await
            {
                let _ = for_each_chain_node_parallel(state, &chain_tunnels, move |node| {
                    let names = names.clone();
                    async move {
                        let res = state
                            .hub
                            .send_msg(node.id, gost::delete_service_payload(names), "DeleteService")
                            .await;
                        let _ = gost::normalize_ok(&res.msg);
                        Ok(())
                    }
                })
                .await;
            }
        }
    }
    repo::delete(&state.db, id).await?;
    let _ = port_repo::delete_by_forward_id(&state.db, id).await;
    Ok(())
}

// ---- pause / resume ----

pub async fn pause(state: &AppState, user: CurrentUser, id: i64) -> AppResult<()> {
    change_forward_status(state, user, id, 0, "PauseService").await
}

pub async fn resume(state: &AppState, user: CurrentUser, id: i64) -> AppResult<()> {
    change_forward_status(state, user, id, 1, "ResumeService").await
}

async fn change_forward_status(
    state: &AppState,
    user: CurrentUser,
    id: i64,
    target_status: i32,
    gost_method: &str,
) -> AppResult<()> {
    if user.role_id != 0 {
        let Some(u) = repo::get_user(&state.db, user.user_id).await? else {
            return Err(AppError::new("用户不存在"));
        };
        if u.status == 0 {
            return Err(AppError::new("用户已到期或被禁用"));
        }
    }

    let Some(forward) = validate_forward_exists(state, id, &user).await? else {
        return Err(AppError::new("转发不存在"));
    };
    let Some(tunnel) = repo::get_tunnel(&state.db, forward.tunnel_id).await? else {
        return Err(AppError::new("隧道不存在"));
    };

    let mut user_tunnel: Option<model::UserTunnel> = None;
    if target_status == 1 {
        if tunnel.status != 1 {
            return Err(AppError::new("隧道已禁用，无法恢复服务"));
        }
        if user.role_id != 0 {
            check_user_flow_limits(state, user.user_id, &tunnel).await?;
            match repo::get_user_tunnel(&state.db, user.user_id, forward.tunnel_id).await? {
                Some(ut) => user_tunnel = Some(ut),
                None => return Err(AppError::new("你没有该隧道权限")),
            }
            if let Some(ut) = &user_tunnel {
                if ut.status != 1 {
                    return Err(AppError::new("隧道被禁用"));
                }
            }
        }
    }
    if user.role_id != 0 && user_tunnel.is_none() {
        match repo::get_user_tunnel(&state.db, user.user_id, forward.tunnel_id).await? {
            Some(ut) => user_tunnel = Some(ut),
            None => return Err(AppError::new("你没有该隧道权限")),
        }
    }
    if user_tunnel.is_none() {
        user_tunnel = repo::get_user_tunnel(&state.db, forward.user_id, forward.tunnel_id)
            .await
            .unwrap_or(None);
    }

    let chain_tunnels =
        repo::list_chain_tunnels_by_tunnel(&state.db, tunnel.id, "1").await?;
    let service_name = build_service_name(forward.id, forward.user_id, user_tunnel.as_ref());
    let limiter = user_tunnel.as_ref().and_then(|ut| ut.speed_id);

    let method = gost_method.to_string();
    let forward_ref = &forward;
    let tunnel_ref = &tunnel;
    for_each_chain_node_parallel(state, &chain_tunnels, move |node| {
        let service_name = service_name.clone();
        let method = method.clone();
        async move {
            let res = state
                .hub
                .send_msg(
                    node.id,
                    gost::pause_resume_payload(&service_name),
                    &method,
                )
                .await;
            if gost::is_ok(&res.msg) {
                return Ok(());
            }
            // 节点上服务不存在：暂停视为已达成；恢复则重建服务（Update 路径同理回落 Add）
            if gost::is_missing(&res.msg) {
                if target_status == 0 {
                    return Ok(());
                }
                let Some(fp) =
                    port_repo::get_by_forward_and_node(&state.db, forward_ref.id, node.id).await?
                else {
                    return Err(AppError::new(format!(
                        "节点 {} 缺少端口记录，无法重建服务",
                        node.name
                    )));
                };
                let msg = add_or_update_service(
                    state,
                    &service_name,
                    limiter,
                    &node,
                    forward_ref,
                    &fp,
                    tunnel_ref,
                    "AddService",
                )
                .await;
                if !gost::is_ok(&msg) {
                    return Err(AppError::new(format!(
                        "节点 {} 重建服务失败: {}",
                        node.name, msg
                    )));
                }
                return Ok(());
            }
            Err(AppError::new(gost::normalize_ok(&res.msg)))
        }
    })
    .await?;

    repo::update_status(&state.db, forward.id, target_status, now_ms()).await
}

// ---- diagnose ----

pub async fn diagnose(
    state: &AppState,
    user: CurrentUser,
    id: i64,
) -> AppResult<Value> {
    if !crate::ratelimit::check_diagnose(user.user_id) {
        return Err(AppError::new("诊断请求过于频繁，请稍后再试"));
    }
    let Some(forward) = validate_forward_exists(state, id, &user).await? else {
        return Err(AppError::new("转发不存在"));
    };
    let Some(tunnel) = repo::get_tunnel(&state.db, forward.tunnel_id).await? else {
        return Err(AppError::new("隧道不存在"));
    };
    // 诊断同样要求隧道使用权限，防止用他人转发/过期权限做端口扫描
    check_user_permissions(state, &user, &tunnel, None).await?;

    let chain_tunnels = repo::list_chain_tunnels_by_tunnel(&state.db, tunnel.id, "").await?;
    if chain_tunnels.is_empty() {
        return Err(AppError::new("隧道配置不完整"));
    }

    let mut in_nodes: Vec<model::ChainTunnel> = Vec::new();
    let mut out_nodes: Vec<model::ChainTunnel> = Vec::new();
    for ct in chain_tunnels {
        match ct.chain_type.as_str() {
            "1" => in_nodes.push(ct),
            "3" => out_nodes.push(ct),
            _ => {}
        }
    }

    // 诊断涉及的节点一次查回
    let mut node_id_set: HashSet<i64> = HashSet::new();
    for ct in &in_nodes {
        node_id_set.insert(ct.node_id);
    }
    for ct in &out_nodes {
        node_id_set.insert(ct.node_id);
    }
    let node_ids: Vec<i64> = node_id_set.into_iter().collect();
    let diag_nodes = repo::list_nodes_by_ids(&state.db, &node_ids)
        .await
        .unwrap_or_default();
    let node_map: HashMap<i64, model::Node> =
        diag_nodes.into_iter().map(|n| (n.id, n)).collect();

    struct PingTask {
        node: model::Node,
        target_ip: String,
        target_port: i32,
        description: String,
        from_chain_type: Option<i32>,
        from_inx: Option<i32>,
        to_chain_type: Option<i32>,
        to_inx: Option<i32>,
    }
    fn push_task(
        tasks: &mut Vec<PingTask>,
        node: &model::Node,
        ip: String,
        port: i32,
        desc: String,
        ft: Option<i32>,
        fi: Option<i32>,
        tt: Option<i32>,
        ti: Option<i32>,
    ) {
        tasks.push(PingTask {
            node: node.clone(),
            target_ip: ip,
            target_port: port,
            description: desc,
            from_chain_type: ft,
            from_inx: fi,
            to_chain_type: tt,
            to_inx: ti,
        });
    }

    let mut tasks: Vec<PingTask> = Vec::new();
    // 单次诊断最多测 5 个目标，防 remote_addr 被塞入大量地址做批量扫描
    let remote_addresses: Vec<String> = forward
        .remote_addr
        .split(',')
        .map(|s| s.to_string())
        .take(5)
        .collect();

    if tunnel.r#type == 1 {
        for in_node in &in_nodes {
            let Some(node) = node_map.get(&in_node.node_id) else {
                continue;
            };
            for ra in &remote_addresses {
                let ra = ra.trim();
                let target_ip = extract_ip_from_address(ra);
                let target_port = extract_port_from_address(ra);
                if !target_ip.is_empty() && target_port != -1 {
                    push_task(
                        &mut tasks,
                        node,
                        target_ip,
                        target_port,
                        format!("入口({})->目标({})", node.name, ra),
                        Some(1),
                        None,
                        None,
                        None,
                    );
                }
            }
        }
    } else {
        // type2 直连：入口→出口、出口→目标
        for in_node in &in_nodes {
            let Some(from_node) = node_map.get(&in_node.node_id) else {
                continue;
            };
            for out in &out_nodes {
                let Some(to_node) = node_map.get(&out.node_id) else {
                    continue;
                };
                push_task(
                    &mut tasks,
                    from_node,
                    to_node.server_ip.clone(),
                    deref_int(out.port),
                    format!("入口({})->出口({})", from_node.name, to_node.name),
                    Some(1),
                    None,
                    Some(3),
                    None,
                );
            }
        }
        for out in &out_nodes {
            let Some(node) = node_map.get(&out.node_id) else {
                continue;
            };
            for ra in &remote_addresses {
                let ra = ra.trim();
                let target_ip = extract_ip_from_address(ra);
                let target_port = extract_port_from_address(ra);
                if !target_ip.is_empty() && target_port != -1 {
                    push_task(
                        &mut tasks,
                        node,
                        target_ip,
                        target_port,
                        format!("出口({})->目标({})", node.name, ra),
                        Some(3),
                        None,
                        None,
                        None,
                    );
                }
            }
        }
    }

    let mut results: Vec<DiagnosisResult> = Vec::new();
    if !tasks.is_empty() {
        let owned: Vec<(usize, model::Node, String, i32, String)> = tasks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                (
                    i,
                    t.node.clone(),
                    t.target_ip.clone(),
                    t.target_port,
                    t.description.clone(),
                )
            })
            .collect();
        let mut pinged: Vec<(usize, DiagnosisResult)> =
            futures_util::stream::iter(owned)
                .map(|(i, node, target_ip, target_port, description)| async move {
                    let r = perform_tcp_ping(
                        state,
                        &node,
                        &target_ip,
                        target_port,
                        &description,
                    )
                    .await;
                    (i, r)
                })
                .buffer_unordered(8)
                .collect()
                .await;
        pinged.sort_by_key(|(i, _)| *i);
        for (i, mut r) in pinged {
            let t = &tasks[i];
            r.from_chain_type = t.from_chain_type;
            r.from_inx = t.from_inx;
            r.to_chain_type = t.to_chain_type;
            r.to_inx = t.to_inx;
            results.push(r);
        }
    }

    let tunnel_type_name = if tunnel.r#type == 2 { "隧道转发" } else { "端口转发" };
    Ok(json!({
        "forwardId": id,
        "forwardName": forward.name,
        "tunnelType": tunnel_type_name,
        "results": results,
        "timestamp": now_ms(),
    }))
}

pub async fn perform_tcp_ping(
    state: &AppState,
    node: &model::Node,
    target_ip: &str,
    port: i32,
    description: &str,
) -> DiagnosisResult {
    let mut result = DiagnosisResult {
        node_id: node.id,
        node_name: node.name.clone(),
        target_ip: target_ip.to_string(),
        target_port: port,
        description: description.to_string(),
        timestamp: now_ms(),
        ..Default::default()
    };
    let payload = json!({
        "ip": target_ip,
        "port": port,
        "count": 4,
        "timeout": 5000,
    });
    let res = state.hub.send_msg(node.id, payload, "TcpPing").await;
    if gost::is_ok(&res.msg) {
        if let Some(Value::Object(data)) = res.data {
            let success = data
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            result.success = success;
            if success {
                result.message = "TCP连接成功".to_string();
                result.average_time = to_float(data.get("averageTime"));
                result.packet_loss = to_float(data.get("packetLoss"));
            } else {
                if let Some(m) = data.get("errorMessage").and_then(Value::as_str) {
                    result.message = m.to_string();
                } else {
                    result.message = "连接失败".to_string();
                }
                result.average_time = -1.0;
                result.packet_loss = 100.0;
            }
            return result;
        }
        result.success = true;
        result.message = "TCP连接成功".to_string();
        result.average_time = 0.0;
        result.packet_loss = 0.0;
        return result;
    }
    result.success = false;
    if !res.msg.is_empty() {
        result.message = res.msg;
    } else {
        result.message = "节点无响应".to_string();
    }
    result.average_time = -1.0;
    result.packet_loss = 100.0;
    result
}

// ---- order ----

pub async fn update_order(
    state: &AppState,
    user: CurrentUser,
    items: Vec<OrderItem>,
) -> AppResult<()> {
    if items.is_empty() {
        return Err(AppError::new("forwards参数不能为空"));
    }
    if items.len() > 500 {
        return Err(AppError::new("单次排序过多"));
    }
    if user.role_id != 0 {
        let ids: Vec<i64> = items.iter().map(|it| it.id).collect();
        let n = repo::count_by_ids_and_user(&state.db, &ids, user.user_id).await?;
        if n != ids.len() as i64 {
            return Err(AppError::new("只能更新自己的转发排序"));
        }
    }
    let mut inx_by_id: HashMap<i64, i32> = HashMap::with_capacity(items.len());
    for it in items {
        inx_by_id.insert(it.id, it.inx);
    }
    repo::update_inx_batch(&state.db, &inx_by_id).await
}

// ---- batch ----

/// 归属权限映射键："userID:tunnelID"
fn ut_key(user_id: i64, tunnel_id: i64) -> String {
    format!("{}:{}", user_id, tunnel_id)
}

/// 一次查询取回 ids 对应的转发并按权限过滤（替代逐条 get_by_id）。
async fn batch_load_forwards(
    state: &AppState,
    user: &CurrentUser,
    ids: &[i64],
) -> AppResult<HashMap<i64, model::Forward>> {
    let list = repo::list_by_ids(&state.db, &dedup_int64s(ids)).await?;
    let mut out: HashMap<i64, model::Forward> = HashMap::with_capacity(list.len());
    for f in list {
        if user.role_id != 0 && f.user_id != user.user_id {
            continue;
        }
        out.insert(f.id, f);
    }
    Ok(out)
}

async fn batch_tunnel_map(
    state: &AppState,
    tunnel_ids: &[i64],
) -> AppResult<HashMap<i64, model::Tunnel>> {
    let list = repo::list_tunnels_by_ids(&state.db, tunnel_ids).await?;
    Ok(list.into_iter().map(|t| (t.id, t)).collect())
}

/// 按顺序收集目标转发涉及的去重隧道 ID
fn distinct_target_tunnel_ids(targets: &[model::Forward]) -> Vec<i64> {
    let mut seen: HashSet<i64> = HashSet::with_capacity(targets.len());
    let mut out: Vec<i64> = Vec::with_capacity(targets.len());
    for f in targets {
        if seen.insert(f.tunnel_id) {
            out.push(f.tunnel_id);
        }
    }
    out
}

/// 按输入顺序拼装批量结果文案（与原 runBatch 格式一致）
fn assemble_batch_msg(
    ok_verb: &str,
    ids: &[i64],
    err_by_id: &HashMap<i64, String>,
    success: usize,
) -> String {
    let mut errs: Vec<String> = Vec::new();
    for id in ids {
        if let Some(msg) = err_by_id.get(id) {
            errs.push(format!("ID-{}: {}", id, msg));
        }
    }
    let mut msg = format!("成功{}{}个转发", ok_verb, success);
    if !errs.is_empty() {
        msg.push_str(&format!("，失败{}个: {}", errs.len(), errs.join("; ")));
    }
    msg
}

/// 在预取的端口列表中找某节点的记录
fn find_forward_port<'a>(
    fps: Option<&'a [model::ForwardPort]>,
    node_id: i64,
) -> Option<&'a model::ForwardPort> {
    fps.and_then(|fps| fps.iter().find(|fp| fp.node_id == node_id))
}

/// 对 ids 有限并发执行 fn，汇总成功/失败文案（与原先串行语义一致）。
async fn run_batch<F, Fut>(
    ids: &[i64],
    empty_msg: &str,
    ok_verb: &str,
    f: F,
) -> (String, bool)
where
    F: Fn(i64) -> Fut,
    Fut: Future<Output = AppResult<()>>,
{
    if ids.is_empty() {
        return (empty_msg.to_string(), false);
    }
    let mut workers = BATCH_CONCURRENCY;
    if workers > ids.len() {
        workers = ids.len();
    }
    let results = parallel_each_indexed(ids.len(), workers, |i| {
        let id = ids[i];
        let fut = f(id);
        async move {
            match fut.await {
                Ok(()) => String::new(),
                Err(e) => e.0,
            }
        }
    })
    .await;

    let mut err_by_id: HashMap<i64, String> = HashMap::with_capacity(ids.len());
    for (i, msg) in &results {
        err_by_id.insert(ids[*i], msg.clone());
    }
    let success = ids.len() - results.len();
    let mut errs: Vec<String> = Vec::new();
    for id in ids {
        if let Some(msg) = err_by_id.get(id) {
            errs.push(format!("ID-{}: {}", id, msg));
        }
    }
    let mut msg = format!("成功{}{}个转发", ok_verb, success);
    if !errs.is_empty() {
        msg.push_str(&format!("，失败{}个: {}", errs.len(), errs.join("; ")));
    }
    (msg, success > 0)
}

pub async fn batch_delete(
    state: &AppState,
    user: CurrentUser,
    ids: Vec<i64>,
) -> (String, bool) {
    if ids.is_empty() {
        return ("请选择要删除的转发".to_string(), false);
    }
    if ids.len() > 200 {
        return ("单次批量操作过多（最多200个）".to_string(), false);
    }
    if let Some(res) = batch_delete_fast(state, &user, &ids).await {
        return res;
    }
    run_batch(&ids, "请选择要删除的转发", "删除", |id| {
        let state = state.clone();
        let user = user.clone();
        async move { delete(&state, user, id).await }
    })
    .await
}

/// 批量删除快路径：一次预取校验 + 按节点合并下发 + 两条语句删库。
/// None 表示预取阶段 DB 出错，调用方回落逐条路径以保留原错误语义。
async fn batch_delete_fast(
    state: &AppState,
    user: &CurrentUser,
    ids: &[i64],
) -> Option<(String, bool)> {
    let uniq = dedup_int64s(ids);
    let forwards = match batch_load_forwards(state, user, &uniq).await {
        Ok(v) => v,
        Err(_) => return None,
    };
    let mut err_by_id: HashMap<i64, String> = HashMap::with_capacity(uniq.len());
    let mut targets: Vec<model::Forward> = Vec::new();
    for id in &uniq {
        match forwards.get(id) {
            Some(f) => targets.push(f.clone()),
            None => {
                err_by_id.insert(*id, "转发不存在".to_string());
            }
        }
    }
    if targets.is_empty() {
        return Some((assemble_batch_msg("删除", &uniq, &err_by_id, 0), false));
    }

    let now = now_ms();
    // 操作者信息只查一次
    let mut op_user: Option<model::User> = None;
    let mut op_user_err = String::new();
    if user.role_id != 0 {
        match repo::get_user(&state.db, user.user_id).await {
            Ok(u) => match u {
                Some(u) => {
                    if u.exp_time > 0 && u.exp_time <= now {
                        op_user_err = "当前账号已到期".to_string();
                    }
                    op_user = Some(u);
                }
                None => op_user_err = "用户不存在".to_string(),
            },
            Err(_) => return None,
        }
    }

    let tunnel_ids = distinct_target_tunnel_ids(&targets);
    let tunnel_map = match batch_tunnel_map(state, &tunnel_ids).await {
        Ok(v) => v,
        Err(_) => return None,
    };
    let mut op_ut_map: HashMap<i64, model::UserTunnel> = HashMap::new();
    let mut user_forward_count: i64 = 0;
    let mut tunnel_forward_count: HashMap<i64, i64> = HashMap::with_capacity(tunnel_ids.len());
    if user.role_id != 0 {
        let op_uts = match repo::list_user_tunnels_by_user_and_tunnel_ids(
            &state.db,
            user.user_id,
            &tunnel_ids,
        )
        .await
        {
            Ok(v) => v,
            Err(_) => return None,
        };
        for ut in op_uts {
            op_ut_map.insert(ut.tunnel_id, ut);
        }
        user_forward_count = match repo::count_by_user(&state.db, user.user_id).await {
            Ok(n) => n,
            Err(_) => return None,
        };
        for tid in &tunnel_ids {
            match repo::count_by_user_tunnel(&state.db, user.user_id, *tid, None).await {
                Ok(n) => {
                    tunnel_forward_count.insert(*tid, n);
                }
                Err(_) => return None,
            }
        }
    }

    let mut valid: Vec<model::Forward> = Vec::new();
    for f in &targets {
        let tid = f.tunnel_id;
        let Some(_tunnel) = tunnel_map.get(&tid) else {
            err_by_id.insert(f.id, "隧道不存在".to_string());
            continue;
        };
        if user.role_id != 0 {
            if !op_user_err.is_empty() {
                err_by_id.insert(f.id, op_user_err.clone());
                continue;
            }
            let op_user = op_user.as_ref().unwrap();
            let Some(ut) = op_ut_map.get(&tid) else {
                err_by_id.insert(f.id, "你没有该隧道权限".to_string());
                continue;
            };
            if ut.status != 1 {
                err_by_id.insert(f.id, "隧道被禁用".to_string());
                continue;
            }
            if ut.exp_time > 0 && ut.exp_time <= now {
                err_by_id.insert(f.id, "该隧道权限已到期".to_string());
                continue;
            }
            if !model::is_unlimited_flow(op_user.flow)
                && (op_user.flow <= 0
                    || op_user.flow * FORWARD_BYTES_TO_GB <= op_user.in_flow + op_user.out_flow)
            {
                err_by_id.insert(f.id, "用户总流量已用完".to_string());
                continue;
            }
            if !model::is_unlimited_flow(ut.flow)
                && (ut.flow <= 0
                    || ut.flow * FORWARD_BYTES_TO_GB <= ut.in_flow + ut.out_flow)
            {
                err_by_id.insert(f.id, "该隧道流量已用完".to_string());
                continue;
            }
            if !model::is_unlimited_num(op_user.num) && user_forward_count >= op_user.num {
                err_by_id.insert(
                    f.id,
                    format!("用户总转发数量已达上限，当前限制：{}个", op_user.num),
                );
                continue;
            }
            let tfc = tunnel_forward_count.get(&tid).copied().unwrap_or(0);
            if !model::is_unlimited_num(ut.num) && tfc >= ut.num {
                err_by_id.insert(
                    f.id,
                    format!("该隧道转发数量已达上限，当前限制：{}个", ut.num),
                );
                continue;
            }
        }
        valid.push(f.clone());
    }
    if valid.is_empty() {
        return Some((assemble_batch_msg("删除", &uniq, &err_by_id, 0), false));
    }

    let valid_tunnel_ids = distinct_target_tunnel_ids(&valid);
    let entries = match repo::list_entries_by_tunnel_ids(&state.db, &valid_tunnel_ids).await {
        Ok(v) => v,
        Err(_) => return None,
    };
    let mut entries_by_tunnel: HashMap<i64, Vec<model::ChainTunnel>> =
        HashMap::with_capacity(valid_tunnel_ids.len());
    let mut node_id_set: HashSet<i64> = HashSet::new();
    for ct in entries {
        entries_by_tunnel
            .entry(ct.tunnel_id)
            .or_default()
            .push(ct.clone());
        node_id_set.insert(ct.node_id);
    }
    let node_ids: Vec<i64> = node_id_set.into_iter().collect();
    let nodes = match repo::list_nodes_by_ids(&state.db, &node_ids).await {
        Ok(v) => v,
        Err(_) => return None,
    };
    let node_map: HashMap<i64, model::Node> = nodes.into_iter().map(|n| (n.id, n)).collect();
    let owner_uts = match repo::list_user_tunnels_by_tunnel_ids(&state.db, &valid_tunnel_ids).await
    {
        Ok(v) => v,
        Err(_) => return None,
    };
    let mut owner_ut_map: HashMap<String, model::UserTunnel> =
        HashMap::with_capacity(owner_uts.len());
    for ut in owner_uts {
        owner_ut_map.insert(ut_key(ut.user_id, ut.tunnel_id), ut);
    }

    // 校验入口节点存在，并把各转发的待删服务名按节点合并
    let mut names_by_node: HashMap<i64, Vec<String>> = HashMap::new();
    let mut entries_of: HashMap<i64, Vec<model::ChainTunnel>> =
        HashMap::with_capacity(valid.len());
    let mut do_delete: Vec<model::Forward> = Vec::new();
    for f in &valid {
        let cts = entries_by_tunnel
            .get(&f.tunnel_id)
            .cloned()
            .unwrap_or_default();
        let mut missing = false;
        for ct in &cts {
            if !node_map.contains_key(&ct.node_id) {
                missing = true;
                break;
            }
        }
        if missing {
            err_by_id.insert(f.id, "部分节点不存在".to_string());
            continue;
        }
        let base = build_service_name(
            f.id,
            f.user_id,
            owner_ut_map.get(&ut_key(f.user_id, f.tunnel_id)),
        );
        for ct in &cts {
            let entry = names_by_node.entry(ct.node_id).or_default();
            entry.push(format!("{}_tcp", base));
            entry.push(format!("{}_udp", base));
        }
        entries_of.insert(f.id, cts);
        do_delete.push(f.clone());
    }
    if do_delete.is_empty() {
        return Some((assemble_batch_msg("删除", &uniq, &err_by_id, 0), false));
    }

    // 每节点 1 次 WS 合并删除；整包失败的节点回落逐条补发
    let mut node_list: Vec<i64> = names_by_node.keys().copied().collect();
    node_list.sort_unstable();
    let state_ref = state;
    let names_ref = &names_by_node;
    let node_list_ref = &node_list;
    let node_errs = parallel_each_indexed(node_list.len(), BATCH_CONCURRENCY, move |i| {
        let nid = node_list_ref[i];
        let names = names_ref.get(&nid).cloned().unwrap_or_default();
        async move {
            let res = state_ref
                .hub
                .send_msg(nid, gost::delete_service_payload(names), "DeleteService")
                .await;
            if gost::is_ok(&res.msg) {
                String::new()
            } else {
                "fail".to_string()
            }
        }
    })
    .await;
    let failed_idx: HashSet<usize> = node_errs.iter().map(|(i, _)| *i).collect();
    if !failed_idx.is_empty() {
        let failed_set: HashSet<i64> = failed_idx
            .iter()
            .map(|i| node_list[*i])
            .collect();
        let state_ref = state;
        let entries_ref = &entries_of;
        let owner_ref = &owner_ut_map;
        let do_delete_ref = &do_delete;
        parallel_each(do_delete.len(), BATCH_CONCURRENCY, move |i| {
            let f = &do_delete_ref[i];
            let cts = entries_ref.get(&f.id).cloned().unwrap_or_default();
            let base = build_service_name(
                f.id,
                f.user_id,
                owner_ref.get(&ut_key(f.user_id, f.tunnel_id)),
            );
            let wanted: Vec<i64> = cts
                .iter()
                .map(|ct| ct.node_id)
                .filter(|nid| failed_set.contains(nid))
                .collect();
            async move {
                for nid in wanted {
                    let res = state_ref
                        .hub
                        .send_msg(
                            nid,
                            gost::delete_service_payload(vec![
                                format!("{}_tcp", base),
                                format!("{}_udp", base),
                            ]),
                            "DeleteService",
                        )
                        .await;
                    let _ = gost::normalize_ok(&res.msg);
                }
                String::new()
            }
        })
        .await;
    }

    let del_ids: Vec<i64> = do_delete.iter().map(|f| f.id).collect();
    let _ = port_repo::delete_by_forward_ids(&state.db, &del_ids).await;
    if let Err(e) = repo::delete_by_ids(&state.db, &del_ids).await {
        for f in &do_delete {
            err_by_id.insert(f.id, e.0.clone());
        }
        return Some((assemble_batch_msg("删除", &uniq, &err_by_id, 0), false));
    }
    Some((
        assemble_batch_msg("删除", &uniq, &err_by_id, do_delete.len()),
        true,
    ))
}

pub async fn batch_pause(
    state: &AppState,
    user: CurrentUser,
    ids: Vec<i64>,
) -> (String, bool) {
    batch_change_status(
        state,
        &user,
        &ids,
        0,
        "请选择要暂停的转发",
        "暂停",
        "PauseService",
    )
    .await
}

pub async fn batch_resume(
    state: &AppState,
    user: CurrentUser,
    ids: Vec<i64>,
) -> (String, bool) {
    batch_change_status(
        state,
        &user,
        &ids,
        1,
        "请选择要恢复的转发",
        "恢复",
        "ResumeService",
    )
    .await
}

/// 批量暂停/恢复：分发到快路径，预取失败回落逐条以保留原错误语义
async fn batch_change_status(
    state: &AppState,
    user: &CurrentUser,
    ids: &[i64],
    target_status: i32,
    empty_msg: &str,
    ok_verb: &str,
    gost_method: &str,
) -> (String, bool) {
    if ids.is_empty() {
        return (empty_msg.to_string(), false);
    }
    if ids.len() > 200 {
        return ("单次批量操作过多（最多200个）".to_string(), false);
    }
    if let Some(res) = batch_change_status_fast(
        state,
        user,
        ids,
        target_status,
        ok_verb,
        gost_method,
    )
    .await
    {
        return res;
    }
    if target_status == 0 {
        run_batch(ids, empty_msg, ok_verb, |id| {
            let state = state.clone();
            let user = user.clone();
            async move { pause(&state, user, id).await }
        })
        .await
    } else {
        run_batch(ids, empty_msg, ok_verb, |id| {
            let state = state.clone();
            let user = user.clone();
            async move { resume(&state, user, id).await }
        })
        .await
    }
}

/// 批量暂停/恢复快路径：一次预取校验 + 限并发 WS + 单语句状态写。
async fn batch_change_status_fast(
    state: &AppState,
    user: &CurrentUser,
    ids: &[i64],
    target_status: i32,
    ok_verb: &str,
    gost_method: &str,
) -> Option<(String, bool)> {
    let uniq = dedup_int64s(ids);
    let forwards = match batch_load_forwards(state, user, &uniq).await {
        Ok(v) => v,
        Err(_) => return None,
    };
    let now = now_ms();
    let mut err_by_id: HashMap<i64, String> = HashMap::with_capacity(uniq.len());

    // 非管理员先校验操作者状态（优先于“转发不存在”）
    let mut op_user: Option<model::User> = None;
    if user.role_id != 0 {
        match repo::get_user(&state.db, user.user_id).await {
            Ok(Some(u)) => {
                if u.status == 0 {
                    for id in &uniq {
                        err_by_id.insert(*id, "用户已到期或被禁用".to_string());
                    }
                    return Some((
                        assemble_batch_msg(ok_verb, &uniq, &err_by_id, 0),
                        false,
                    ));
                }
                op_user = Some(u);
            }
            Ok(None) => {
                for id in &uniq {
                    err_by_id.insert(*id, "用户不存在".to_string());
                }
                return Some((assemble_batch_msg(ok_verb, &uniq, &err_by_id, 0), false));
            }
            Err(_) => return None,
        }
    }
    let mut targets: Vec<model::Forward> = Vec::new();
    for id in &uniq {
        match forwards.get(id) {
            Some(f) => targets.push(f.clone()),
            None => {
                err_by_id.insert(*id, "转发不存在".to_string());
            }
        }
    }
    if targets.is_empty() {
        return Some((assemble_batch_msg(ok_verb, &uniq, &err_by_id, 0), false));
    }

    let tunnel_ids = distinct_target_tunnel_ids(&targets);
    let tunnel_map = match batch_tunnel_map(state, &tunnel_ids).await {
        Ok(v) => v,
        Err(_) => return None,
    };
    let mut op_ut_map: HashMap<i64, model::UserTunnel> = HashMap::new();
    if user.role_id != 0 {
        let op_uts = match repo::list_user_tunnels_by_user_and_tunnel_ids(
            &state.db,
            user.user_id,
            &tunnel_ids,
        )
        .await
        {
            Ok(v) => v,
            Err(_) => return None,
        };
        for ut in op_uts {
            op_ut_map.insert(ut.tunnel_id, ut);
        }
    }

    let mut valid: Vec<model::Forward> = Vec::new();
    for f in &targets {
        let tid = f.tunnel_id;
        let Some(tunnel) = tunnel_map.get(&tid) else {
            err_by_id.insert(f.id, "隧道不存在".to_string());
            continue;
        };
        if target_status == 1 {
            if tunnel.status != 1 {
                err_by_id.insert(f.id, "隧道已禁用，无法恢复服务".to_string());
                continue;
            }
            if user.role_id != 0 {
                let op_user = op_user.as_ref().unwrap();
                // 对齐 check_user_flow_limits 的检查顺序与文案
                if op_user.exp_time > 0 && op_user.exp_time <= now {
                    err_by_id.insert(f.id, "当前账号已到期".to_string());
                    continue;
                }
                let Some(ut) = op_ut_map.get(&tid) else {
                    err_by_id.insert(f.id, "你没有该隧道权限".to_string());
                    continue;
                };
                if ut.exp_time > 0 && ut.exp_time <= now {
                    err_by_id.insert(f.id, "该隧道权限已到期，无法恢复服务".to_string());
                    continue;
                }
                if !model::is_unlimited_flow(op_user.flow)
                    && op_user.flow * FORWARD_BYTES_TO_GB
                        <= op_user.in_flow + op_user.out_flow
                {
                    err_by_id.insert(f.id, "用户总流量已用完，无法恢复服务".to_string());
                    continue;
                }
                if !model::is_unlimited_flow(ut.flow)
                    && ut.flow * FORWARD_BYTES_TO_GB <= ut.in_flow + ut.out_flow
                {
                    err_by_id.insert(f.id, "该隧道流量已用完，无法恢复服务".to_string());
                    continue;
                }
                if ut.status != 1 {
                    err_by_id.insert(f.id, "隧道被禁用".to_string());
                    continue;
                }
            }
        } else if user.role_id != 0 && !op_ut_map.contains_key(&tid) {
            err_by_id.insert(f.id, "你没有该隧道权限".to_string());
            continue;
        }
        valid.push(f.clone());
    }
    if valid.is_empty() {
        return Some((assemble_batch_msg(ok_verb, &uniq, &err_by_id, 0), false));
    }

    let valid_tunnel_ids = distinct_target_tunnel_ids(&valid);
    let entries = match repo::list_entries_by_tunnel_ids(&state.db, &valid_tunnel_ids).await {
        Ok(v) => v,
        Err(_) => return None,
    };
    let mut entries_by_tunnel: HashMap<i64, Vec<model::ChainTunnel>> =
        HashMap::with_capacity(valid_tunnel_ids.len());
    let mut node_id_set: HashSet<i64> = HashSet::new();
    for ct in entries {
        entries_by_tunnel
            .entry(ct.tunnel_id)
            .or_default()
            .push(ct.clone());
        node_id_set.insert(ct.node_id);
    }
    let node_ids: Vec<i64> = node_id_set.into_iter().collect();
    let nodes = match repo::list_nodes_by_ids(&state.db, &node_ids).await {
        Ok(v) => v,
        Err(_) => return None,
    };
    let node_map: HashMap<i64, model::Node> = nodes.into_iter().map(|n| (n.id, n)).collect();
    let owner_uts = match repo::list_user_tunnels_by_tunnel_ids(&state.db, &valid_tunnel_ids).await
    {
        Ok(v) => v,
        Err(_) => return None,
    };
    let mut owner_ut_map: HashMap<String, model::UserTunnel> =
        HashMap::with_capacity(owner_uts.len());
    for ut in owner_uts {
        owner_ut_map.insert(ut_key(ut.user_id, ut.tunnel_id), ut);
    }
    // 恢复时节点缺服务需按端口重建，端口一次查回（暂停路径不需要）
    let ports_map: HashMap<i64, Vec<model::ForwardPort>> = if target_status == 1 {
        let valid_ids: Vec<i64> = valid.iter().map(|f| f.id).collect();
        match port_repo::list_by_forward_ids(&state.db, &valid_ids).await {
            Ok(v) => v,
            Err(_) => return None,
        }
    } else {
        HashMap::new()
    };

    struct StatusWork {
        fw: model::Forward,
        tunnel: model::Tunnel,
        entries: Vec<model::ChainTunnel>,
        base: String,
        limiter: Option<i64>,
    }
    let mut works: Vec<StatusWork> = Vec::with_capacity(valid.len());
    for f in &valid {
        let tid = f.tunnel_id;
        let cts = entries_by_tunnel
            .get(&tid)
            .cloned()
            .unwrap_or_default();
        let mut missing = false;
        for ct in &cts {
            if !node_map.contains_key(&ct.node_id) {
                missing = true;
                break;
            }
        }
        if missing {
            err_by_id.insert(f.id, "部分节点不存在".to_string());
            continue;
        }
        let ut = if user.role_id != 0 {
            op_ut_map.get(&tid)
        } else {
            owner_ut_map.get(&ut_key(f.user_id, f.tunnel_id))
        };
        let limiter = ut.and_then(|ut| ut.speed_id);
        works.push(StatusWork {
            fw: f.clone(),
            tunnel: tunnel_map.get(&tid).cloned().unwrap_or_default(),
            entries: cts,
            base: build_service_name(f.id, f.user_id, ut),
            limiter,
        });
    }
    if works.is_empty() {
        return Some((assemble_batch_msg(ok_verb, &uniq, &err_by_id, 0), false));
    }

    // 限并发下发 WS（暂停缺服务视为已达成；恢复缺服务按端口重建）
    let state_ref = state;
    let node_map_ref = &node_map;
    let ports_ref = &ports_map;
    let method = gost_method.to_string();
    let method_ref = &method;
    let works_ref = &works;
    let fail_msgs = parallel_each_indexed(works.len(), BATCH_CONCURRENCY, move |i| {
        let w = &works_ref[i];
        let method = method_ref.clone();
        async move {
            for ct in &w.entries {
                let node = node_map_ref.get(&ct.node_id).unwrap();
                let res = state_ref
                    .hub
                    .send_msg(
                        node.id,
                        gost::pause_resume_payload(&w.base),
                        &method,
                    )
                    .await;
                if gost::is_ok(&res.msg) {
                    continue;
                }
                if gost::is_missing(&res.msg) {
                    if target_status == 0 {
                        continue;
                    }
                    let fps = ports_ref.get(&w.fw.id).map(|v| v.as_slice());
                    let Some(fp) = find_forward_port(fps, node.id) else {
                        return format!("节点 {} 缺少端口记录，无法重建服务", node.name);
                    };
                    let msg = add_or_update_service(
                        state_ref,
                        &w.base,
                        w.limiter,
                        node,
                        &w.fw,
                        fp,
                        &w.tunnel,
                        "AddService",
                    )
                    .await;
                    if !gost::is_ok(&msg) {
                        return format!("节点 {} 重建服务失败: {}", node.name, msg);
                    }
                    continue;
                }
                return gost::normalize_ok(&res.msg);
            }
            String::new()
        }
    })
    .await;
    let mut failed: HashMap<usize, String> = fail_msgs.into_iter().collect();
    let mut succeeded: Vec<i64> = Vec::new();
    for (i, w) in works.iter().enumerate() {
        if let Some(msg) = failed.remove(&i) {
            err_by_id.insert(w.fw.id, msg);
            continue;
        }
        succeeded.push(w.fw.id);
    }
    if !succeeded.is_empty() {
        if let Err(e) = repo::update_status_by_ids(&state.db, &succeeded, target_status, now).await
        {
            for id in &succeeded {
                err_by_id.insert(*id, e.0.clone());
            }
            return Some((assemble_batch_msg(ok_verb, &uniq, &err_by_id, 0), false));
        }
    }
    let ok = !succeeded.is_empty();
    Some((
        assemble_batch_msg(ok_verb, &uniq, &err_by_id, succeeded.len()),
        ok,
    ))
}

pub async fn batch_change_tunnel(
    state: &AppState,
    user: CurrentUser,
    ids: Vec<i64>,
    tunnel_id: i64,
) -> (String, bool) {
    if tunnel_id == 0 {
        return ("请选择目标隧道".to_string(), false);
    }
    if ids.is_empty() {
        return ("请选择要更改隧道的转发".to_string(), false);
    }
    if ids.len() > 200 {
        return ("单次批量操作过多（最多200个）".to_string(), false);
    }
    let (forwards, ports_map, prefetch_ok) = prefetch_change_tunnel(state, &user, &ids).await;
    let state_ref = state;
    let user_ref = &user;
    let forwards_ref = &forwards;
    let ports_map_ref = &ports_map;
    run_batch(&ids, "请选择要更改隧道的转发", "迁移", move |id| {
        let state = state_ref.clone();
        let user = user_ref.clone();
        let forwards = forwards_ref;
        let ports_map = ports_map_ref;
        async move {
            let exist = if prefetch_ok {
                let Some(f) = forwards.get(&id) else {
                    return Err(AppError::new("转发不存在或无权限"));
                };
                f.clone()
            } else {
                let Some(f) = validate_forward_exists(&state, id, &user).await? else {
                    return Err(AppError::new("转发不存在或无权限"));
                };
                f
            };
            let in_port = if prefetch_ok {
                // 沿用原入口端口，避免重分配（对齐 resolve_keep_in_port）
                ports_map
                    .get(&id)
                    .and_then(|fps| fps.iter().find(|fp| fp.port > 0))
                    .map(|fp| fp.port)
            } else {
                resolve_keep_in_port(&state, None, exist.id).await?
            };
            let req = ForwardUpdateReq {
                id,
                tunnel_id: Some(tunnel_id),
                name: exist.name.clone(),
                user_id: exist.user_id,
                remote_addr: exist.remote_addr.clone(),
                strategy: exist.strategy.clone(),
                in_port,
            };
            update(&state, user, req).await
        }
    })
    .await
}

/// 一次取回批量改隧道所需的转发与原端口
async fn prefetch_change_tunnel(
    state: &AppState,
    user: &CurrentUser,
    ids: &[i64],
) -> (
    HashMap<i64, model::Forward>,
    HashMap<i64, Vec<model::ForwardPort>>,
    bool,
) {
    let forwards = match batch_load_forwards(state, user, ids).await {
        Ok(v) => v,
        Err(_) => return (HashMap::new(), HashMap::new(), false),
    };
    let ports_map = match port_repo::list_by_forward_ids(&state.db, &dedup_int64s(ids)).await {
        Ok(v) => v,
        Err(_) => return (HashMap::new(), HashMap::new(), false),
    };
    (forwards, ports_map, true)
}

/// 对入口节点并行执行 fn；任一失败返回首个错误。
async fn for_each_chain_node_parallel<F, Fut>(
    state: &AppState,
    chain_tunnels: &[model::ChainTunnel],
    f: F,
) -> AppResult<()>
where
    F: Fn(model::Node) -> Fut + Sync,
    Fut: Future<Output = AppResult<()>>,
{
    if chain_tunnels.is_empty() {
        return Ok(());
    }
    if chain_tunnels.len() == 1 {
        let Some(node) = repo::get_node(&state.db, chain_tunnels[0].node_id).await? else {
            return Err(AppError::new("部分节点不存在"));
        };
        return f(node).await;
    }

    let f_ref = &f;
    let state_ref = state;
    let mut results: Vec<(usize, AppResult<()>)> =
        futures_util::stream::iter(chain_tunnels.iter().cloned().enumerate())
            .map(|(i, ct)| {
                let f = f_ref;
                async move {
                    let r = async {
                        let Some(node) =
                            repo::get_node(&state_ref.db, ct.node_id).await?
                        else {
                            return Err(AppError::new("部分节点不存在"));
                        };
                        f(node).await
                    }
                    .await;
                    (i, r)
                }
            })
            .buffer_unordered(chain_tunnels.len())
            .collect()
            .await;
    results.sort_by_key(|(i, _)| *i);
    for (_, r) in results {
        r?;
    }
    Ok(())
}

/// 换隧道时沿用原入口端口：请求指定优先，否则取该转发已有端口。
/// 返回 None 表示历史上无端口记录（仅此时才允许 get_port 自动分配）。
async fn resolve_keep_in_port(
    state: &AppState,
    req_port: Option<i32>,
    forward_id: i64,
) -> AppResult<Option<i32>> {
    if let Some(p) = req_port {
        if p > 0 {
            return Ok(Some(p));
        }
    }
    let old_ports = port_repo::list_by_forward_id(&state.db, forward_id).await?;
    if old_ports.is_empty() {
        return Ok(None);
    }
    // 多入口节点时端口应一致；取首个非 0 端口
    for fp in old_ports {
        if fp.port > 0 {
            return Ok(Some(fp.port));
        }
    }
    Ok(None)
}

// ---- helpers ----

async fn validate_forward_exists(
    state: &AppState,
    forward_id: i64,
    user: &CurrentUser,
) -> AppResult<Option<model::Forward>> {
    let Some(f) = repo::get_by_id(&state.db, forward_id).await? else {
        return Ok(None);
    };
    if user.role_id != 0 && f.user_id != user.user_id {
        return Ok(None);
    }
    Ok(Some(f))
}

async fn check_user_permissions(
    state: &AppState,
    user: &CurrentUser,
    tunnel: &model::Tunnel,
    exclude_forward_id: Option<i64>,
) -> AppResult<UserPermission> {
    if user.role_id == 0 {
        return Ok(UserPermission {
            limiter: None,
            user_tunnel: None,
        });
    }
    let Some(user_info) = repo::get_user(&state.db, user.user_id).await? else {
        return Err(AppError::new("用户不存在"));
    };
    if user_info.exp_time > 0 && user_info.exp_time <= now_ms() {
        return Err(AppError::new("当前账号已到期"));
    }
    let Some(ut) = repo::get_user_tunnel(&state.db, user.user_id, tunnel.id).await? else {
        return Err(AppError::new("你没有该隧道权限"));
    };
    if ut.status != 1 {
        return Err(AppError::new("隧道被禁用"));
    }
    if ut.exp_time > 0 && ut.exp_time <= now_ms() {
        return Err(AppError::new("该隧道权限已到期"));
    }
    // 无限流量直接通过配额检查；有限流量检查已用是否超限
    if !model::is_unlimited_flow(user_info.flow)
        && (user_info.flow <= 0
            || user_info.flow * FORWARD_BYTES_TO_GB
                <= user_info.in_flow + user_info.out_flow)
    {
        return Err(AppError::new("用户总流量已用完"));
    }
    if !model::is_unlimited_flow(ut.flow)
        && (ut.flow <= 0 || ut.flow * FORWARD_BYTES_TO_GB <= ut.in_flow + ut.out_flow)
    {
        return Err(AppError::new("该隧道流量已用完"));
    }
    check_forward_quota(state, user.user_id, tunnel.id, &ut, &user_info, exclude_forward_id)
        .await?;
    Ok(UserPermission {
        limiter: ut.speed_id,
        user_tunnel: Some(ut),
    })
}

async fn check_forward_quota(
    state: &AppState,
    user_id: i64,
    tunnel_id: i64,
    ut: &model::UserTunnel,
    user_info: &model::User,
    exclude_forward_id: Option<i64>,
) -> AppResult<()> {
    let n = repo::count_by_user(&state.db, user_id).await?;
    if !model::is_unlimited_num(user_info.num) && n >= user_info.num {
        return Err(AppError::new(format!(
            "用户总转发数量已达上限，当前限制：{}个",
            user_info.num
        )));
    }
    let tn =
        repo::count_by_user_tunnel(&state.db, user_id, tunnel_id, exclude_forward_id).await?;
    if !model::is_unlimited_num(ut.num) && tn >= ut.num {
        return Err(AppError::new(format!(
            "该隧道转发数量已达上限，当前限制：{}个",
            ut.num
        )));
    }
    Ok(())
}

async fn check_user_flow_limits(
    state: &AppState,
    user_id: i64,
    tunnel: &model::Tunnel,
) -> AppResult<()> {
    let Some(user_info) = repo::get_user(&state.db, user_id).await? else {
        return Err(AppError::new("用户不存在"));
    };
    if user_info.exp_time > 0 && user_info.exp_time <= now_ms() {
        return Err(AppError::new("当前账号已到期"));
    }
    let Some(ut) = repo::get_user_tunnel(&state.db, user_id, tunnel.id).await? else {
        return Err(AppError::new("你没有该隧道权限"));
    };
    if ut.exp_time > 0 && ut.exp_time <= now_ms() {
        return Err(AppError::new("该隧道权限已到期，无法恢复服务"));
    }
    if !model::is_unlimited_flow(user_info.flow)
        && user_info.flow * FORWARD_BYTES_TO_GB <= user_info.in_flow + user_info.out_flow
    {
        return Err(AppError::new("用户总流量已用完，无法恢复服务"));
    }
    let tunnel_flow = ut.in_flow + ut.out_flow;
    if !model::is_unlimited_flow(ut.flow) && ut.flow * FORWARD_BYTES_TO_GB <= tunnel_flow {
        return Err(AppError::new("该隧道流量已用完，无法恢复服务"));
    }
    Ok(())
}

pub fn build_service_name(
    forward_id: i64,
    user_id: i64,
    user_tunnel: Option<&model::UserTunnel>,
) -> String {
    let ut_id = user_tunnel.map(|ut| ut.id).unwrap_or(0);
    gost::forward_service_base(forward_id, user_id, ut_id)
}

pub async fn add_or_update_service(
    state: &AppState,
    service_name: &str,
    limiter: Option<i64>,
    node: &model::Node,
    forward: &model::Forward,
    fp: &model::ForwardPort,
    tunnel: &model::Tunnel,
    method: &str,
) -> String {
    let iface = node.interface_name.clone().unwrap_or_default();
    let mut tcp_listen = node.tcp_listen_addr.clone();
    let mut udp_listen = node.udp_listen_addr.clone();
    if tcp_listen.is_empty() {
        tcp_listen = "0.0.0.0".to_string();
    }
    if udp_listen.is_empty() {
        udp_listen = "0.0.0.0".to_string();
    }
    let services = gost::build_forward_services(
        service_name,
        limiter,
        &tcp_listen,
        &udp_listen,
        fp.port,
        &iface,
        tunnel.r#type,
        forward.tunnel_id,
        &forward.remote_addr,
        &forward.strategy,
    );
    let payload = Value::Array(services.clone());
    let mut res = state.hub.send_msg(node.id, payload.clone(), method).await;
    // Update 在空节点上必然 not found：回落 Add（弃用本地 gost.json 后的主路径）
    if method == "UpdateService" && !gost::is_ok(&res.msg) {
        res = state.hub.send_msg(node.id, payload, "AddService").await;
    }
    gost::normalize_ok(&res.msg)
}

/// 对齐 Java get_port：为入口节点分配端口
async fn get_port(
    state: &AppState,
    mut chain_tunnel_list: Vec<model::ChainTunnel>,
    in_port: Option<i32>,
    forward_id: i64,
) -> AppResult<Vec<model::ChainTunnel>> {
    if chain_tunnel_list.is_empty() {
        return Ok(chain_tunnel_list);
    }
    let mut lists: Vec<Vec<i32>> = Vec::with_capacity(chain_tunnel_list.len());
    for ct in &chain_tunnel_list {
        let ports = get_node_port(state, ct.node_id, forward_id).await?;
        if ports.is_empty() {
            return Err(AppError::new("暂无可用端口"));
        }
        lists.push(ports);
    }

    if let Some(p) = in_port {
        for ports in &lists {
            if !contains_int(ports, p) {
                return Err(AppError::new(format!(
                    "指定端口 {} 不可用（并非所有节点都有此端口）",
                    p
                )));
            }
        }
        for ct in chain_tunnel_list.iter_mut() {
            ct.port = Some(p);
        }
        return Ok(chain_tunnel_list);
    }

    // 交集随机
    let mut inter: HashSet<i32> = lists[0].iter().copied().collect();
    for list in lists.iter().skip(1) {
        let cur = inter.clone();
        let set: HashSet<i32> = list
            .iter()
            .copied()
            .filter(|p| cur.contains(p))
            .collect();
        inter = set;
    }
    if !inter.is_empty() {
        let mut common: Vec<i32> = inter.into_iter().collect();
        common.sort_unstable();
        let mut rng = rand::thread_rng();
        let chosen = common[rng.gen_range(0..common.len())];
        for ct in chain_tunnel_list.iter_mut() {
            ct.port = Some(chosen);
        }
        return Ok(chain_tunnel_list);
    }

    // 各自随机
    let mut rng = rand::thread_rng();
    for (i, ct) in chain_tunnel_list.iter_mut().enumerate() {
        let ports = &lists[i];
        let p = ports[rng.gen_range(0..ports.len())];
        ct.port = Some(p);
    }
    Ok(chain_tunnel_list)
}

async fn get_node_port(
    state: &AppState,
    node_id: i64,
    forward_id: i64,
) -> AppResult<Vec<i32>> {
    let Some(node) = repo::get_node(&state.db, node_id).await? else {
        return Err(AppError::new("节点不存在"));
    };
    let mut used: HashSet<i32> = HashSet::new();
    // chain_tunnel 占用
    let cts = repo::list_chain_tunnels_by_node(&state.db, node_id).await?;
    for ct in cts {
        if let Some(p) = ct.port {
            used.insert(p);
        }
    }
    // forward_port 占用（排除自身）
    let fps = port_repo::list_by_node_exclude_forward(&state.db, node_id, forward_id).await?;
    for fp in fps {
        used.insert(fp.port);
    }
    let parsed = repo::parse_ports(&node.port)?;
    Ok(parsed.into_iter().filter(|p| !used.contains(p)).collect())
}

// ---- small utils ----

/// remote_addr 入库校验：逗号分隔 host:port，最多 10 个，总长 ≤1024，
/// host 为 IP 或合规域名，port 1-65535。内网地址属转发业务正常需求，
/// 不在此拦截；扫描风险由 diagnose 限频+权限收敛。
fn validate_remote_addr(raw: &str) -> AppResult<()> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(AppError::new("远程地址不能为空"));
    }
    if raw.len() > 1024 {
        return Err(AppError::new("远程地址过长"));
    }
    let parts: Vec<&str> = raw.split(',').collect();
    if parts.len() > 10 {
        return Err(AppError::new("远程地址过多（最多10个）"));
    }
    for p in parts {
        let a = p.trim();
        if a.is_empty() || a.len() > 253 {
            return Err(AppError::new("远程地址格式不正确"));
        }
        let ip = extract_ip_from_address(a);
        let port = extract_port_from_address(a);
        if ip.is_empty() || port < 1 || port > 65535 {
            return Err(AppError::new("远程地址格式不正确"));
        }
        if !(ip.parse::<std::net::IpAddr>().is_ok() || is_valid_hostname(&ip)) {
            return Err(AppError::new("远程地址格式不正确"));
        }
    }
    Ok(())
}

fn is_valid_hostname(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    // 兼容 gost 变量占位（如 {{port}} 类模板不走此处，入库时已是具体地址）
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    })
}

fn deref_int(p: Option<i32>) -> i32 {
    p.unwrap_or(0)
}

fn contains_int(list: &[i32], v: i32) -> bool {
    list.contains(&v)
}

fn ports_to_ints(ports: &[model::ForwardPort]) -> Vec<i32> {
    ports.iter().map(|p| p.port).collect()
}

fn unique_ints(input: &[i32]) -> Vec<i32> {
    let mut seen: HashSet<i32> = HashSet::new();
    let mut out: Vec<i32> = Vec::with_capacity(input.len());
    for v in input {
        if seen.insert(*v) {
            out.push(*v);
        }
    }
    out
}

fn unique_strings(input: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::with_capacity(input.len());
    for v in input {
        if seen.insert(v.clone()) {
            out.push(v.clone());
        }
    }
    out
}

fn extract_ip_from_address(address: &str) -> String {
    let address = address.trim();
    if address.is_empty() {
        return String::new();
    }
    if address.starts_with('[') {
        if let Some(close) = address.find(']') {
            if close > 1 {
                return address[1..close].to_string();
            }
        }
    }
    if let Some(last_colon) = address.rfind(':') {
        if last_colon > 0 {
            return address[..last_colon].to_string();
        }
    }
    address.to_string()
}

fn extract_port_from_address(address: &str) -> i32 {
    let address = address.trim();
    if address.is_empty() {
        return -1;
    }
    if address.starts_with('[') {
        if let Some(close) = address.find(']') {
            if close > 1 && close + 1 < address.len() && address.as_bytes()[close + 1] == b':' {
                let port_str = &address[close + 2..];
                if let Some(p) = scan_int_prefix(port_str) {
                    return p;
                }
                return -1;
            }
        }
    }
    if let Some(last_colon) = address.rfind(':') {
        if last_colon > 0 && last_colon + 1 < address.len() {
            if let Some(p) = scan_int_prefix(&address[last_colon + 1..]) {
                return p;
            }
        }
    }
    -1
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

fn to_float(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}
