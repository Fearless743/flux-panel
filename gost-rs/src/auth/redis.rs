//! auth/redis stub：通过 Redis HGET 校验。

use super::Authenticator;

pub struct RedisAuth;

impl RedisAuth {
    pub fn new() -> Self {
        Self
    }
}

impl Authenticator for RedisAuth {
    fn authenticate(&self, _user: &str, _pass: &str) -> bool {
        false
    }
}

impl Default for RedisAuth {
    fn default() -> Self {
        Self::new()
    }
}
