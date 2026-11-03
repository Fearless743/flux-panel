use axum::extract::State;
use axum::Json;
use serde_json::Value;

use crate::handler::extract::AppJson;
use crate::response;
use crate::service::backup as svc;
use crate::state::AppState;

/// 导出全部数据（管理员）
pub async fn backup_export(State(state): State<AppState>) -> Json<Value> {
    match svc::export(&state).await {
        Ok(data) => response::ok(serde_json::to_value(data).unwrap_or(Value::Null)),
        Err(e) => response::err(e.0),
    }
}

/// 全量覆盖导入（管理员，危险操作，前端需二次确认）
pub async fn backup_import(
    State(state): State<AppState>,
    AppJson(body): AppJson<Value>,
) -> Json<Value> {
    let data = match serde_json::from_value::<svc::BackupData>(body) {
        Ok(d) => d,
        Err(_) => return response::err("参数错误：不是有效的备份文件"),
    };
    match svc::import(&state, &data).await {
        Ok(()) => response::ok(Value::Null),
        Err(e) => response::err(e.0),
    }
}
