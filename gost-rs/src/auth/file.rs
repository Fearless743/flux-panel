//! auth/file：基于 htpasswd 文件的鉴权后端。

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use super::Authenticator;

/// htpasswd 后端（内存映射）。
#[derive(Default, Clone)]
pub struct FileAuth {
    inner: Arc<RwLock<HashMap<String, String>>>,
}

impl FileAuth {
    pub fn from_str(content: &str) -> anyhow::Result<Self> {
        let mut map = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // 简化：user:pass 平文（生产应支持 crypt/APR1 哈希）
            if let Some((u, p)) = line.split_once(':') {
                map.insert(u.to_string(), p.to_string());
            }
        }
        Ok(Self {
            inner: Arc::new(RwLock::new(map)),
        })
    }
}

impl Authenticator for FileAuth {
    fn authenticate(&self, user: &str, pass: &str) -> bool {
        let r = self.inner.read();
        r.get(user).is_some_and(|p| p == pass)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_user() {
        let auth = FileAuth::from_str("alice:secret\nbob:12345").unwrap();
        assert!(auth.authenticate("alice", "secret"));
        assert!(auth.authenticate("bob", "12345"));
        assert!(!auth.authenticate("alice", "wrong"));
        assert!(!auth.authenticate("eve", "secret"));
    }
}
