# DEPRECATED — Spring Boot 后端已弃用

本目录为 **历史对照代码**，**不再参与构建、发布与部署**。

## 现状

| 项 | 说明 |
|----|------|
| 替代实现 | 仓库根目录 `rust-backend/` |
| 生产镜像 | `ghcr.io/fearless743/flux-panel` |
| CI | `.github/workflows/docker-build.yml` 已改为构建 Go 镜像；不再执行 `mvn package` |
| Compose | `docker-compose-v4.yml` / `docker-compose-v6.yml` 使用 `flux-panel` 镜像 |
| 契约 | Rust 后端保持 `/api/v1`、`/flow`、`/system-info`、JWT、AES-GCM、SQLite schema 兼容 |

## 为何保留目录

- 业务逻辑对照与回归参考  
- 未迁移的细节排查  

确认不再需要后，可删除整个 `springboot-backend/` 目录。

## 本地勿再发布 Java 镜像

```bash
# 正确
docker build -t ghcr.io/fearless743/flux-panel:latest .

# 错误（已弃用）
# docker build ./springboot-backend
```
