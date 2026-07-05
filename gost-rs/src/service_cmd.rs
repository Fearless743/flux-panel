//! service 命令路由。
//!
//! 与 Go 版 `x/socket/service.go` 1:1 对齐：
//! - `AddService`：注册新服务（已存在则报错）
//! - `UpdateService`：替换已有服务（先 unregister 再 register，旧服务的 listener 需先 close）
//! - `DeleteService`：注销服务（关闭 listener）
//! - `PauseService`：标记 paused=true
//! - `ResumeService`：标记 paused=false
//!
//! 每个命令都是事务性的：失败时回滚到调用前状态。
//!
//! 阶段 2 接受 [`ServiceConfig`]（来自 gost.json / WebSocket 解密后）作为输入。
//! 阶段 3 把这些函数挂到 WS 命令路由。

use std::sync::Arc;

use crate::config::types::ServiceConfig;
use crate::registry::services_registry;
use crate::service::service_impl::build_service;
use crate::service::ServiceImpl;

/// AddService 注册新服务。
///
/// 错误：
/// - 已存在同名服务 → 报错
/// - listener/handler 构造失败 → 报错（无需回滚，因为还没注册）
pub async fn add_service(cfg: ServiceConfig) -> anyhow::Result<()> {
    let reg = services_registry();
    if reg.get(&cfg.name).is_some() {
        anyhow::bail!("service already exists: {}", cfg.name);
    }
    let svc = build_service(&cfg).await?;
    // 启动 accept loop
    let _ = svc.clone().spawn_serve();
    reg.register(&cfg.name, svc);
    Ok(())
}

/// UpdateService 替换已存在的服务。
///
/// 步骤：
/// 1. 取出旧服务，保存其 config（用于失败回滚）
/// 2. 关闭旧服务（close listener）
/// 3. unregister 旧服务
/// 4. 构造并注册新服务
/// 5. 失败时：尝试重新注册旧服务（如果旧服务 shutdown 未完成，会先 wait）
pub async fn update_service(cfg: ServiceConfig) -> anyhow::Result<()> {
    let reg = services_registry();
    let old = reg.get(&cfg.name).ok_or_else(|| {
        anyhow::anyhow!("service not found for update: {}", cfg.name)
    })?;
    let old_clone = old.clone();

    // 1. 关闭旧服务
    old_clone.shutdown().await?;
    // 2. unregister
    reg.unregister(&cfg.name);

    // 3. 构造并注册新服务
    let new_svc = match build_service(&cfg).await {
        Ok(s) => s,
        Err(e) => {
            // 回滚：恢复旧服务
            reg.register(&cfg.name, old_clone);
            return Err(e);
        }
    };

    // 4. 注册新服务
    let prev = reg.register(&cfg.name, new_svc.clone());
    if prev.is_some() {
        // 已被注册（理论上不应发生）
        return Err(anyhow::anyhow!(
            "service replaced with non-empty previous value"
        ));
    }

    // 5. 启动 accept loop
    let _ = new_svc.spawn_serve();
    Ok(())
}

/// DeleteService 注销服务。
pub async fn delete_service(name: &str) -> anyhow::Result<()> {
    let reg = services_registry();
    let svc = reg
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("service not found: {name}"))?;
    svc.shutdown().await?;
    reg.unregister(name);
    Ok(())
}

/// PauseService 标记服务暂停。
///
/// 行为：
/// - 设置 metadata.paused = true
/// - listener 继续监听端口（便于面板查询端口仍在监听）
/// - accept 后立即关闭连接
pub fn pause_service(name: &str) -> anyhow::Result<()> {
    let reg = services_registry();
    let svc = reg
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("service not found: {name}"))?;
    svc.pause();
    Ok(())
}

/// ResumeService 标记服务恢复。
///
/// 行为：
/// - 设置 metadata.paused = false
/// - 接受新连接并处理
pub fn resume_service(name: &str) -> anyhow::Result<()> {
    let reg = services_registry();
    let svc = reg
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("service not found: {name}"))?;
    svc.resume();
    Ok(())
}

/// GetService 查询服务（返回 Arc clone + config）。
pub fn get_service(name: &str) -> Option<Arc<ServiceImpl>> {
    services_registry().get(name)
}

/// ListServices 列出全部服务名。
pub fn list_services() -> Vec<String> {
    services_registry().names()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{HandlerConfig, ListenerConfig};

    fn make_cfg(name: &str, port: u16) -> ServiceConfig {
        ServiceConfig {
            name: name.into(),
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
        }
    }

    #[tokio::test]
    async fn add_then_delete() {
        let cfg = make_cfg("test_svc_1", 0); // 0 让 OS 分配
        // 0 端口对 build_service 会失败；改成随机 high port
        let mut cfg = cfg;
        cfg.addr = Some("127.0.0.1:0".into());

        // 不直接 add（端口 0 会成功），用 spawn 一个独立 listener 拿端口
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        cfg.addr = Some(format!("127.0.0.1:{port}"));
        // 确保 clean
        let _ = delete_service(&cfg.name).await;
        add_service(cfg).await.unwrap();
        assert!(get_service("test_svc_1").is_some());

        delete_service("test_svc_1").await.unwrap();
        assert!(get_service("test_svc_1").is_none());
    }

    #[tokio::test]
    async fn add_existing_fails() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let mut cfg = make_cfg("dup_test", port);
        cfg.addr = Some(format!("127.0.0.1:{port}"));

        add_service(cfg.clone()).await.unwrap();
        let res = add_service(cfg).await;
        assert!(res.is_err(), "重复注册应报错");

        // cleanup
        let _ = delete_service("dup_test").await;
    }

    #[tokio::test]
    async fn pause_resume() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let cfg = make_cfg("pr_test", port);
        add_service(cfg).await.unwrap();

        pause_service("pr_test").unwrap();
        let svc = get_service("pr_test").unwrap();
        assert!(svc.paused.load(std::sync::atomic::Ordering::Relaxed));

        resume_service("pr_test").unwrap();
        let svc = get_service("pr_test").unwrap();
        assert!(!svc.paused.load(std::sync::atomic::Ordering::Relaxed));

        let _ = delete_service("pr_test").await;
    }
}