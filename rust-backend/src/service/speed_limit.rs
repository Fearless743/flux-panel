use serde::Deserialize;
use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::gost;
use crate::model::{SpeedLimit, Tunnel};
use crate::repo::speed_limit as repo;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpeedLimitReq {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub speed: i32,
    #[serde(default)]
    pub tunnel_id: i64,
    #[serde(default)]
    pub tunnel_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSpeedLimitReq {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub speed: i32,
}

pub async fn create(state: &AppState, req: CreateSpeedLimitReq) -> AppResult<()> {
    if req.name.is_empty() {
        return Err(AppError::new("限速规则名称不能为空"));
    }
    if req.speed < 1 {
        return Err(AppError::new("速度限制必须大于0"));
    }
    if req.tunnel_id == 0 {
        return Err(AppError::new("隧道ID不能为空"));
    }
    if req.tunnel_name.is_empty() {
        return Err(AppError::new("隧道名称不能为空"));
    }

    let tunnel = repo::get_tunnel_by_id(&state.db, req.tunnel_id)
        .await
        .map_err(|e| AppError::new(format!("查询隧道失败: {}", e.0)))?;
    if tunnel.is_none() {
        return Err(AppError::new("隧道不存在"));
    }

    let now = crate::response::now_ms();
    let mut sl = SpeedLimit {
        name: req.name,
        speed: req.speed,
        tunnel_id: req.tunnel_id,
        tunnel_name: req.tunnel_name,
        created_time: now,
        updated_time: Some(now),
        status: 1,
        ..Default::default()
    };
    let id = repo::create(&state.db, &sl)
        .await
        .map_err(|e| AppError::new(format!("保存限速规则失败: {}", e.0)))?;
    sl.id = id;

    let speed_in_mbps = convert_bits_to_mbps(sl.speed);
    let node_ids = match repo::list_chain_node_ids(&state.db, sl.tunnel_id).await {
        Ok(v) => v,
        Err(e) => {
            let _ = repo::delete(&state.db, sl.id).await;
            return Err(AppError::new(format!("查询隧道节点失败: {}", e.0)));
        }
    };

    let mut success: Vec<i64> = Vec::with_capacity(node_ids.len());
    let mut errs: Vec<String> = Vec::new();
    for node_id in &node_ids {
        let res = state
            .hub
            .send_msg(
                *node_id,
                gost::limiter_data(sl.id, &speed_in_mbps),
                "AddLimiters",
            )
            .await;
        if gost::is_ok(&res.msg) {
            success.push(*node_id);
        } else {
            errs.push(gost::normalize_ok(&res.msg));
        }
    }
    if !errs.is_empty() {
        // 失败回滚 DB + 已成功节点的 DeleteLimiters
        let _ = repo::delete(&state.db, sl.id).await;
        for node_id in &success {
            let _ = state
                .hub
                .send_msg(
                    *node_id,
                    json!({ "limiter": sl.id.to_string() }),
                    "DeleteLimiters",
                )
                .await;
        }
        return Err(AppError::new(errs[0].clone()));
    }
    Ok(())
}

pub async fn list(state: &AppState) -> AppResult<Vec<SpeedLimit>> {
    repo::list(&state.db).await
}

pub async fn update(state: &AppState, req: UpdateSpeedLimitReq) -> AppResult<()> {
    if req.id == 0 {
        return Err(AppError::new("ID不能为空"));
    }
    if req.name.is_empty() {
        return Err(AppError::new("限速规则名称不能为空"));
    }
    if req.speed < 1 {
        return Err(AppError::new("速度限制必须大于0"));
    }

    let Some(mut sl) = repo::get_by_id(&state.db, req.id)
        .await
        .map_err(|e| AppError::new(format!("查询限速规则失败: {}", e.0)))?
    else {
        return Err(AppError::new("限速不存在"));
    };

    sl.name = req.name;
    sl.speed = req.speed;
    sl.updated_time = Some(crate::response::now_ms());

    let speed_in_mbps = convert_bits_to_mbps(sl.speed);
    let node_ids = repo::list_chain_node_ids(&state.db, sl.tunnel_id)
        .await
        .map_err(|e| AppError::new(format!("查询隧道节点失败: {}", e.0)))?;
    let mut errs: Vec<String> = Vec::new();
    for node_id in &node_ids {
        let payload = json!({
            "limiter": sl.id.to_string(),
            "data": gost::limiter_data(sl.id, &speed_in_mbps),
        });
        let res = state
            .hub
            .send_msg(*node_id, payload, "UpdateLimiters")
            .await;
        // Java 对 Update 不做 exists/not found 归一，仅接受 OK
        if res.msg != "OK" {
            errs.push(res.msg);
        }
    }
    if !errs.is_empty() {
        return Err(AppError::new(errs[0].clone()));
    }

    repo::update(&state.db, &sl)
        .await
        .map_err(|e| AppError::new(format!("更新限速规则失败: {}", e.0)))
}

pub async fn delete(state: &AppState, id: i64) -> AppResult<()> {
    if id == 0 {
        return Err(AppError::new("ID不能为空"));
    }
    let Some(sl) = repo::get_by_id(&state.db, id)
        .await
        .map_err(|e| AppError::new(format!("查询限速规则失败: {}", e.0)))?
    else {
        return Err(AppError::new("限速规则不存在"));
    };

    let cnt = repo::count_user_tunnel_by_speed_id(&state.db, sl.id)
        .await
        .map_err(|e| AppError::new(format!("检查引用失败: {}", e.0)))?;
    if cnt != 0 {
        return Err(AppError::new("该限速规则还有用户在使用 请先取消分配"));
    }

    let node_ids = repo::list_chain_node_ids(&state.db, sl.tunnel_id)
        .await
        .map_err(|e| AppError::new(format!("查询隧道节点失败: {}", e.0)))?;
    let mut errs: Vec<String> = Vec::new();
    for node_id in &node_ids {
        let res = state
            .hub
            .send_msg(
                *node_id,
                json!({ "limiter": sl.id.to_string() }),
                "DeleteLimiters",
            )
            .await;
        if !gost::is_ok(&res.msg) {
            errs.push(res.msg);
        }
    }
    if !errs.is_empty() {
        return Err(AppError::new(errs[0].clone()));
    }

    repo::delete(&state.db, id)
        .await
        .map_err(|e| AppError::new(format!("删除限速规则失败: {}", e.0)))
}

pub async fn list_tunnels(state: &AppState) -> AppResult<Vec<Tunnel>> {
    repo::list_all_tunnels(&state.db).await
}

/// speed(Mbps 量级，Java 命名 bits) / 8 → MB/s，一位小数
pub fn convert_bits_to_mbps(speed_in_bits: i32) -> String {
    let mbs = speed_in_bits as f64 / 8.0;
    let rounded = (mbs * 10.0).round() / 10.0;
    format!("{:.1}", rounded)
}
