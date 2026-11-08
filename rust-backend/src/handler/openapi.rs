use axum::extract::{Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::response;
use crate::service::user as svc;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct OpenApiQuery {
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub pwd: String,
    #[serde(default = "default_tunnel")]
    pub tunnel: String,
}

fn default_tunnel() -> String {
    "-1".to_string()
}

pub async fn open_api_sub_store(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(q): Query<OpenApiQuery>,
) -> Response {
    if !crate::ratelimit::check_openapi(&crate::ratelimit::client_ip(&headers)) {
        return response::err("请求过于频繁，请稍后再试").into_response();
    }
    match svc::open_api_sub_store(&state, &q.user, &q.pwd, &q.tunnel).await {
        Ok(header_value) => {
            let mut res = (StatusCode::OK, header_value.clone()).into_response();
            if let Ok(v) = HeaderValue::from_str(&header_value) {
                res.headers_mut().insert("subscription-userinfo", v);
            }
            res.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            );
            res
        }
        Err(e) => response::err(e.0).into_response(),
    }
}
