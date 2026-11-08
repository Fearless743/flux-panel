# rust-backend（生产后端）

Go 后端（已归档到分支 `archive/go-backend`）的 Rust 一比一实现：Axum + Tokio + sqlx/SQLite。生产一体镜像默认使用本实现。

- API：`/api/v1/*`、`/flow/*`、`/system-info`
- 契约：响应 `{code,msg,ts,data}`；JWT Header `Authorization: <token>`；密码 MD5；AES-256-GCM；SQLite schema 与 Go 版完全一致
- 迁移约定见 `PORTING.md`

## 本地运行

```bash
cd rust-backend
export PORT=6365
export DB_PATH=./data/gost.db
export JWT_SECRET=your-secret
cargo run
```

健康检查：`GET|POST /flow/test` → `test`

默认管理员：`admin_user` / `admin_user`

## 构建

```bash
cargo build --release
# 或
docker build -t flux-panel-rust-backend:dev .
```

## 契约对照

```bash
./scripts/parity.sh
```

同时启动 Go / Rust 后端（各自独立 SQLite），对比登录、配置、用户、节点、隧道、转发、
流量上报、备份导出、CORS 与 WebSocket 上线/离线等接口的规范化响应。生产镜像由仓库根
`Dockerfile` 构建 Rust 后端，Caddy 继续反代 `/api/*`、`/flow/*`、`/system-info*` 到
`127.0.0.1:6365`。
