//! auth/plugin stub：调 gost 风格 plugin 子进程做鉴权。

use std::collections::HashMap;

use parking_lot::Mutex;

use super::Authenticator;

pub struct PluginAuth {
    inner: Mutex<HashMap<String, String>>,
}

impl PluginAuth {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }
}

impl Authenticator for PluginAuth {
    fn authenticate(&self, _user: &str, _pass: &str) -> bool {
        // 阶段 8 stub：没有注册 plugin handler 时拒绝
        false
    }
}

impl Default for PluginAuth {
    fn default() -> Self {
        Self::new()
    }
}
