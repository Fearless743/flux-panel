use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::handler::extract::AppJson;
use crate::response;
use crate::service::speed_limit as svc;
use crate::state::AppState;

/// POST /api/v1/speed-limit/create
pub async fn speed_limit_create(
    State(state): State<AppState>,
    AppJson(req): AppJson<svc::CreateSpeedLimitReq>,
) -> Json<Value> {
    match svc::create(&state, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

/// POST /api/v1/speed-limit/list
pub async fn speed_limit_list(State(state): State<AppState>) -> Json<Value> {
    match svc::list(&state).await {
        Ok(list) => response::ok(serde_json::to_value(list).unwrap_or(Value::Null)),
        Err(e) => response::err(e.0),
    }
}

/// POST /api/v1/speed-limit/update
pub async fn speed_limit_update(
    State(state): State<AppState>,
    AppJson(req): AppJson<svc::UpdateSpeedLimitReq>,
) -> Json<Value> {
    match svc::update(&state, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

/// POST /api/v1/speed-limit/delete
pub async fn speed_limit_delete(
    State(state): State<AppState>,
    AppJson(params): AppJson<Params>,
) -> Json<Value> {
    match svc::delete(&state, params.id).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

#[derive(Deserialize)]
pub struct Params {
    #[serde(default)]
    pub id: i64,
}

/// POST /api/v1/speed-limit/tunnels
pub async fn speed_limit_tunnels(State(state): State<AppState>) -> Json<Value> {
    match svc::list_tunnels(&state).await {
        Ok(list) => response::ok(serde_json::to_value(list).unwrap_or(Value::Null)),
        Err(e) => response::err(e.0),
    }
}
