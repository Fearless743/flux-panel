# 安全公告：CVE-2026-98432

## 结论（先读）

CVE-2026-98432 是 **Linux 内核 IPv4 TCP 协议栈的 use-after-free**，不是本仓库
（flux-panel / go-gost / go-backend）用户态代码里的漏洞。**无法通过修改本项目代码根治**，
唯一根治方式是升级宿主机内核（或使用发行版的 rebootless 内核补丁）。

本项目能做的是**缩小攻击面**：默认关闭 Go 1.24+ 在监听套接字上自动启用的 MPTCP。
这不能替代内核补丁，宿主机仍必须按下方步骤处置。

## 漏洞要点

- 位置：`net/ipv4`。`tcp_v4_syn_recv_sock()` 把 `ireq->ireq_opt` 的所有权转给子 socket，
  另一 CPU 处理重传 SYN 时会在 `ip_options_build()` 中继续读取该内存；子 socket 被释放时
  `inet_sock_destruct()` 用普通 `kfree()` 释放 `inet_opt`，形成 UAF。
- 引入版本：Linux 4.4（commit `079096f103fa`）。
- 触发条件：监听 IPv4 TCP 端口，客户端发送**带 IPv4 选项（Record Route / Timestamp）的 SYN**
  （正常流量极少这样发）。主路径不需要 MPTCP，也不需要特殊配置。
- 次要路径：request socket migration（`net.ipv4.tcp_migrate_req=1` 或
  `BPF_SK_REUSEPORT_SELECT_OR_MIGRATE`），默认关闭。
- 影响：远程 DoS / 内核内存破坏；公开演示的利用链以 MPTCP 监听为前提，可进一步提权。
- IPv6：主路径不受影响；CALIPSO 相关的 migration 问题由上游另行修复。

## 本项目已实施的缓解

1. **生产 Rust 后端不涉及 MPTCP**：`rust-backend/` 基于 Tokio/axum，不会像 Go 1.24+
   那样在监听套接字上默认启用 MPTCP，天然没有该利用链需要的监听面。
2. **Go 组件构建期固定**：节点 agent 与历史面板后端仍加入
   ```text
   godebug multipathtcp=0
   ```
   即使以后把 `go` 指令或构建工具链升到 Go 1.24+，监听套接字也不会默认启用 MPTCP。
   涉及文件：`go-backend/go.mod`、`go-gost/go.mod`、`go-gost/x/go.mod`。
3. **运行期固定**：`GODEBUG=multipathtcp=0`
   - 一体镜像（Caddy 为 Go 程序）与历史后端镜像：`Dockerfile`、`go-backend/Dockerfile`、`docker/entrypoint.sh`
   - Docker Compose：`docker-compose-v4.yml`、`docker-compose-v6.yml`
   - systemd 节点：`install.sh` 生成的 `flux_agent.service`
4. **保留显式 MPTCP 能力**：节点配置中显式写 `mptcp=true` 时，代码仍会调用
   `SetMultipathTCP(true)` 覆盖上述默认值。**内核修复前请勿在生产节点开启该选项。**

> 说明：Go 版 `go` 指令为 1.23，构建工具链为 Go 1.23，本身不会触发 Go 1.24+ 的
> MPTCP 默认行为；上述固定是为了防止未来升级 Go 工具链后无意中暴露攻击面。

## 宿主机处置（必须执行）

容器与宿主机共享内核，**升级本项目的镜像不会修复该漏洞**。

### 1. 检查现状

```bash
uname -r
cat /etc/os-release
sysctl net.mptcp.enabled net.ipv4.tcp_migrate_req
ss -H -lMn          # 每行都是一个 MPTCP 监听；无输出表示没有
```

### 2. 升级内核（根治）

优先按发行版安全公告升级并重启。截至 2026-10-30 的上游修复版本：

```text
5.10.272 / 5.15.223 / 6.1.190 / 6.6.159 / 6.12.113 / 6.18.56 / 7.2.10 / 7.3
```

各发行版状态会变化，请以官方公告为准：

- Debian 12/13：已修复（DSA-6568-1、DLA-4841-1、DLA-4850-1）
- Oracle Linux UEK：已修复
- Amazon Linux 2023 `kernel6.12` / `kernel6.18`：已修复
- RHEL 8/9/10 及其重建版、Ubuntu：截至 2026-10-30 尚无正式内核包，需跟踪厂商公告
- 托管 Kubernetes / 云主机：升级节点池镜像或宿主内核

无法立即重启时，可评估厂商的 rebootless 补丁（如 KernelCare）。

### 3. 临时缓解（打补丁前）

```bash
# 关闭 MPTCP（不需要该特性的宿主机）
sysctl -w net.mptcp.enabled=0
printf 'net.mptcp.enabled = 0\n' > /etc/sysctl.d/99-cve-2026-98432.conf
sysctl --system

# 确认迁移路径关闭（默认值）
sysctl -w net.ipv4.tcp_migrate_req=0
```

已打开 MPTCP 监听的进程需要重启，才会回落到普通 TCP。

在**测试环境验证后**，可在边缘/宿主机丢弃带 IPv4 选项的 TCP 报文，阻断文档中的主触发路径：

```bash
nft add table ip cve_2026_98432
nft add chain ip cve_2026_98432 prerouting '{ type filter hook prerouting priority -300; }'
nft add rule ip cve_2026_98432 prerouting meta l4proto tcp ip hdrlength != 5 drop

# 回滚
nft delete table ip cve_2026_98432
```

注意：该规则会丢弃所有携带 IPv4 选项的 TCP 流量（包括合法的 Record Route / Timestamp），
且不会持久化；如确认保留，请写入防火墙持久化配置。

### 4. 验证

```bash
ss -H -lMn                              # 期望无输出
sysctl net.mptcp.enabled                # 期望 0
sysctl net.ipv4.tcp_migrate_req         # 期望 0
nft list table ip cve_2026_98432        # 若启用，确认规则存在
```

## 参考

- 内核 CVE 公告：<https://git.kernel.org/pub/scm/linux/security/vulns.git/plain/cve/published/2026/CVE-2026-98432.mbox>
- 上游修复：<https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/commit/?id=e3f33b0a1d89d738e2b913dab519226eaf76e15b>
- NVD：<https://nvd.nist.gov/vuln/detail/CVE-2026-98432>
- Red Hat：<https://access.redhat.com/security/cve/cve-2026-98432>
- Ubuntu：<https://ubuntu.com/security/CVE-2026-98432>
- Go 安全团队关于 `multipathtcp` 的说明：<https://go.dev/doc/godebug>
