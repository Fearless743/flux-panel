//! plugin hosts stub。

use crate::plugin::PluginResponse;

pub fn lookup(_host: &str) -> PluginResponse {
    PluginResponse::err("plugin hosts not registered (stub)")
}
