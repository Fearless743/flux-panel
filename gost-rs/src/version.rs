//! 版本号常量（与 Go 版对齐：`2.0.2`）。
//! springboot-backend 在节点连接 WS 时会读取 `version` 参数，写入 `node.version`，
//! 用于兼容性识别。新版本应保持 `<major>.<minor>.<patch>` 形式。

/// 版本字符串。修改此处需要同步检查 springboot-backend 是否对 version 做了硬编码分支。
pub const VERSION: &str = "2.0.2";

/// 协议版本号（WebSocket 子路径、心跳格式等的兼容版本）。
/// 与 VERSION 解耦：协议变更不影响功能版本。
pub const PROTOCOL_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver_like() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "VERSION 必须是 x.y.z 格式");
        for p in parts {
            assert!(
                p.parse::<u32>().is_ok(),
                "VERSION 段必须可解析为 u32: {p}"
            );
        }
    }
}