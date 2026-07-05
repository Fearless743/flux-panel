//! recorder/redis stub。

pub struct RedisRecorder;

impl RedisRecorder {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RedisRecorder {
    fn default() -> Self {
        Self::new()
    }
}
