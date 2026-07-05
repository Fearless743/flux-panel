//! flux_agent 主入口。
//!
//! 启动流程：
//!   1. 解析命令行
//!   2. 初始化 tracing
//!   3. 加载 config.json
//!   4. 启动 WebSocket reporter（连接 springboot-backend）
//!   5. 启动全局流量上报循环（每 5s）
//!   6. 启动配置上报循环（每 10min）
//!   7. 等 shutdown 信号

use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info};

use flux_agent::config;
use flux_agent::report::WsReporter;
use flux_agent::service::{config_reporter, global_traffic};
use flux_agent::version;

#[derive(Debug, Parser)]
#[command(
    name = "flux_agent",
    version = version::VERSION,
    about = "flux-panel 节点端代理（go-gost Rust 移植）"
)]
struct Cli {
    /// gost.json 路径
    #[arg(short = 'C', long, env = "GOST_CONFIG", default_value = "gost.json")]
    config: PathBuf,

    /// 调试模式（-D 开启）
    #[arg(short = 'D', long)]
    debug: bool,

    /// trace 模式（-DD 开启）
    #[arg(short = 'D', long = "DD", num_args = 0)]
    trace: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let local = tokio::task::LocalSet::new();
    local.run_until(run()).await;
}

async fn run() {
    let cli = Cli::parse();

    if cli.debug || cli.trace {
        std::env::set_var(
            "RUST_LOG",
            if cli.trace { "trace" } else { "debug" },
        );
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    info!(
        version = version::VERSION,
        protocol = version::PROTOCOL_VERSION,
        "flux_agent 启动中..."
    );

    // 1. 加载 config.json
    let node_cfg = match load_node_config() {
        Ok(c) => {
            info!(
                addr = %c.addr,
                ssl = c.ssl,
                protocol_block = ?(c.http, c.tls, c.socks),
                "config.json 加载成功"
            );
            c
        }
        Err(e) => {
            error!("config.json 加载失败：{e}");
            std::process::exit(1);
        }
    };

    info!("flux_agent 已就绪");

    // 2. 启动 WS reporter（连接 springboot-backend）
    // 因为 WsReporter 包含 NonSend 类型（MaybeTlsStream），需要 LocalSet/task。
    let gost_path = cli.config.clone();
    let cfg_path_str = gost_path
        .to_str()
        .unwrap_or("gost.json")
        .to_string();
    let reporter = Arc::new(WsReporter::new(node_cfg.clone(), cfg_path_str));

    // spawn_local 在 LocalSet 中调用
    let ws_handle = tokio::task::spawn_local({
        let r = reporter.clone();
        async move { r.run().await }
    });

    // 流量 + 配置上报 loop
    let traffic_handle = {
        let r = reporter.clone();
        tokio::task::spawn_local(async move { global_traffic::run_global_traffic_loop(r).await })
    };
    let config_handle = {
        let r = reporter.clone();
        tokio::task::spawn_local(async move { config_reporter::run_config_reporter_loop(r).await })
    };

    // 5. 等 shutdown 信号
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("收到 SIGINT，准备退出...");
        }
        _ = ws_handle => {
            error!("WebSocket reporter 异常退出");
        }
        _ = traffic_handle => {
            error!("全局流量上报异常退出");
        }
        _ = config_handle => {
            error!("配置上报异常退出");
        }
    }

    reporter.shutdown();
    info!("flux_agent 退出");
}

fn load_node_config() -> anyhow::Result<crate::config::NodeConfig> {
    crate::config::load_node_config()
}
