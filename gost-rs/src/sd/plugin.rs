//! sd/plugin stub。

pub struct PluginSD;

impl PluginSD {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PluginSD {
    fn default() -> Self {
        Self::new()
    }
}
