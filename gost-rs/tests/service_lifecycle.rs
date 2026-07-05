//! 端到端：service 接受连接 → forward 到 echo server → 双向通信。
//!
//! 与阶段 2 目标对应：
//! 1. 起 echo TCP server
//! 2. 手动构造 listener + forward handler（阶段 2 build_service 不支持 target）
//! 3. spawn accept loop
//! 4. client 连接 → 写入 → 收到 echo
//! 5. close listener

use flux_agent::core::{Handler, Listener};
use flux_agent::handler::forward::{ForwardHandler, ForwardHandlerOptions};
use flux_agent::listener::TcpListenerImpl;
use flux_agent::service_cmd;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn find_free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

#[tokio::test]
async fn e2e_forward_through_listener() {
    // 1. echo server
    let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_addr = echo.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = echo.accept().await.unwrap();
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

    // 2. listener + handler
    let listener = Arc::new(TcpListenerImpl::bind("127.0.0.1:0").await.unwrap());
    let listener_addr = listener.local_addr();
    let handler = Arc::new(ForwardHandler::new(ForwardHandlerOptions {
        chain: None,
        target: Some(echo_addr.to_string()),
        dialer: Some(Arc::new(flux_agent::dialer::TcpDialer::new())),
        buffer_size: 4096,
    }));

    // 3. accept loop
    let listener_clone = listener.clone();
    let handler_clone = handler.clone();
    let server_task = tokio::spawn(async move {
        loop {
            let conn = match listener_clone.accept().await {
                Ok(c) => c,
                Err(_) => break,
            };
            let h = handler_clone.clone();
            tokio::spawn(async move {
                let _ = h.handle(conn).await;
            });
        }
    });

    // 4. client → listener → forward → echo
    let mut client = TcpStream::connect(listener_addr).await.unwrap();
    client.write_all(b"hello world").await.unwrap();
    let mut buf = [0u8; 11];
    let n = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        client.read_exact(&mut buf),
    )
    .await
    .expect("echo 应在 2s 内返回")
    .unwrap();
    assert_eq!(n, 11);
    assert_eq!(&buf, b"hello world");

    // 5. close
    listener.close().await.unwrap();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), server_task).await;
}

#[tokio::test]
async fn e2e_multiple_concurrent_clients() {
    // echo server
    let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_addr = echo.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = echo.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 1024];
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

    // listener
    let listener = Arc::new(TcpListenerImpl::bind("127.0.0.1:0").await.unwrap());
    let addr = listener.local_addr();
    let handler = Arc::new(ForwardHandler::new(ForwardHandlerOptions {
        chain: None,
        target: Some(echo_addr.to_string()),
        dialer: Some(Arc::new(flux_agent::dialer::TcpDialer::new())),
        buffer_size: 1024,
    }));

    let listener_clone = listener.clone();
    let handler_clone = handler.clone();
    let server_task = tokio::spawn(async move {
        loop {
            let conn = match listener_clone.accept().await {
                Ok(c) => c,
                Err(_) => break,
            };
            let h = handler_clone.clone();
            tokio::spawn(async move {
                let _ = h.handle(conn).await;
            });
        }
    });

    // 10 个并发 client
    let mut handles = vec![];
    for i in 0..10 {
        let addr = addr;
        handles.push(tokio::spawn(async move {
            let mut client = TcpStream::connect(addr).await.unwrap();
            let msg = format!("client-{i:02}");
            client.write_all(msg.as_bytes()).await.unwrap();
            let mut buf = vec![0u8; 16];
            let n = client.read_exact(&mut buf[..msg.len()]).await.unwrap();
            assert_eq!(n, msg.len());
            assert_eq!(&buf[..msg.len()], msg.as_bytes());
        }));
    }

    for h in handles {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(3), h).await;
    }

    listener.close().await.unwrap();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), server_task).await;
}

#[tokio::test]
async fn service_cmd_add_pause_resume_delete() {
    use flux_agent::config::types::{HandlerConfig, ListenerConfig, ServiceConfig};

    // 清理可能残留
    for n in ["svc_lc_1", "svc_lc_2"] {
        let _ = service_cmd::delete_service(n).await;
    }

    let port = find_free_port().await;
    let cfg = ServiceConfig {
        name: "svc_lc_1".into(),
        addr: Some(format!("127.0.0.1:{port}")),
        listener: Some(ListenerConfig {
            r#type: "tcp".into(),
            ..Default::default()
        }),
        handler: Some(HandlerConfig {
            r#type: "forward".into(),
            ..Default::default()
        }),
        ..Default::default()
    };

    service_cmd::add_service(cfg).await.unwrap();
    assert!(service_cmd::get_service("svc_lc_1").is_some());

    service_cmd::pause_service("svc_lc_1").unwrap();
    let svc = service_cmd::get_service("svc_lc_1").unwrap();
    assert!(svc.paused.load(std::sync::atomic::Ordering::Relaxed));

    service_cmd::resume_service("svc_lc_1").unwrap();
    let svc = service_cmd::get_service("svc_lc_1").unwrap();
    assert!(!svc.paused.load(std::sync::atomic::Ordering::Relaxed));

    service_cmd::delete_service("svc_lc_1").await.unwrap();
    assert!(service_cmd::get_service("svc_lc_1").is_none());
}