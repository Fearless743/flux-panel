use axum::extract::{FromRequest, Request};
use axum::Json;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::response;

/// 与 Go `ShouldBindJSON` 失败时的行为对齐：HTTP 200 + {code:-1,msg:"参数错误"}
pub struct AppJson<T>(pub T);

impl<S, T> FromRequest<S> for AppJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Json<Value>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(v)) => Ok(AppJson(v)),
            Err(_) => Err(response::err("参数错误")),
        }
    }
}
