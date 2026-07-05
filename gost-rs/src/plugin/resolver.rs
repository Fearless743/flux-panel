//! plugin resolver stub。

use crate::plugin::PluginResponse;

pub fn resolve(_host: &str) -> PluginResponse {
    PluginResponse::err("plugin resolver not registered (stub)")
}
