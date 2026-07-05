//! plugin authenticator stub。

use crate::plugin::PluginResponse;

pub fn authenticate(_user: &str, _pass: &str) -> PluginResponse {
    PluginResponse::err("plugin authenticator not registered (stub)")
}
