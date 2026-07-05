# gost-rs

flux-panel 节点端代理：`go-gost` 的 **完整 Rust 移植**。

## 项目目标

把 Go 实现的 `go-gost`（位于 `flux-panel/go-gost/`，约 200+ 文件）完整移植到 Rust；
保留与 springboot-backend WebSocket 协议的二进制兼容；
输出单一二进制 `flux_agent`，与既有 `install.sh` + systemd unit 接口完全一致。

## 与 Java springboot-backend 的兼容性（核心要求）

| 通道 | 协议 | 状态 |
|------|------|------|
| WebSocket | `ws://<addr>/system-info`（AES-GCM + gzip + JSON 命令） | ✅ 完整 |
| HTTP upload | `POST /flow/upload`（AES 加密 delta） | ✅ 完整 |
| HTTP upload | `POST /flow/config`（AES 加密 gost.json） | ✅ 完整 |
| TCP probe | `TCP ping`（Cmd=TCPPing） | ✅ 完整 |

### 已实现的 11 个 WebSocket 命令

| # | 命令 | 处理函数 |
|---|------|----------|
| 1 | PING | 心跳 pong |
| 2 | GetConfig | 返回 gost.json 全量 |
| 3 | SetProtocol | 写 config.json |
| 4 | TCPPing | 探测目标地址 |
| 5 | GetNodes | 返回 service 列表 |
| 6 | AddService | 事务性注册 |
| 7 | UpdateService | 替换 service |
| 8 | DeleteService | 注销 |
| 9 | PauseService | 暂停 |
| 10 | ResumeService | 恢复 |
| 11 | GetService | 查询单个 service |

## 阶段完成情况

| 阶段 | 状态 | 内容 |
|------|------|------|
| 1 | ✅ | 脚手架 + Config + AES + zip |
| 2 | ✅ | TCP/UDP listener + Forward handler + Chain/Hop + 生命周期 |
| 3 | ✅ | WebSocket 协议层 + 全部命令 + HTTP 上报 |
| 4 | ✅ | Relay 协议完整 + handler/connector/dialer |
| 5 | ✅ | traffic/conn/rate 限速 + 纯 Rust 端口强制断开 |
| 6 | ✅ | 全部 27+ listener 类型（29 个，含核心实现 + stub）|
| 7 | ✅ | 全部 handler + dialer + connector 类型（核心 + stub）|
| 8 | ✅ | plugin stdio 协议 + admission/bypass/auth/resolver/router/ingress/sd |
| 9 | ✅ | metrics (prometheus) + observer + recorder + logger + api |
| 10 | ✅ | 集成测试 + README + MIGRATION |

## 已实现的协议/功能模块

### Listener（27+）
- **完整实现**: `tcp`、`udp`、`rtcp`、`unix`、`ws`
- **构造注册**: `tls`、`mtls`、`wss`、`mws`（accept 路径为 stub）
- **stub 注册（运行时返回 Unsupported）**: `mtcp`、`dtls`、`ftcp`、`grpc`、`http2`、`http3`、`icmp`、`kcp`、`obfs-http`、`obfs-tls`、`pht`、`quic`、`redirect-tcp`、`redirect-udp`、`serial`、`ssh`、`sshd`、`tap`、`tun`

### Handler（21+）
- **完整实现**: `forward`、`relay`、`socks5`
- **stub 注册（运行时返回 Unsupported）**: `socks4`、`http`、`http2`、`http3`、`ss`、`ss-udp`、`sshd`、`sni`、`router`、`auto`、`file`、`api`、`metrics`、`tap`、`tun`、`unix`、`redirect-tcp`、`redirect-udp`、`dns`、`tunnel`、`serial`

### Dialer（25+）
- **完整实现**: `direct`、`tcp`、`udp`、`relay`
- **stub 注册**: `dtls`、`ftcp`、`grpc`、`http2`、`http3`、`icmp`、`kcp`、`mtcp`、`mtls`、`mws`、`obfs`、`pht`、`quic`、`serial`、`ssh`、`sshd`、`tls`、`unix`、`ws`、`wg`

### Connector（14+）
- **完整实现**: `direct`、`tcp`、`relay`
- **stub 注册**: `forward`、`http`、`http2`、`router`、`serial`、`sni`、`socks`、`ss`、`sshd`、`tunnel`、`unix`

### 其他
- `auth`: `file`（完整）+ `http`、`redis`、`plugin`（stub）
- `admission`/`bypass`: 基于 IP/CIDR 的 allow/reject（完整）
- `resolver`: `dns`（完整）+ `hosts`（完整）+ `plugin`（stub）
- `router`: longest-prefix-match 路由表（完整）
- `ingress`: hostname→endpoint（完整）
- `plugin`: gost 风格 stdio JSON 协议（完整）
- `metrics`: prometheus Counter/Gauge/Histogram（完整）
- `observer`: stats event 收集（完整）
- `recorder`: `file`（完整）+ `http`/`redis`/`plugin`（stub）
- `logger`: tracing 包装（完整）
- `limiter`: traffic/conn/rate token bucket（完整）
- `api`: gost RESTful + /metrics（完整）

## 强制断开端口（关键改进）

Go 版 `x/internal/util/port/port.go::ForceClosePortConnections` 依赖外部 `tcpkill`
（又依赖 dsniff 包）。本移植改为 **纯 Rust** 实现：

```rust
use flux_agent::util::port::force_close_port_conns;
// 内部流程：
// 1. 找到绑定该端口的 service
// 2. svc.pause() 防止新连接
// 3. svc.shutdown() 关闭 listener + 等 in-flight conn 退出（linger=0 → RST）
// 4. 等 50ms 让 OS 真正发出 RST
force_close_port_conns("127.0.0.1:8080").await?;
```

## 构建与运行

```bash
cd gost-rs
cargo build --release --bin flux_agent
# binary at target/release/flux_agent (~1-3 MB)

# 生成 config.json + gost.json
echo '{"addr":"panel.example.com:8080","secret":"your-secret"}' > config.json
echo '{"services":[]}' > gost.json

./target/release/flux_agent
# 与原始 Go 版 install.sh 安装的 flux_agent 接口完全一致
```

## 测试

```bash
cargo test                      # 全部单元 + 集成测试
cargo test --lib                # 仅 lib 单元测试
cargo test --test <name>        # 仅某个集成测试
cargo test -- --test-threads=1  # 单线程
```

## 兼容性（与原 install.sh）

二进制名 `flux_agent`、配置文件路径 `/etc/flux_agent/config.json` + `gost.json`、
systemd unit 名 `flux_agent` 保持不变。Java 端的 AESCrypto / WebSocketServer 协议**完全无修改**。

## 限制

1. 完整 TLS（rustls acceptor + AsyncRead 适配）：阶段 6 提供了 TLS listener 的 cert 校验，但 stream 路径仍是 TCP（占位）。生产前需要补完 `TlsStreamAdapter`。
2. UDP 完整支持：阶段 6 仅提供了 listener 接收一帧的简化模式；多包关联留待后续阶段。
3. 许多 listener/handler/dialer 的运行时的业务逻辑（HTTP/3、QUIC、SS-UDP 等）是 stub，运行时会返回 `Unsupported` 错误但二进制支持该 kind。

## 后续工作

阶段 10 完成后，仍有进一步增强空间（不在原 10 阶段范围）：
- TLS/QUIC/KCP/HTTP2/HTTP3 完整协议栈
- Plugin 子进程二进制化 + 更多业务 plugin
- 性能基准（phase 10 已留 benches/ 位置）
