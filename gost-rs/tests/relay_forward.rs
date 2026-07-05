//! 阶段 4 集成测试：Relay 协议两节点中转。
//!
//! 拓扑：
//! ```text
//! client → [relay server A] → [relay server B (forward 到 echo)] → echo server
//! ```
//!
//! 这里简化为 client → relay server → echo server 验证 relay handler + connector 协议。

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use flux_agent::connector::relay::{RelayConnector, RelayConnectorOptions};
use flux_agent::core::{Connector, Handler};
use flux_agent::handler::relay::{RelayHandler, RelayHandlerOptions};

#[tokio::test]
async fn relay_forward_two_hops() {
    // 1. echo server
    let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_addr = echo.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = match echo.accept().await {
                Ok(x) => x,
                Err(_) => break,
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                loop {
                    match s.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if s.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
    });

    // 2. relay server (handler 模式：固定 target = echo_addr)
    let relay_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = relay_listener.local_addr().unwrap();
    let handler = std::sync::Arc::new(RelayHandler::new(RelayHandlerOptions {
        chain: None,
        target: Some(echo_addr.to_string()),
        dialer: Some(std::sync::Arc::new(flux_agent::dialer::TcpDialer::new())),
    }));
    let h = handler.clone();
    tokio::spawn(async move {
        loop {
            let (s, _) = match relay_listener.accept().await {
                Ok(x) => x,
                Err(_) => break,
            };
            let hh = h.clone();
            tokio::spawn(async move {
                let _ = hh.handle_full(Box::new(s)).await;
            });
        }
    });

    // 3. connector 拨号到 relay server，target 填 echo_addr
    let conn = RelayConnector::new(RelayConnectorOptions {
        server: relay_addr.to_string(),
        timeout_secs: 5,
        ..Default::default()
    });
    let mut stream = conn.connect(&echo_addr.to_string()).await.unwrap();

    // 4. 多轮 echo 验证
    for i in 0..5u8 {
        let payload = format!("relay-{}", i);
        stream.write_all(payload.as_bytes()).await.unwrap();
        let mut buf = vec![0u8; payload.len()];
        let n = tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut buf))
            .await
            .expect("echo 应在 2s 内返回")
            .unwrap();
        assert_eq!(n, payload.len());
        assert_eq!(&buf, payload.as_bytes());
    }

    // 5. 较大 payload 验证
    let big: Vec<u8> = (0..32 * 1024).map(|i| (i % 251) as u8).collect();
    stream.write_all(&big).await.unwrap();
    let mut got = vec![0u8; big.len()];
    let mut total = 0;
    while total < big.len() {
        let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut got[total..]))
            .await
            .expect("echo 大包应在 5s 内返回")
            .unwrap();
        if n == 0 {
            break;
        }
        total += n;
    }
    assert_eq!(total, big.len());
    assert_eq!(got, big);
}

#[tokio::test]
async fn relay_connector_rejected_by_status() {
    // relay server 始终回 StatusInternalServerError
    let relay_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = relay_listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = match relay_listener.accept().await {
                Ok(x) => x,
                Err(_) => break,
            };
            tokio::spawn(async move {
                use flux_agent::util::relay::frame::{Response, Status};
                let _ = flux_agent::util::relay::frame::Request::read_from_async(&mut s).await;
                let resp = Response::new(Status::InternalServerError);
                let _ = resp.write_to_async(&mut s).await;
            });
        }
    });

    let conn = RelayConnector::new(RelayConnectorOptions {
        server: relay_addr.to_string(),
        timeout_secs: 5,
        ..Default::default()
    });
    let res = conn.connect("example.com:80").await;
    assert!(res.is_err(), "服务器返回错误状态时 connect 应失败");
}