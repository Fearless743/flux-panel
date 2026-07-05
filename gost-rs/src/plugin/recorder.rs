//! plugin recorder stub。

use crate::plugin::PluginResponse;

pub fn record(_event: serde_json::Value) -> PluginResponse {
    PluginResponse::err("plugin recorder not registered (stub)")
}
