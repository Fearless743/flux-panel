//! plugin handler / handler runner。
//!
//! 与 Go 版 `x/plugin/handler.go` 对齐：
//! 节点端 spawn 一个 plugin 子进程，并通过 stdin/stdout 与其交换 JSON 行。

use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

use super::{PluginRequest, PluginResponse};

/// Plugin handler 子进程。
pub struct PluginHandler {
    cmd: Command,
    child: Option<Child>,
}

impl PluginHandler {
    /// 用可执行文件路径构造（plugin 启动命令）。
    pub fn new(exe: impl Into<String>) -> Self {
        let mut cmd = Command::new(exe.into());
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Self { cmd, child: None }
    }

    /// 启动子进程。
    pub async fn start(&mut self) -> Result<()> {
        let child = self
            .cmd
            .spawn()
            .context("failed to spawn plugin subprocess")?;
        self.child = Some(child);
        Ok(())
    }

    /// 发送请求 + 读取一行响应。
    pub async fn call(&mut self, req: &PluginRequest) -> Result<PluginResponse> {
        let child = self.child.as_mut().context("plugin not started")?;
        let stdin = child.stdin.as_mut().context("plugin stdin missing")?;
        let stdout = child.stdout.as_mut().context("plugin stdout missing")?;
        let mut reader = BufReader::new(stdout);

        let json = serde_json::to_string(req)? + "\n";
        stdin.write_all(json.as_bytes()).await?;
        stdin.flush().await?;

        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            anyhow::bail!("plugin closed stdout");
        }
        let resp: PluginResponse = serde_json::from_str(&line)?;
        Ok(resp)
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill().await;
        }
        Ok(())
    }
}

/// plugin handler runner：阻塞地从 stdin 读请求行，写到 stdout。
///
/// 用于实现 gost 风格 plugin 子进程（gost.x/plugin/handler.go 对端）。
pub async fn run_handler_plugin() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut lines = BufReader::new(stdin).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.is_empty() {
            continue;
        }
        let resp = match serde_json::from_str::<PluginRequest>(&line) {
            Ok(_req) => PluginResponse::err("no plugin handler registered (stub)"),
            Err(e) => PluginResponse::err(e),
        };
        let out = serde_json::to_string(&resp)? + "\n";
        stdout.write_all(out.as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_response_ok() {
        let r = PluginResponse::ok(serde_json::json!({"foo": 1}));
        assert!(r.ok);
        assert!(r.data.is_some());
        assert!(r.error.is_none());
    }

    #[test]
    fn build_response_err() {
        let r = PluginResponse::err("boom");
        assert!(!r.ok);
        assert_eq!(r.error.as_deref(), Some("boom"));
    }
}
