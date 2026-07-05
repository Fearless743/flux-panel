//! plugin sd (service discovery) stub。

use crate::plugin::PluginResponse;

pub fn register(_kind: &str) -> PluginResponse {
    PluginResponse::err("plugin sd not registered (stub)")
}
