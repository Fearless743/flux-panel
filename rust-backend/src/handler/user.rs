use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::handler::extract::AppJson;
use crate::middleware::AuthUser;
use crate::response;
use crate::service::user as svc;
use crate::state::AppState;
use axum::Extension;

pub async fn login(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AppJson(req): AppJson<svc::LoginReq>,
) -> Json<Value> {
    let ip = crate::ratelimit::client_ip(&headers);
    if !crate::ratelimit::check_login(&ip) {
        return response::err("登录尝试过于频繁，请稍后再试");
    }
    match svc::login(&state, req).await {
        Ok(resp) => response::ok(json!(resp)),
        Err(e) => response::err(e.0),
    }
}

pub async fn user_create(
    State(state): State<AppState>,
    AppJson(req): AppJson<svc::CreateUserReq>,
) -> Json<Value> {
    match svc::create(&state, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

pub async fn user_list(State(state): State<AppState>) -> Json<Value> {
    match svc::list(&state).await {
        Ok(list) => response::ok(serde_json::to_value(list).unwrap_or(Value::Null)),
        Err(e) => response::err(e.0),
    }
}

pub async fn user_update(
    State(state): State<AppState>,
    AppJson(req): AppJson<svc::UpdateUserReq>,
) -> Json<Value> {
    match svc::update(&state, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

#[derive(Deserialize)]
pub struct IdBody {
    #[serde(default)]
    pub id: i64,
}

pub async fn user_delete(
    State(state): State<AppState>,
    AppJson(body): AppJson<IdBody>,
) -> Json<Value> {
    if body.id == 0 {
        return response::err("参数错误");
    }
    match svc::delete(&state, body.id).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

pub async fn user_package(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Json<Value> {
    match svc::package(&state, user.user_id).await {
        Ok(pkg) => response::ok(json!(pkg)),
        Err(e) => response::err(e.0),
    }
}

pub async fn user_update_password(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    AppJson(req): AppJson<svc::ChangePasswordReq>,
) -> Json<Value> {
    match svc::update_password(&state, user.user_id, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

pub async fn user_reset(
    State(state): State<AppState>,
    AppJson(req): AppJson<svc::ResetFlowReq>,
) -> Json<Value> {
    match svc::reset(&state, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}
