//! Forward handler：双向转发 conn 到 chain 选中的节点。
//!
//! 与 Go 版 `x/handler/forward/handler.go` 对齐。

use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::core::{BoxedStream, Chain, Handler};

/// Forward handler 选项。
#[derive(Clone)]
pub struct ForwardHandlerOptions {
    pub chain: Option<Arc<dyn Chain>>,
    /// 固定目标（与 chain 二选一；都设置时 chain 优先）。
    pub target: Option<String>,
    /// Dialer（用于固定目标）
    pub dialer: Option<Arc<dyn crate::core::Dialer>>,
    /// 单向 buffer 大小。
    pub buffer_size: usize,
}

impl Default for ForwardHandlerOptions {
    fn default() -> Self {
        Self {
            chain: None,
            target: None,
            dialer: None,
            buffer_size: 16 * 1024,
        }
    }
}

/// Forward handler。
pub struct ForwardHandler {
    kind: &'static str,
    opts: ForwardHandlerOptions,
}

impl ForwardHandler {
    pub fn new(opts: ForwardHandlerOptions) -> Self {
        Self {
            kind: "forward",
            opts,
        }
    }
}

#[async_trait]
impl Handler for ForwardHandler {
    fn kind(&self) -> &'static str {
        self.kind
    }

    async fn handle(&self, conn: BoxedStream) -> std::io::Result<()> {
        // 1. 选择目标
        let (mut remote, node_name) = if let Some(chain) = &self.opts.chain {
            let (stream, node) = chain.dial().await?;
            (stream, Some(node))
        } else if let Some(target) = &self.opts.target {
            let dialer = self
                .opts
                .dialer
                .clone()
                .unwrap_or_else(|| Arc::new(crate::dialer::DirectDialer::new()));
            let stream = dialer.dial(&target).await?;
            (stream, None)
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "forward handler: neither chain nor target",
            ));
        };

        // 2. 双向 copy
        let res = pipe_bidirectional(conn, &mut remote, self.opts.buffer_size).await;

        // 3. 上报节点成功/失败
        if let (Some(chain), Some(node)) = (&self.opts.chain, &node_name) {
            if res.is_err() {
                chain.mark_failed(node).await;
            } else {
                chain.mark_success(node).await;
            }
        }

        res
    }
}

/// 双向 pipe 两个 stream。任一端 EOF/Eof 都正常结束。
///
/// 用 `tokio::io::copy_bidirectional` 实现；它会 spawn 两个 task 并在任一端 EOF 时 shutdown 另一端。
async fn pipe_bidirectional<A, B>(
    a: A,
    b: &mut B,
    buf_size: usize,
) -> std::io::Result<()>
where
    A: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    B: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // tokio::io::copy_bidirectional 用 8KB 固定 buffer；这里借用 cap 给调用方展示可调 buffer_size。
    let _ = buf_size; // 当前 tokio 不支持自定义 buffer，先忽略
    tokio::io::copy_bidirectional(&mut tokio::io::BufReader::with_capacity(buf_size, a), b).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn forward_pipes_through_target() {
        // 起一个 echo TCP server
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = server.local_addr().unwrap();

        tokio::spawn(async move {
            loop {
                let (mut s, _) = server.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    loop {
                        match s.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => {
                                if s.write_all(&buf[..n]).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });
            }
        });

        // 起一个 listener 模拟 client → handler
        let client_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client_addr = client_listener.local_addr().unwrap();

        let opts = ForwardHandlerOptions {
            chain: None,
            target: Some(target_addr.to_string()),
            dialer: Some(Arc::new(crate::dialer::TcpDialer::new())),
            buffer_size: 4096,
        };
        let handler = Arc::new(ForwardHandler::new(opts));

        // 服务端接收 client 连接后交给 handler
        let handler_clone = handler.clone();
        let server_task = tokio::spawn(async move {
            let (conn, _) = client_listener.accept().await.unwrap();
            handler_clone.handle(Box::new(conn)).await
        });

        // 真正的 client
        let mut client = tokio::net::TcpStream::connect(client_addr).await.unwrap();
        client.write_all(b"hello").await.unwrap();
        let mut buf = [0u8; 5];
        let n = tokio::time::timeout(std::time::Duration::from_secs(2), client.read(&mut buf))
            .await
            .expect("echo 应在 2s 内返回")
            .unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf, b"hello");

        // 关闭
        drop(client);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), server_task).await;
    }
}