use axum::extract::State;
use axum::{Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::handler::extract::AppJson;
use crate::middleware::AuthUser;
use crate::response;
use crate::service::tunnel as svc;
use crate::service::user_tunnel as user_tunnel_svc;
use crate::state::AppState;

// TunnelCreate POST /api/v1/tunnel/create
pub async fn tunnel_create(
    State(state): State<AppState>,
    AppJson(req): AppJson<svc::TunnelCreateReq>,
) -> Json<Value> {
    match svc::create(&state, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

// TunnelList POST /api/v1/tunnel/list
pub async fn tunnel_list(State(state): State<AppState>) -> Json<Value> {
    match svc::list(&state).await {
        Ok(list) => response::ok(json!(list)),
        Err(e) => response::err(e.0),
    }
}

// TunnelUpdate POST /api/v1/tunnel/update
pub async fn tunnel_update(
    State(state): State<AppState>,
    AppJson(req): AppJson<svc::TunnelUpdateReq>,
) -> Json<Value> {
    match svc::update(&state, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

#[derive(Deserialize)]
pub struct TunnelDeleteBody {
    #[serde(default)]
    pub id: i64,
}

// TunnelDelete POST /api/v1/tunnel/delete
pub async fn tunnel_delete(
    State(state): State<AppState>,
    AppJson(body): AppJson<TunnelDeleteBody>,
) -> Json<Value> {
    if body.id == 0 {
        return response::err("参数错误");
    }
    match svc::delete(&state, body.id).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

#[derive(Deserialize)]
pub struct TunnelDiagnoseBody {
    #[serde(default, rename = "tunnelId")]
    pub tunnel_id: i64,
}

// TunnelDiagnose POST /api/v1/tunnel/diagnose
pub async fn tunnel_diagnose(
    State(state): State<AppState>,
    AppJson(body): AppJson<TunnelDiagnoseBody>,
) -> Json<Value> {
    if body.tunnel_id == 0 {
        return response::err("参数错误");
    }
    match svc::diagnose(&state, body.tunnel_id).await {
        Ok(report) => response::ok(report),
        Err(e) => response::err(e.0),
    }
}

// UserTunnelAssign POST /api/v1/tunnel/user/assign
pub async fn user_tunnel_assign(
    State(state): State<AppState>,
    AppJson(req): AppJson<user_tunnel_svc::UserTunnelAssignReq>,
) -> Json<Value> {
    match user_tunnel_svc::assign(&state, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

// UserTunnelList POST /api/v1/tunnel/user/list
pub async fn user_tunnel_list(
    State(state): State<AppState>,
    AppJson(req): AppJson<user_tunnel_svc::UserTunnelListReq>,
) -> Json<Value> {
    match user_tunnel_svc::list(&state, req.user_id).await {
        Ok(list) => response::ok(json!(list)),
        Err(e) => response::err(e.0),
    }
}

#[derive(Deserialize)]
pub struct UserTunnelRemoveBody {
    #[serde(default)]
    pub id: i64,
}

// UserTunnelRemove POST /api/v1/tunnel/user/remove
pub async fn user_tunnel_remove(
    State(state): State<AppState>,
    AppJson(body): AppJson<UserTunnelRemoveBody>,
) -> Json<Value> {
    if body.id == 0 {
        return response::err("参数错误");
    }
    match user_tunnel_svc::remove(&state, body.id).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

// UserTunnelUpdate POST /api/v1/tunnel/user/update
// 注意：成功返回 ok（Java 恒 err 是 bug）
pub async fn user_tunnel_update(
    State(state): State<AppState>,
    AppJson(req): AppJson<user_tunnel_svc::UserTunnelUpdateReq>,
) -> Json<Value> {
    match user_tunnel_svc::update(&state, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

// UserTunnelMine POST /api/v1/tunnel/user/tunnel
pub async fn user_tunnel_mine(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Json<Value> {
    match svc::mine(&state, user.user_id, user.role_id).await {
        Ok(list) => response::ok(json!(list)),
        Err(e) => response::err(e.0),
    }
}
