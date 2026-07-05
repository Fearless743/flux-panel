//! recorder/plugin stub。

pub struct PluginRecorder;

impl PluginRecorder {
    pub fn new() -> Self {
        Self
    }

    pub async fn record(&self, _event: serde_json::Value) -> anyhow::Result<()> {
        Ok(())
    }
}

impl Default for PluginRecorder {
    fn default() -> Self {
        Self::new()
    }
}
