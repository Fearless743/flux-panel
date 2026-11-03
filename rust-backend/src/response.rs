use axum::Json;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn ok(data: Value) -> Json<Value> {
    Json(json!({
        "code": 0,
        "msg": "操作成功",
        "ts": now_ms(),
        "data": data,
    }))
}

pub fn ok_msg(msg: &str, data: Value) -> Json<Value> {
    Json(json!({
        "code": 0,
        "msg": msg,
        "ts": now_ms(),
        "data": data,
    }))
}

pub fn err(msg: impl Into<String>) -> Json<Value> {
    Json(json!({
        "code": -1,
        "msg": msg.into(),
        "ts": now_ms(),
        "data": Value::Null,
    }))
}

pub fn err_code(code: i32, msg: impl Into<String>) -> Json<Value> {
    Json(json!({
        "code": code,
        "msg": msg.into(),
        "ts": now_ms(),
        "data": Value::Null,
    }))
}

pub fn unauthorized(msg: impl Into<String>) -> Json<Value> {
    err_code(401, msg)
}

pub fn forbidden(msg: impl Into<String>) -> Json<Value> {
    err_code(403, msg)
}
