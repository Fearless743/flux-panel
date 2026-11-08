use axum::body::Bytes;
use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::response;
use crate::service::captcha as svc;
use crate::state::AppState;

pub async fn captcha_check(State(state): State<AppState>) -> Json<Value> {
    response::ok(json!(svc::check_enabled(&state.db).await))
}

pub async fn captcha_generate(headers: axum::http::HeaderMap) -> Json<Value> {
    if !crate::ratelimit::check_captcha(&crate::ratelimit::client_ip(&headers)) {
        return response::err("请求过于频繁，请稍后再试");
    }
    match svc::generate() {
        Ok(res) => response::ok(json!({
            "id": res.id,
            "captchaId": res.captcha_id,
            "captcha": res,
            "type": res.r#type,
            "captchaImage": res.captcha_image,
            "backgroundImage": res.background_image,
        })),
        Err(_) => response::err("生成验证码失败"),
    }
}

pub async fn captcha_verify(
    State(_state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Json<Value> {
    if !crate::ratelimit::check_captcha(&crate::ratelimit::client_ip(&headers)) {
        return response::err("请求过于频繁，请稍后再试");
    }
    let Ok(m) = serde_json::from_slice::<Value>(&body) else {
        return response::err("参数错误");
    };
    let id = first_string(&m, &["id", "captchaId"]);
    let mut code = first_string(&m, &["code", "answer", "value"]);
    if code.is_empty() {
        if let Some(data) = m.get("data") {
            match data {
                Value::String(s) => code = s.clone(),
                Value::Object(_) => {
                    code = first_string(data, &["code", "answer", "value", "data"])
                }
                _ => {}
            }
        }
    }
    if code.is_empty() {
        code = first_string(&m, &["trackData"]);
    }

    if !svc::verify(&id, &code) {
        return Json(json!({
            "code": 4001,
            "msg": "验证码校验失败",
            "success": false,
            "data": Value::Null,
        }));
    }
    Json(json!({
        "code": 200,
        "msg": "success",
        "success": true,
        "data": { "validToken": id },
    }))
}

fn first_string(m: &Value, keys: &[&str]) -> String {
    for k in keys {
        if let Some(s) = m.get(*k).and_then(Value::as_str) {
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    String::new()
}
