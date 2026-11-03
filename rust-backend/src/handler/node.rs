use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::handler::extract::AppJson;
use crate::response;
use crate::service::node as svc;
use crate::state::AppState;

pub async fn node_create(
    State(state): State<AppState>,
    AppJson(req): AppJson<svc::NodeCreateReq>,
) -> Json<Value> {
    match svc::create(&state, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

pub async fn node_list(State(state): State<AppState>) -> Json<Value> {
    match svc::list(&state).await {
        Ok(list) => response::ok(serde_json::to_value(list).unwrap_or(Value::Null)),
        Err(e) => response::err(e.0),
    }
}

pub async fn node_update(
    State(state): State<AppState>,
    AppJson(req): AppJson<svc::NodeUpdateReq>,
) -> Json<Value> {
    match svc::update(&state, req).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

pub async fn node_delete(
    State(state): State<AppState>,
    AppJson(params): AppJson<Value>,
) -> Json<Value> {
    let Some(id) = params.get("id").and_then(as_int64) else {
        return response::err("节点ID不能为空");
    };
    if id == 0 {
        return response::err("节点ID不能为空");
    }
    match svc::delete(&state, id).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

pub async fn node_install(
    State(state): State<AppState>,
    AppJson(params): AppJson<Value>,
) -> Json<Value> {
    let Some(id) = params.get("id").and_then(as_int64) else {
        return response::err("节点ID不能为空");
    };
    if id == 0 {
        return response::err("节点ID不能为空");
    }
    match svc::install_command(&state, id).await {
        Ok(cmd) => response::ok(json!(cmd)),
        Err(e) => response::err(e.0),
    }
}

/// POST /node/upgrade 远程升级在线节点二进制
pub async fn node_upgrade(
    State(state): State<AppState>,
    AppJson(req): AppJson<svc::NodeUpgradeReq>,
) -> Json<Value> {
    match svc::upgrade(&state, req).await {
        Ok(msg) => response::ok(json!(msg)),
        Err(e) => response::err(e.0),
    }
}

fn as_int64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(i)
            } else {
                n.as_f64().map(|f| f as i64)
            }
        }
        Value::String(s) => {
            let mut out: i64 = 0;
            for ch in s.chars() {
                if !ch.is_ascii_digit() {
                    return None;
                }
                out = out * 10 + (ch as i64 - '0' as i64);
            }
            Some(out)
        }
        _ => None,
    }
}
