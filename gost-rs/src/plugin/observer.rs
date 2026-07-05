//! plugin observer stub。

use crate::plugin::PluginResponse;

pub fn observe(_event: serde_json::Value) -> PluginResponse {
    PluginResponse::err("plugin observer not registered (stub)")
}
