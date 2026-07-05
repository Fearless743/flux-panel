//! Unix Domain Socket listener。
//!
//! 与 Go 版 `x/listener/unix/listener.go` 对齐。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::net::UnixListener;
use tokio::sync::Notify;

use crate::core::{BoxedStream, Listener};

pub struct UnixListenerImpl {
    kind: &'static str,
    inner: UnixListener,
    addr: PathBuf,
    close_notify: Arc<Notify>,
    closed: Arc<AtomicBool>,
}

impl UnixListenerImpl {
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        let path = PathBuf::from(addr);
        // 若已存在 .sock 文件，先删除（与 Go 行为一致）
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        let inner = UnixListener::bind(&path)?;
        Ok(Self {
            kind: "unix",
            inner,
            addr: path,
            close_notify: Arc::new(Notify::new()),
            closed: Arc::new(AtomicBool::new(false)),
        })
    }
}

#[async_trait]
impl Listener for UnixListenerImpl {
    fn kind(&self) -> &'static str {
        self.kind
    }

    async fn accept(&self) -> std::io::Result<BoxedStream> {
        if self.closed.load(Ordering::Acquire) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "listener closed",
            ));
        }
        let accept_fut = self.inner.accept();
        tokio::select! {
            biased;
            _ = self.close_notify.notified() => {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "listener closed",
                ))
            }
            res = accept_fut => {
                let (stream, _) = res?;
                Ok(Box::new(stream))
            }
        }
    }

    async fn close(&self) -> std::io::Result<()> {
        self.closed.store(true, Ordering::Release);
        self.close_notify.notify_waiters();
        // 清理 sock 文件
        let _ = std::fs::remove_file(&self.addr);
        Ok(())
    }
}

#[cfg(test)]
#[cfg(not(target_os = "windows"))]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn unix_accept() {
        let dir = std::env::temp_dir().join(format!("gost-rs-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.sock");
        let l = UnixListenerImpl::bind(path.to_str().unwrap()).await.unwrap();

        let handle = tokio::spawn(async move { l.accept().await });
        let mut client = tokio::net::UnixStream::connect(&path).await.unwrap();
        client.write_all(b"hi").await.unwrap();
        let mut server = handle.await.unwrap().unwrap();
        let mut buf = [0u8; 2];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hi");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
