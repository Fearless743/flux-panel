//! auth/http：通过 HTTP POST 转发鉴权。
//!
//! 与 Go 版 `x/auth/http/authenticator.go` 对齐：
//! POST {auth_url} → 2xx = 接受，4xx/5xx = 拒绝。

use super::Authenticator;

pub struct HttpAuth {
    url: String,
    client: reqwest::Client,
}

impl HttpAuth {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: reqwest::Client::new(),
        }
    }
}

impl Authenticator for HttpAuth {
    fn authenticate(&self, _user: &str, _pass: &str) -> bool {
        // 阶段 8 stub：未真正发请求；
        // 真实实现：POST {url} with body=cred → 2xx = ok
        false
    }
}
