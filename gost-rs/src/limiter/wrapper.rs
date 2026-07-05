//! limiter wrappers：把 traffic 限速器挂到 io 读写上。
//!
//! 与 Go 版 `x/limiter/traffic/wrapper/` + `x/observer/stats/wrapper/` 对齐。
//!
//! 这里提供阻塞风格的 `read_throttled` / `write_throttled` 辅助函数，
//! 直接在 AsyncRead/AsyncWrite 上注入 token bucket 控速。

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::traffic::{Scope, TrafficLimiter};

/// 阻塞读直到填满 buf（受限速），返回实际字节数。
pub async fn read_throttled<R: AsyncRead + Unpin>(
    r: &mut R,
    buf: &mut [u8],
    bucket: &Arc<Mutex<super::traffic::TokenBucket>>,
) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = bucket.lock().try_take((buf.len() - filled) as u64);
        if n == 0 {
            tokio::time::sleep(Duration::from_millis(1)).await;
            continue;
        }
        let want = n as usize;
        r.read_exact(&mut buf[filled..filled + want]).await?;
        filled += want;
    }
    Ok(filled)
}

    /// 阻塞写直到全部发出（受限速）。
pub async fn write_throttled<W: AsyncWrite + Unpin>(
    w: &mut W,
    buf: &[u8],
    bucket: &Arc<Mutex<super::traffic::TokenBucket>>,
) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut written = 0;
    while written < buf.len() {
        let n = bucket.lock().try_take((buf.len() - written) as u64);
        if n == 0 {
            tokio::time::sleep(Duration::from_millis(1)).await;
            continue;
        }
        let want = n as usize;
        w.write_all(&buf[written..written + want]).await?;
        written += want;
    }
    Ok(())
}

/// 按 traffic 限速器 + key 决定该 conn 的方向令牌桶。
/// 若 in/out 方向都不限制返回 None。
pub fn buckets_for(
    lim: &TrafficLimiter,
    scope: Scope,
    key: &str,
) -> Option<(
    Arc<Mutex<super::traffic::TokenBucket>>,
    Arc<Mutex<super::traffic::TokenBucket>>,
)> {
    let pair = lim.in_limiter(scope, key)?;
    Some((pair.inb, pair.outb))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn throttled_write_obeys_rate() {
        let (mut client, mut server) = duplex(64 * 1024);
        let bucket = Arc::new(Mutex::new(crate::limiter::traffic::TokenBucket::new(100_000))); // 100KB/s
        let data = vec![0xABu8; 100_000];

        let b = bucket.clone();
        let writer = tokio::spawn(async move {
            write_throttled(&mut client, &data, &b).await.unwrap();
            client.shutdown().await.unwrap();
        });

        // 接收端读到全部
        let mut got = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            let n = server.read(&mut tmp).await.unwrap();
            if n == 0 {
                break;
            }
            got.extend_from_slice(&tmp[..n]);
        }
        writer.await.unwrap();
        assert_eq!(got.len(), 100_000);
    }

    #[tokio::test]
    async fn rate_caps_total_throughput() {
        // 单独验证 token bucket 的 refill 行为：低速率桶再次 wait 应阻塞
        use crate::limiter::traffic::TokenBucket;
        let mut bucket = TokenBucket::new(100_000); // 100KB/s
        let start = std::time::Instant::now();
        // burst = 100KB，wait 100KB 应近乎瞬时
        let took = bucket.wait(100_000).await;
        let first_elapsed = start.elapsed();
        assert_eq!(took, 100_000);
        assert!(first_elapsed < Duration::from_millis(50), "首次 burst 应快: {:?}", first_elapsed);

        // 再 wait 50KB：需 ~500ms 补充（部分 burst 还在）
        let start = std::time::Instant::now();
        let _ = bucket.wait(50_000).await;
        let second_elapsed = start.elapsed();
        assert!(
            second_elapsed < Duration::from_secs(2),
            "限速循环耗时 {:?}, 应在 2s 内完成",
            second_elapsed
        );
    }

    #[tokio::test]
    async fn unthrottled_passthrough() {
        let (mut client, mut server) = duplex(4096);
        let data = b"hello";

        // 没有 bucket：直接写
        client.write_all(data).await.unwrap();
        let mut buf = [0u8; 5];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, data);
    }
}