# Go → Rust 一比一迁移约定

本目录是 Go 后端（已归档到分支 `archive/go-backend`）的 Rust（Axum + Tokio + sqlx/SQLite）一比一实现，生产一体镜像默认使用本实现。
目标：API 路径、请求/响应 JSON、SQLite schema、JWT、AES-GCM、Gost 指令、定时任务行为与 Go 版一致。

## 已有基础设施（不要改）

- `src/state.rs`：`AppState { db: SqlitePool, jwt: Arc<Jwt>, hub: Arc<Hub>, config: Arc<Config> }`
- `src/error.rs`：`AppResult<T>` / `AppError(String)`；`?` 可直接转换 `sqlx::Error`。
- `src/response.rs`：`ok(Value)` / `ok_msg(&str, Value)` / `err(String)` / `unauthorized` / `forbidden`
- `src/model.rs`：所有表结构，已实现 `FromRow + Serialize + Deserialize`
- `src/repo/user.rs`、`src/service/user.rs`：已完成的参考实现
- `src/auth/`：JWT（HmacSHA256，字段与 Go 一致）、MD5 密码
- `src/crypto.rs`：AES-256-GCM（nonce+密文，标准 Base64），`get_or_create(secret)`
- `src/gost.rs`：服务命名、payload 构造、`is_ok` / `normalize_ok`
- `src/ws/hub.rs`：`Hub::send_msg(...).await`、`send_msg_timeout`、`send_no_wait`、`broadcast_admins`
- `src/handler/extract.rs`：`AppJson<T>`（解析失败返回 `{code:-1,msg:"参数错误"}`）
- `src/middleware.rs`：`AuthUser { user_id, role_id, name, token }`，`Extension<AuthUser>` 取当前用户

## 新代码约定

1. **repo 层**：自由函数，第一个参数 `&SqlitePool`，SQL 与 Go 版逐字一致；使用
   `sqlx::query_as::<_, Model>` / `query_scalar` / `query`，不要用编译期 `query!`。
2. **service 层**：自由 async 函数，第一个参数 `&AppState`（需要时再加 DTO）；错误用
   `AppError::new("中文文案")`，文案与 Go 一致。
3. **handler 层**：`pub async fn xxx(State(state): State<AppState>, ...) -> Json<Value>`；
   请求体用 `AppJson<T>`，当前用户用 `Extension<AuthUser>`；成功 `response::ok(json!(...))`，
   失败 `response::err(e.0)`。不要在 handler 里写业务逻辑。
4. **JSON 字段名**：DTO 用 `#[serde(rename_all = "camelCase")]`，与 Go 的 tag 对齐；
   注意 `role_id` 登录响应是 snake_case。
5. **数据库只有 1 条连接**（max_connections=1）：不要在持有事务时再调用走 pool 的函数；
   事务内所有语句用 `&mut *tx`。
6. **异步并发**：Go 的 `parallelEach` 可用 `futures_util::future::join_all` + `buffer_unordered`
   或顺序执行；行为（部分成功文案、错误顺序）要与 Go 一致。
7. **不要修改**：`Cargo.toml`、`src/main.rs`、`src/state.rs`、`src/response.rs`、
   `src/model.rs`、`src/auth/*`、`src/crypto.rs`、`src/gost.rs`、`src/ws/*`、`src/middleware.rs`、
   `src/handler/mod.rs`、`src/service/mod.rs`、`src/repo/mod.rs`。如需新依赖/共享函数，在最终回复中说明。
8. 只改分配给你的文件。完成后运行 `cargo check`，报告仍存在的错误（其他 agent 的文件可能未完成）。

## 跨域函数（tunnel / forward / node 互相调用）

forward 域必须公开：

```rust
pub fn build_service_name(forward_id: i64, user_id: i64, user_tunnel: Option<&model::UserTunnel>) -> String;
pub async fn add_or_update_service(
    state: &AppState, service_name: &str, limiter: Option<i64>,
    node: &model::Node, forward: &model::Forward, fp: &model::ForwardPort,
    tunnel: &model::Tunnel, method: &str, // "AddService" | "UpdateService"
) -> String; // 返回节点 msg
pub async fn perform_tcp_ping(state: &AppState, node: &model::Node, ip: &str, port: i32, desc: &str) -> DiagnosisResult;
// DiagnosisResult 字段与 forward.go 的 json tag 一致（nodeId/nodeName/.../fromChainType 等）。
pub async fn delete(state: &AppState, user: CurrentUser, forward_id: i64) -> AppResult<()>;
pub async fn update(state: &AppState, user: CurrentUser, req: ForwardUpdateReq) -> AppResult<()>;
```

`CurrentUser { user_id: i64, role_id: i32, user_name: String }` 定义在 `service/forward.rs`。

tunnel 域必须公开：

```rust
pub async fn detach_node_from_tunnels(state: &AppState, node_id: i64) -> AppResult<()>;
pub fn split_topology(cts: Vec<model::ChainTunnel>) -> (Vec<model::ChainTunnel>, Vec<Vec<model::ChainTunnel>>, Vec<model::ChainTunnel>);
pub async fn push_chains(state: &AppState, node_id: i64, target: &[model::ChainTunnel], nodes: &HashMap<i64, model::Node>) -> GostResult;
pub async fn push_chain_service(state: &AppState, node_id: i64, ct: &model::ChainTunnel, nodes: &HashMap<i64, model::Node>) -> GostResult;
```

node/flow 域可以调用 `service::forward` 和 `service::tunnel` 的上述函数。
