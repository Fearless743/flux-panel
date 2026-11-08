use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::repo::user as user_repo;
use crate::response;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub role_id: i32,
    pub name: String,
    pub token: String,
}

/// 兼容两种写法：`Authorization: <token>`（历史约定）与标准 `Bearer <token>`。
pub fn extract_token(raw: &str) -> &str {
    let t = raw.trim();
    if t.len() > 7 && t[..6].eq_ignore_ascii_case("bearer") && t[6..7].chars().all(|c| c == ' ' || c == '\t') {
        t[7..].trim()
    } else {
        t
    }
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let raw = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let token = extract_token(&raw).to_string();
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
    // DB 二次确认：用户存在、未停用、角色与签发时一致；
    // 用户资料更新（含改密/改权/停用）后 updated_time 推进，旧 token 自动失效。
    let user = match user_repo::find_by_id(&state.db, uid).await {
        Ok(Some(u)) => u,
        _ => return response::unauthorized("无效的token或token已过期").into_response(),
    };
    if user.status != 1 {
        return response::unauthorized("账号被停用").into_response();
    }
    if user.role_id != claims.role_id {
        return response::unauthorized("无效的token或token已过期").into_response();
    }
    if let Some(updated) = user.updated_time {
        // 允许 2s 时钟/精度偏差；密码/权限变更后旧 token 照样失效
        if updated > claims.iat * 1000 + 2000 {
            return response::unauthorized("无效的token或token已过期").into_response();
        }
    }
    req.extensions_mut().insert(AuthUser {
        user_id: uid,
        role_id: user.role_id,
        name: user.user.clone(),
        token,
    });
    next.run(req).await
}

pub async fn admin_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let Some(user) = req.extensions().get::<AuthUser>().cloned() else {
        return response::unauthorized("无法获取用户权限信息").into_response();
    };
    if user.role_id != 0 {
        return response::forbidden("无权限访问").into_response();
    }
    // 管理接口再查一次 DB，防止降权后旧 token 仍可用
    match user_repo::find_by_id(&state.db, user.user_id).await {
        Ok(Some(u)) if u.role_id == 0 && u.status == 1 => next.run(req).await,
        _ => response::forbidden("无权限访问").into_response(),
    }
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
