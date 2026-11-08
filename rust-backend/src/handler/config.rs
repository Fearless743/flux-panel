use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::handler::extract::AppJson;
use crate::response;
use crate::service::config as svc;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct NameBody {
    #[serde(default)]
    pub name: String,
}

pub async fn config_get(
    State(state): State<AppState>,
    AppJson(body): AppJson<NameBody>,
) -> Json<Value> {
    match svc::get_public_by_name(&state.db, &body.name).await {
        Ok(cfg) => response::ok(json!(cfg)),
        Err(e) => response::err(e.0),
    }
}

pub async fn config_list(State(state): State<AppState>) -> Json<Value> {
    match svc::list_map(&state.db).await {
        Ok(m) => response::ok(json!(m)),
        Err(e) => response::err(e.0),
    }
}

pub async fn config_update(
    State(state): State<AppState>,
    AppJson(body): AppJson<HashMap<String, String>>,
) -> Json<Value> {
    match svc::update_map(&state.db, body).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}

#[derive(Deserialize)]
pub struct SingleBody {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
}

pub async fn config_update_single(
    State(state): State<AppState>,
    AppJson(body): AppJson<SingleBody>,
) -> Json<Value> {
    match svc::update_single(&state.db, &body.name, &body.value).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}
