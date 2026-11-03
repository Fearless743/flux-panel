use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::model::UserTunnel;
use crate::repo::user_tunnel as repo;
use crate::service::forward::{self, CurrentUser, ForwardUpdateReq};
use crate::service::tunnel as tunnel_service;
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserTunnelAssignReq {
    #[serde(default)]
    pub user_id: i64,
    #[serde(default)]
    pub tunnel_id: i64,
    #[serde(default)]
    pub flow: i64,
    #[serde(default)]
    pub num: i64,
    #[serde(default)]
    pub flow_reset_time: i64,
    #[serde(default)]
    pub exp_time: i64,
    #[serde(default)]
    pub speed_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserTunnelListReq {
    #[serde(default)]
    pub user_id: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserTunnelUpdateReq {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub flow: i64,
    #[serde(default)]
    pub num: i64,
    #[serde(default)]
    pub flow_reset_time: i64,
    #[serde(default)]
    pub exp_time: i64,
    #[serde(default)]
    pub status: i32,
    #[serde(default)]
    pub speed_id: Option<i64>,
}

pub async fn assign(state: &AppState, req: UserTunnelAssignReq) -> AppResult<()> {
    if req.user_id == 0 {
        return Err(AppError::new("用户ID不能为空"));
    }
    if req.tunnel_id == 0 {
        return Err(AppError::new("隧道ID不能为空"));
    }
    if req.flow < 0 {
        return Err(AppError::new("流量限制不能小于0"));
    }
    if req.num < 0 {
        return Err(AppError::new("转发数量不能小于0"));
    }
    let cnt = repo::count_by_user_and_tunnel(&state.db, req.user_id, req.tunnel_id).await?;
    if cnt > 0 {
        return Err(AppError::new("该用户已拥有此隧道权限"));
    }
    let mut ut = UserTunnel {
        user_id: req.user_id,
        tunnel_id: req.tunnel_id,
        speed_id: req.speed_id,
        num: req.num,
        flow: req.flow,
        flow_reset_time: req.flow_reset_time,
        exp_time: req.exp_time,
        status: 1,
        ..Default::default()
    };
    repo::insert(&state.db, &mut ut).await
}

pub async fn list(
    state: &AppState,
    user_id: i64,
) -> AppResult<Vec<repo::UserTunnelDetail>> {
    if user_id == 0 {
        return Err(AppError::new("用户ID不能为空"));
    }
    repo::list_details_by_user_id(&state.db, user_id).await
}

pub async fn remove(state: &AppState, id: i64) -> AppResult<()> {
    if id == 0 {
        return Err(AppError::new("ID不能为空"));
    }
    let Some(ut) = repo::find_by_id(&state.db, id).await? else {
        return Err(AppError::new("未找到对应的用户隧道权限记录"));
    };

    // 删除该用户在此隧道下的所有转发
    let forwards =
        tunnel_service::list_forwards_by_user_and_tunnel(&state.db, ut.user_id, ut.tunnel_id)
            .await?;
    for fw in &forwards {
        let admin = CurrentUser {
            user_id: 0,
            role_id: 0,
            user_name: String::new(),
        };
        if forward::delete(state, admin, fw.id).await.is_err() {
            // 尽力清理
            let _ = tunnel_service::delete_forward_port_by_forward_id(&state.db, fw.id).await;
            let _ = tunnel_service::delete_forward_by_id(&state.db, fw.id).await;
        }
    }
    repo::delete(&state.db, id).await
}

pub async fn update(state: &AppState, req: UserTunnelUpdateReq) -> AppResult<()> {
    if req.id == 0 {
        return Err(AppError::new("用户隧道权限ID不能为空"));
    }
    if req.flow < 0 {
        return Err(AppError::new("流量限制不能小于0"));
    }
    if req.num < 0 {
        return Err(AppError::new("转发数量不能小于0"));
    }
    let Some(mut ut) = repo::find_by_id(&state.db, req.id).await? else {
        return Err(AppError::new("隧道不存在"));
    };

    let speed_changed = has_speed_changed(ut.speed_id, req.speed_id);
    ut.flow = req.flow;
    ut.num = req.num;
    ut.flow_reset_time = req.flow_reset_time;
    ut.exp_time = req.exp_time;
    ut.status = req.status;
    ut.speed_id = req.speed_id;

    repo::update(&state.db, &ut).await?;

    // speed 变化 → 重建 forward 服务（对齐 Java updateForward）
    if speed_changed {
        let forwards =
            tunnel_service::list_forwards_by_user_and_tunnel(&state.db, ut.user_id, ut.tunnel_id)
                .await?;
        for fw in &forwards {
            let upd = ForwardUpdateReq {
                id: fw.id,
                user_id: fw.user_id,
                name: fw.name.clone(),
                remote_addr: fw.remote_addr.clone(),
                strategy: fw.strategy.clone(),
                // 不传 TunnelID → 走同隧道更新路径（会重建 limiter）
                tunnel_id: None,
                in_port: None,
            };
            let admin = CurrentUser {
                user_id: 0,
                role_id: 0,
                user_name: String::new(),
            };
            // 不阻断主更新；记录但继续
            let _ = forward::update(state, admin, upd).await;
        }
    }
    // Java 这里恒返回 err，Go 修 bug：成功返回 nil
    Ok(())
}

fn has_speed_changed(old_id: Option<i64>, new_id: Option<i64>) -> bool {
    match (old_id, new_id) {
        (None, None) => false,
        (Some(a), Some(b)) => a != b,
        _ => true,
    }
}
