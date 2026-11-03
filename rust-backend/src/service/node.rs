use std::time::Duration;

use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::gost;
use crate::model::Node;
use crate::repo::node as repo;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeCreateReq {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub server_ip: String,
    #[serde(default)]
    pub port: String,
    #[serde(default)]
    pub interface_name: Option<String>,
    #[serde(default)]
    pub tcp_listen_addr: String,
    #[serde(default)]
    pub udp_listen_addr: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeUpdateReq {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub server_ip: String,
    #[serde(default)]
    pub port: String,
    #[serde(default)]
    pub interface_name: Option<String>,
    #[serde(default)]
    pub http: Option<i32>,
    #[serde(default)]
    pub tls: Option<i32>,
    #[serde(default)]
    pub socks: Option<i32>,
    #[serde(default)]
    pub tcp_listen_addr: String,
    #[serde(default)]
    pub udp_listen_addr: String,
}

pub async fn create(state: &AppState, req: NodeCreateReq) -> AppResult<()> {
    if req.name.trim().is_empty() {
        return Err(AppError::new("节点名称不能为空"));
    }
    if req.server_ip.trim().is_empty() {
        return Err(AppError::new("服务器ip不能为空"));
    }
    if req.port.trim().is_empty() {
        return Err(AppError::new("可用端口不能为空"));
    }
    validate_port_range(&req.port)?;
    let mut n = Node {
        name: req.name,
        secret: Uuid::new_v4().simple().to_string(),
        server_ip: req.server_ip,
        port: req.port,
        interface_name: req.interface_name,
        status: 0,
        http: 0,
        tls: 0,
        socks: 0,
        tcp_listen_addr: req.tcp_listen_addr,
        udp_listen_addr: req.udp_listen_addr,
        ..Default::default()
    };
    repo::create(&state.db, &mut n).await
}

pub async fn list(state: &AppState) -> AppResult<Vec<Node>> {
    let mut list = repo::list_order_by_status_desc(&state.db).await?;
    for n in list.iter_mut() {
        n.secret = String::new();
    }
    Ok(list)
}

pub async fn update(state: &AppState, req: NodeUpdateReq) -> AppResult<()> {
    if req.id == 0 {
        return Err(AppError::new("节点ID不能为空"));
    }
    if req.name.trim().is_empty() {
        return Err(AppError::new("节点名称不能为空"));
    }
    if req.server_ip.trim().is_empty() {
        return Err(AppError::new("服务器ip不能为空"));
    }
    if req.port.trim().is_empty() {
        return Err(AppError::new("可用port不能为空"));
    }
    validate_port_range(&req.port)?;

    let Some(node) = repo::find_by_id(&state.db, req.id).await? else {
        return Err(AppError::new("节点不存在"));
    };

    let online = node.status == 1;
    let new_http = req.http;
    let new_tls = req.tls;
    let new_socks = req.socks;

    let http_changed = new_http.map(|v| v != node.http).unwrap_or(false);
    let tls_changed = new_tls.map(|v| v != node.tls).unwrap_or(false);
    let socks_changed = new_socks.map(|v| v != node.socks).unwrap_or(false);

    if online && (http_changed || tls_changed || socks_changed) {
        let payload = json!({
            "http": new_http,
            "tls": new_tls,
            "socks": new_socks,
        });
        let res = state.hub.send_msg(node.id, payload, "SetProtocol").await;
        if !gost::is_ok(&res.msg) {
            return Err(AppError::new(res.msg));
        }
    }

    let mut upd = Node {
        id: req.id,
        name: req.name,
        server_ip: req.server_ip,
        port: req.port,
        interface_name: req.interface_name,
        tcp_listen_addr: req.tcp_listen_addr,
        udp_listen_addr: req.udp_listen_addr,
        http: new_http.unwrap_or(node.http),
        tls: new_tls.unwrap_or(node.tls),
        socks: new_socks.unwrap_or(node.socks),
        ..Default::default()
    };
    repo::update(&state.db, &mut upd).await
}

pub async fn delete(state: &AppState, id: i64) -> AppResult<()> {
    if repo::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::new("节点不存在"));
    }
    // 从隧道拓扑剔除，禁止级联删隧道
    crate::service::tunnel_detach::detach_node_from_tunnels(state, id).await?;
    repo::delete(&state.db, id).await
}

pub async fn install_command(state: &AppState, id: i64) -> AppResult<String> {
    let Some(node) = repo::find_by_id(&state.db, id).await? else {
        return Err(AppError::new("节点不存在"));
    };
    let ip = repo::get_vite_config(&state.db, "ip").await?;
    if ip.is_empty() {
        return Err(AppError::new("请先前往网站配置中设置ip"));
    }
    let processed = gost::process_server_address(&ip);
    let mut b = String::new();
    b.push_str("curl -L https://raw.githubusercontent.com/Fearless743/flux-panel/refs/heads/beta/install.sh");
    b.push_str(" -o ./install.sh && chmod +x ./install.sh && ");
    b.push_str("./install.sh");
    b.push_str(" -a ");
    b.push_str(&processed);
    b.push_str(" -s ");
    b.push_str(&node.secret);

    let ssl = repo::get_vite_config(&state.db, "ssl").await?;
    if ssl == "true" {
        b.push_str(" -l");
    }
    Ok(b)
}

/// 远程升级请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeUpgradeReq {
    #[serde(default)]
    pub id: i64,
    /// 空或 latest；也可为 tag 如 v1.2.3
    #[serde(default)]
    pub version: String,
    /// 可选，覆盖默认 Release 地址（节点侧仍校验白名单）
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub sha256: String,
    /// 可选 amd64/arm64，默认由节点自检
    #[serde(default)]
    pub arch: String,
}

/// 向在线节点下发 Upgrade 命令；节点下载校验替换后重启，失败自动 rollback
pub async fn upgrade(state: &AppState, req: NodeUpgradeReq) -> AppResult<String> {
    if req.id == 0 {
        return Err(AppError::new("节点ID不能为空"));
    }
    let Some(node) = repo::find_by_id(&state.db, req.id).await? else {
        return Err(AppError::new("节点不存在"));
    };
    if !state.hub.is_node_online(req.id) {
        return Err(AppError::new("节点不在线，无法远程升级"));
    }

    let mut version = req.version.trim().to_string();
    if version.is_empty() {
        version = "latest".to_string();
    }
    let mut payload = json!({ "version": version });
    let url = req.url.trim();
    if !url.is_empty() {
        payload["url"] = json!(url);
    }
    let sha256 = req.sha256.trim();
    if !sha256.is_empty() {
        payload["sha256"] = json!(sha256);
    }
    let arch = req.arch.trim();
    if !arch.is_empty() {
        payload["arch"] = json!(arch);
    }

    // 节点侧异步下载；此处只等「任务已接受」
    let res = state
        .hub
        .send_msg_timeout(req.id, payload, "Upgrade", Duration::from_secs(15))
        .await;
    if !gost::is_ok(&res.msg) {
        return Err(AppError::new(res.msg));
    }
    let mut msg =
        "升级任务已接受，节点将在后台下载并重启；失败时自动回滚到上一版本，面板会自动更新版本号"
            .to_string();
    if let Some(v) = &node.version {
        if !v.is_empty() {
            msg = format!("当前版本 {}；{}", v, msg);
        }
    }
    Ok(msg)
}

pub async fn get_by_secret(state: &AppState, secret: &str) -> AppResult<Option<Node>> {
    repo::find_by_secret(&state.db, secret).await
}

pub async fn mark_online(
    state: &AppState,
    id: i64,
    version: &str,
    http: &str,
    tls: &str,
    socks: &str,
) -> AppResult<()> {
    let v = if version.is_empty() {
        None
    } else {
        Some(version)
    };
    let h = if http.is_empty() {
        None
    } else {
        http.parse::<i32>().ok()
    };
    let t = if tls.is_empty() {
        None
    } else {
        tls.parse::<i32>().ok()
    };
    let so = if socks.is_empty() {
        None
    } else {
        socks.parse::<i32>().ok()
    };
    repo::update_online(&state.db, id, v, h, t, so).await
}

pub async fn mark_offline(state: &AppState, id: i64) -> AppResult<()> {
    repo::update_status(&state.db, id, 0).await
}

/// 对齐 Java validatePortRange
pub fn validate_port_range(port: &str) -> AppResult<()> {
    if port.is_empty() {
        return Err(AppError::new("可用端口不合法"));
    }
    for part in port.split(',') {
        let part = part.trim();
        if !valid_port_pattern(part) {
            return Err(AppError::new("可用端口不合法"));
        }
        if part.contains('-') {
            let mut it = part.split('-');
            let start = it.next().unwrap_or("").parse::<i64>().unwrap_or(0);
            let end = it.next().unwrap_or("").parse::<i64>().unwrap_or(0);
            if start < 0 || end < 0 || end > 65535 || start > end {
                return Err(AppError::new("可用端口不合法"));
            }
        } else {
            let p = part.parse::<i64>().unwrap_or(0);
            if !(0..=65535).contains(&p) {
                return Err(AppError::new("可用端口不合法"));
            }
        }
    }
    Ok(())
}

/// `^([0-9]{1,5})(-([0-9]{1,5}))?$`
fn valid_port_pattern(s: &str) -> bool {
    let Some((a, b)) = s.split_once('-') else {
        return is_digits_1_to_5(s);
    };
    is_digits_1_to_5(a) && is_digits_1_to_5(b) && !b.contains('-')
}

fn is_digits_1_to_5(s: &str) -> bool {
    !s.is_empty() && s.len() <= 5 && s.bytes().all(|b| b.is_ascii_digit())
}
