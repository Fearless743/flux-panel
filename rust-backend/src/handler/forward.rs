use axum::body::Bytes;
use axum::extract::State;
use axum::Json;
use axum::Extension;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::handler::extract::AppJson;
use crate::middleware::AuthUser;
use crate::response;
use crate::service::forward as svc;
use crate::state::AppState;

fn current_user(user: AuthUser) -> svc::CurrentUser {
    svc::CurrentUser {
        user_id: user.user_id,
        role_id: user.role_id,
        user_name: user.name,
    }
}

pub async fn forward_create(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(req): AppJson<svc::ForwardCreateReq>,
) -> Json<Value> {
    match svc::create(&state, current_user(user), req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

/// 空 body（移动端/历史调用）走 legacy 全量数组返回，保持兼容；
/// 带分页/过滤字段时走高性能分页（COUNT + 单页 LIMIT/OFFSET + 页内填充）。
/// 空对象/解析失败同样回落 legacy，避免老客户端被误伤。
pub async fn forward_list(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    body: Bytes,
) -> Json<Value> {
    let user = current_user(user);
    if !body.is_empty() {
        if let Ok(req) = serde_json::from_slice::<svc::ForwardListReq>(&body) {
            if req.is_paged() {
                return match svc::list_paged(&state, &user, req).await {
                    Ok(page) => response::ok(json!(page)),
                    Err(e) => response::err(e.0),
                };
            }
        }
    }
    match svc::list(&state, &user).await {
        Ok(list) => response::ok(json!(list)),
        Err(e) => response::err(e.0),
    }
}

pub async fn forward_update(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(req): AppJson<svc::ForwardUpdateReq>,
) -> Json<Value> {
    match svc::update(&state, current_user(user), req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ForwardIdBody {
    #[serde(default)]
    id: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ForwardDiagnoseBody {
    #[serde(default, rename = "forwardId")]
    forward_id: Option<Value>,
    #[serde(default)]
    id: Option<Value>,
}

/// 对齐 Go asInt64：接受 JSON 数字、整数值浮点数以及纯数字字符串。
fn as_int64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => {
            if !s.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let mut n: i64 = 0;
            for c in s.chars() {
                n = n
                    .wrapping_mul(10)
                    .wrapping_add((c as u8 - b'0') as i64);
            }
            Some(n)
        }
        _ => None,
    }
}

pub async fn forward_delete(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(body): AppJson<ForwardIdBody>,
) -> Json<Value> {
    let Some(id) = as_int64(body.id.as_ref()) else {
        return response::err("转发ID不能为空");
    };
    if id == 0 {
        return response::err("转发ID不能为空");
    }
    match svc::delete(&state, current_user(user), id).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

pub async fn forward_force_delete(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(body): AppJson<ForwardIdBody>,
) -> Json<Value> {
    let Some(id) = as_int64(body.id.as_ref()) else {
        return response::err("转发ID不能为空");
    };
    if id == 0 {
        return response::err("转发ID不能为空");
    }
    match svc::force_delete(&state, current_user(user), id).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

pub async fn forward_pause(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(body): AppJson<ForwardIdBody>,
) -> Json<Value> {
    let Some(id) = as_int64(body.id.as_ref()) else {
        return response::err("转发ID不能为空");
    };
    if id == 0 {
        return response::err("转发ID不能为空");
    }
    match svc::pause(&state, current_user(user), id).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

pub async fn forward_resume(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(body): AppJson<ForwardIdBody>,
) -> Json<Value> {
    let Some(id) = as_int64(body.id.as_ref()) else {
        return response::err("转发ID不能为空");
    };
    if id == 0 {
        return response::err("转发ID不能为空");
    }
    match svc::resume(&state, current_user(user), id).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

pub async fn forward_diagnose(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(body): AppJson<ForwardDiagnoseBody>,
) -> Json<Value> {
    let id = match as_int64(body.forward_id.as_ref()) {
        Some(v) if v != 0 => Some(v),
        // 兼容 id 字段
        _ => as_int64(body.id.as_ref()),
    };
    let Some(id) = id else {
        return response::err("转发ID不能为空");
    };
    if id == 0 {
        return response::err("转发ID不能为空");
    }
    match svc::diagnose(&state, current_user(user), id).await {
        Ok(report) => response::ok(report),
        Err(e) => response::err(e.0),
    }
}

pub async fn forward_update_order(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(req): AppJson<svc::UpdateOrderReq>,
) -> Json<Value> {
    if req.forwards.is_empty() {
        return response::err("缺少forwards参数");
    }
    match svc::update_order(&state, current_user(user), req.forwards).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

pub async fn forward_batch_delete(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(req): AppJson<svc::BatchForwardReq>,
) -> Json<Value> {
    let (msg, ok) = svc::batch_delete(&state, current_user(user), req.ids).await;
    if !ok {
        response::err(msg)
    } else {
        response::ok_msg(&msg, Value::Null)
    }
}

pub async fn forward_batch_pause(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(req): AppJson<svc::BatchForwardReq>,
) -> Json<Value> {
    let (msg, ok) = svc::batch_pause(&state, current_user(user), req.ids).await;
    if !ok {
        response::err(msg)
    } else {
        response::ok_msg(&msg, Value::Null)
    }
}

pub async fn forward_batch_resume(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(req): AppJson<svc::BatchForwardReq>,
) -> Json<Value> {
    let (msg, ok) = svc::batch_resume(&state, current_user(user), req.ids).await;
    if !ok {
        response::err(msg)
    } else {
        response::ok_msg(&msg, Value::Null)
    }
}

pub async fn forward_batch_change_tunnel(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(req): AppJson<svc::BatchForwardReq>,
) -> Json<Value> {
    let tunnel_id = req.tunnel_id.unwrap_or(0);
    let (msg, ok) =
        svc::batch_change_tunnel(&state, current_user(user), req.ids, tunnel_id).await;
    if !ok {
        response::err(msg)
    } else {
        response::ok_msg(&msg, Value::Null)
    }
}
