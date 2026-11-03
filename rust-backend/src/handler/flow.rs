use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::service::flow as svc;
use crate::state::AppState;

/// FlowTest 返回纯文本 "test"
pub async fn flow_test() -> &'static str {
    "test"
}

#[derive(Debug, Default, Deserialize)]
pub struct SecretQuery {
    #[serde(default)]
    pub secret: String,
}

/// FlowUpload ?secret= 解密 JSON 数组并计费
pub async fn flow_upload(
    State(state): State<AppState>,
    Query(q): Query<SecretQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let mut secret = q.secret;
    let mut raw: Vec<u8> = body.to_vec();
    if secret.is_empty() {
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if let Some(s) = secret_from_form(content_type, &body) {
            secret = s;
            // 与 Go `c.PostForm` 行为一致：读取表单后原始 body 已被消费
            raw.clear();
        }
    }

    if !svc::is_valid_node(&state, &secret).await {
        return "ok".into_response();
    }

    let plain = match svc::decrypt_if_needed(&raw, &secret) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(err = %e, "flow upload decrypt");
            return "ok".into_response();
        }
    };

    let items: Vec<svc::FlowDto> = match serde_json::from_slice(&plain) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(err = %e, "flow upload parse");
            return "ok".into_response();
        }
    };
    tracing::info!(count = items.len(), "节点上报流量数据");
    svc::upload(&state, items).await;
    "ok".into_response()
}

/// FlowConfig ?secret= 解密配置并异步 CleanNodeConfigs
pub async fn flow_config(
    State(state): State<AppState>,
    Query(q): Query<SecretQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let mut secret = q.secret;
    let mut raw: Vec<u8> = body.to_vec();
    if secret.is_empty() {
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if let Some(s) = secret_from_form(content_type, &body) {
            secret = s;
            raw.clear();
        }
    }

    let node = match svc::find_node_by_secret(&state, &secret).await {
        Ok(Some(n)) => n,
        _ => return "ok".into_response(),
    };

    let plain = match svc::decrypt_if_needed(&raw, &secret) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(err = %e, "flow config decrypt");
            return "ok".into_response();
        }
    };

    let cfg: svc::GostConfigDto = match serde_json::from_slice(&plain) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(err = %e, node = node.id, "flow config parse");
            return "ok".into_response();
        }
    };

    let task_state = state.clone();
    let node_id = node.id;
    tokio::spawn(async move {
        svc::clean_node_configs(&task_state, node_id, cfg).await;
    });
    tracing::info!(node = node_id, "节点配置数据接收成功");
    "ok".into_response()
}

/// 从 query/form 中取 secret：支持 urlencoded 与 multipart 表单。
fn secret_from_form(content_type: &str, body: &[u8]) -> Option<String> {
    if content_type.starts_with("application/x-www-form-urlencoded") {
        let text = String::from_utf8_lossy(body);
        return form_value(&text, "secret");
    }
    if content_type.starts_with("multipart/form-data") {
        // 简易 multipart 解析：查找 name="secret" 后的 body 部分
        let text = String::from_utf8_lossy(body);
        let marker = "name=\"secret\"";
        let idx = text.find(marker)?;
        let rest = &text[idx + marker.len()..];
        // 跳过头部剩余行与空行
        let start = rest.find("\r\n\r\n").or_else(|| rest.find("\n\n"))?;
        let value_start = if rest[start..].starts_with("\r\n\r\n") {
            start + 4
        } else {
            start + 2
        };
        let rest = &rest[value_start..];
        let end = rest
            .find("\r\n--")
            .or_else(|| rest.find("\n--"))
            .unwrap_or(rest.len());
        return Some(rest[..end].to_string());
    }
    None
}

fn form_value(body: &str, key: &str) -> Option<String> {
    for pair in body.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        if percent_decode(k) == key {
            return Some(percent_decode(v));
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    out.push((hi * 16 + lo) as u8);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}
