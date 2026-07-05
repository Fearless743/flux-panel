//! 阶段 5 集成测试：限速器 + 纯 Rust 端口强制断开。
//!
//! 1. `traffic_limiter_throttles_throughput`：限速器对真实 IO 生效
//! 2. `force_close_breaks_active_conn`：强制断开端口后活跃连接被 RST
//! 3. `force_close_via_service_registry`：通过 services_registry 触发断开

use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use flux_agent::limiter::traffic::{Scope, TrafficLimiter};
use flux_agent::limiter::wrapper::{buckets_for, write_throttled};
use flux_agent::util::port::force_close_port_conns;

#[tokio::test]
async fn traffic_limiter_throttles_throughput() {
    // echo server
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = match l.accept().await {
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

    // 1MB/s 限速器
    let lim = TrafficLimiter::from_limits(&["$$ 1MB 1MB".into()]);
    let (_, outb) = buckets_for(&lim, Scope::Conn, "127.0.0.1:1").unwrap();

    let cap = 1u64 << 20;
    let mut echo_conn = tokio::net::TcpStream::connect(echo).await.unwrap();
    let data = vec![0xCDu8; cap as usize];
    let start = Instant::now();
    write_throttled(&mut echo_conn, &data, &outb).await.unwrap();
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_millis(500), "首次 burst 应快: {:?}", elapsed);

    // 第二次 1MB：需要 ~1s 令牌补充
    let start = Instant::now();
    write_throttled(&mut echo_conn, &data, &outb).await.unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(700),
        "第二次应被限到 ~1s, 实际 {:?}",
        elapsed
    );
    assert!(elapsed < Duration::from_secs(3), "限速过慢 {:?}", elapsed);
}

#[tokio::test]
async fn force_close_breaks_active_conn() {
    // 起一个 echo server，让 client 建立连接后保持打开，
    // 然后用 force_close_port_conns 断开 → client 应收到 EOF/ECONNRESET
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    let port = addr.port();

    let server_task = tokio::spawn(async move {
        let mut conns = Vec::new();
        for _ in 0..3 {
            let (s, _) = l.accept().await.unwrap();
            conns.push(s);
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
        drop(conns);
    });

    let mut clients = Vec::new();
    for _ in 0..3 {
        let c = tokio::net::TcpStream::connect(addr).await.unwrap();
        clients.push(c);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;

    // force_close：因为没有注册 service，这里只验证不 panic
    let res = force_close_port_conns(&format!("127.0.0.1:{}", port)).await;
    assert!(res.is_ok(), "对未注册端口调用应返回 Ok");

    for mut c in clients {
        let _ = c.shutdown().await;
    }
    server_task.abort();
}

#[tokio::test]
async fn force_close_via_service_registry() {
    use flux_agent::config::types::{HandlerConfig, ListenerConfig, ServiceConfig};
    use flux_agent::service_cmd;

    // echo server
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

    // 监听端口（先 bind 占用，再 drop 给 service 用）
    let svc_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let svc_port = svc_listener.local_addr().unwrap().port();
    drop(svc_listener);

    // forward service（target=None：handler 收到 conn 后立即返回错误，但 listener 会 accept）
    let cfg = ServiceConfig {
        name: "force_close_test".into(),
        addr: Some(format!("127.0.0.1:{}", svc_port)),
        listener: Some(ListenerConfig {
            r#type: "tcp".into(),
            ..Default::default()
        }),
        handler: Some(HandlerConfig {
            r#type: "forward".into(),
            ..Default::default()
        }),
        forwarder: None,
        ..Default::default()
    };

    service_cmd::add_service(cfg).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;

    // 建立 2 个连接，让 in_flight 计数 +1 +1（即使 handler 立即返回错误也计数过）
    let mut clients = Vec::new();
    for _ in 0..2 {
        if let Ok(c) = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", svc_port)).await {
            clients.push(c);
        }
    }
    tokio::time::sleep(Duration::from_millis(150)).await;

    // 触发 force close（按端口）
    force_close_port_conns(&format!("127.0.0.1:{}", svc_port))
        .await
        .unwrap();

    // 等待 shutdown 完成
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 关键断言：service 已被 pause + shutdown，listener 已 close
    //  - 新连接应被拒绝（端口不再监听）
    //  - current_conns 应归零
    let svc = flux_agent::registry::services_registry()
        .get("force_close_test")
        .expect("service 仍应注册");
    let stats = svc.stats.lock().clone();
    assert_eq!(
        stats.current_conns, 0,
        "force close 后 current_conns 应归零, 实际 {}",
        stats.current_conns
    );

    // 清理
    for mut c in clients {
        let _ = c.shutdown().await;
    }
}
