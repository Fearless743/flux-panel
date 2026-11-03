use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::response;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub role_id: i32,
    pub name: String,
    pub token: String,
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if token.is_empty() {
        return response::unauthorized("未登录或token已过期").into_response();
    }
    let claims = match state.jwt.parse(&token) {
        Ok(c) => c,
        Err(_) => return response::unauthorized("无效的token或token已过期").into_response(),
    };
    let Ok(uid) = claims.sub.parse::<i64>() else {
        return response::unauthorized("无效的token或token已过期").into_response();
    };
    req.extensions_mut().insert(AuthUser {
        user_id: uid,
        role_id: claims.role_id,
        name: claims.name,
        token,
    });
    next.run(req).await
}

pub async fn admin_middleware(req: Request, next: Next) -> Response {
    let Some(user) = req.extensions().get::<AuthUser>().cloned() else {
        return response::unauthorized("无法获取用户权限信息").into_response();
    };
    if user.role_id != 0 {
        return response::forbidden("无权限访问").into_response();
    }
    next.run(req).await
}

pub async fn cors_middleware(req: Request, next: Next) -> Response {
    if req.method() == Method::OPTIONS {
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::NO_CONTENT;
        add_cors_headers(res.headers_mut());
        return res;
    }
    let mut res = next.run(req).await;
    add_cors_headers(res.headers_mut());
    res
}

fn add_cors_headers(headers: &mut axum::http::HeaderMap) {
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Origin, Content-Type, Accept, Authorization"),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("Authorization"),
    );
}
