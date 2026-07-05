//! recorder/http：通过 HTTP POST 转发事件到面板 API。

use serde::Serialize;

pub struct HttpRecorder {
    url: String,
    client: reqwest::Client,
}

impl HttpRecorder {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn record<E: Serialize + Send + Sync>(&self, event: &E) -> anyhow::Result<()> {
        // 阶段 9 stub：dry call
        let _body = serde_json::to_string(event)?;
        let _ = self.url.as_str();
        Ok(())
    }
}
