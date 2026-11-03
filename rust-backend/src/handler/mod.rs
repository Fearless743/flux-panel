pub mod backup;
pub mod captcha;
pub mod config;
pub mod extract;
pub mod flow;
pub mod forward;
pub mod node;
pub mod openapi;
pub mod speed_limit;
pub mod tunnel;
pub mod user;
pub mod websocket;

use axum::routing::{any, get, post};
use axum::{middleware, Router};
use tower_http::catch_panic::CatchPanicLayer;

use crate::middleware::{admin_middleware, auth_middleware, cors_middleware};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let public = Router::new()
        .route("/user/login", post(user::login))
        .route("/captcha/check", post(captcha::captcha_check))
        .route("/captcha/generate", post(captcha::captcha_generate))
        .route("/captcha/verify", post(captcha::captcha_verify))
        .route("/config/get", post(config::config_get))
        .route("/open_api/sub_store", get(openapi::open_api_sub_store));

    let admin = Router::new()
        .route("/user/create", post(user::user_create))
        .route("/user/list", post(user::user_list))
        .route("/user/update", post(user::user_update))
        .route("/user/delete", post(user::user_delete))
        .route("/user/reset", post(user::user_reset))
        .route("/config/list", post(config::config_list))
        .route("/config/update", post(config::config_update))
        .route(
            "/config/update-single",
            post(config::config_update_single),
        )
        .route("/node/create", post(node::node_create))
        .route("/node/list", post(node::node_list))
        .route("/node/update", post(node::node_update))
        .route("/node/delete", post(node::node_delete))
        .route("/node/install", post(node::node_install))
        .route("/node/upgrade", post(node::node_upgrade))
        .route("/speed-limit/create", post(speed_limit::speed_limit_create))
        .route("/speed-limit/list", post(speed_limit::speed_limit_list))
        .route("/speed-limit/update", post(speed_limit::speed_limit_update))
        .route("/speed-limit/delete", post(speed_limit::speed_limit_delete))
        .route(
            "/speed-limit/tunnels",
            post(speed_limit::speed_limit_tunnels),
        )
        .route("/backup/export", post(backup::backup_export))
        .route("/backup/import", post(backup::backup_import))
        .route("/tunnel/create", post(tunnel::tunnel_create))
        .route("/tunnel/list", post(tunnel::tunnel_list))
        .route("/tunnel/update", post(tunnel::tunnel_update))
        .route("/tunnel/delete", post(tunnel::tunnel_delete))
        .route("/tunnel/diagnose", post(tunnel::tunnel_diagnose))
        .route("/tunnel/user/assign", post(tunnel::user_tunnel_assign))
        .route("/tunnel/user/list", post(tunnel::user_tunnel_list))
        .route("/tunnel/user/remove", post(tunnel::user_tunnel_remove))
        .route("/tunnel/user/update", post(tunnel::user_tunnel_update))
        .route_layer(middleware::from_fn(admin_middleware));

    let auth = Router::new()
        .route("/user/package", post(user::user_package))
        .route("/user/updatePassword", post(user::user_update_password))
        .route("/tunnel/user/tunnel", post(tunnel::user_tunnel_mine))
        .route("/forward/create", post(forward::forward_create))
        .route("/forward/list", post(forward::forward_list))
        .route("/forward/update", post(forward::forward_update))
        .route("/forward/delete", post(forward::forward_delete))
        .route(
            "/forward/force-delete",
            post(forward::forward_force_delete),
        )
        .route("/forward/pause", post(forward::forward_pause))
        .route("/forward/resume", post(forward::forward_resume))
        .route("/forward/diagnose", post(forward::forward_diagnose))
        .route(
            "/forward/update-order",
            post(forward::forward_update_order),
        )
        .route(
            "/forward/batch-delete",
            post(forward::forward_batch_delete),
        )
        .route(
            "/forward/batch-pause",
            post(forward::forward_batch_pause),
        )
        .route(
            "/forward/batch-resume",
            post(forward::forward_batch_resume),
        )
        .route(
            "/forward/batch-change-tunnel",
            post(forward::forward_batch_change_tunnel),
        )
        .merge(admin)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .route("/flow/test", any(flow::flow_test))
        .route("/flow/upload", post(flow::flow_upload))
        .route("/flow/config", post(flow::flow_config))
        .route("/system-info", get(websocket::handle_websocket))
        .nest("/api/v1", public.merge(auth))
        .with_state(state)
        .layer(middleware::from_fn(cors_middleware))
        .layer(CatchPanicLayer::new())
}
