//! gost.json 原子持久化。
//!
//! 设计要点：
//! - 写入流程：写 `.tmp` → fsync → rename 到目标路径（POSIX 保证原子性）
//! - 写入时**不**调用 serde_json::to_string_pretty，而是显式序列化保留字段顺序以便 diff 友好
//! - 备份策略：可选保留 `.bak`；默认关闭，避免磁盘浪费
//!
//! 与 Go 版 `gost.Run()` 中 `gost.yml` → `gost.json` 的导出格式保持一致；
//! Java 端 WebSocketServer 收到的 `Config` 字段就是同样的 camelCase 结构。

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::types::Config;

/// 把 [`Config`] 原子写入 `path`。
///
/// 流程：
/// 1. 把 JSON 序列化到字节数组
/// 2. 写到 `<path>.tmp`
/// 3. fsync `.tmp` 强制落盘
/// 4. rename `.tmp` → `path`
///
/// 如果 `path` 已存在，会被覆盖；旧文件在 `rename` 成功后立即被替换（POSIX 语义）。
pub fn save_config<P: AsRef<Path>>(path: P, config: &Config) -> Result<()> {
    let path = path.as_ref();
    let tmp = tmp_path(path);

    // 1. 序列化
    let bytes = serde_json::to_vec_pretty(config).context("序列化 Config 失败")?;

    // 2. 写 .tmp
    {
        let mut f = fs::File::create(&tmp)
            .with_context(|| format!("创建临时文件 {} 失败", tmp.display()))?;
        f.write_all(&bytes)
            .with_context(|| format!("写入临时文件 {} 失败", tmp.display()))?;
        f.sync_all()
            .with_context(|| format!("fsync {} 失败", tmp.display()))?;
    }

    // 3. 原子 rename
    fs::rename(&tmp, path).with_context(|| {
        format!("rename {} -> {} 失败", tmp.display(), path.display())
    })?;

    Ok(())
}

/// 从 `path` 读取 [`Config`]（与 `save_config` 配对）。
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let path = path.as_ref();
    let data = fs::read(path)
        .with_context(|| format!("读取 {} 失败（请确认目录存在或 gost.json 已创建）", path.display()))?;
    let cfg: Config = serde_json::from_slice(&data)
        .with_context(|| format!("解析 {} 失败", path.display()))?;
    Ok(cfg)
}

/// 若 `path` 不存在，创建一个空的 Config（仅有空 services/chains/hops）。
///
/// 启动时调用，避免 `fs::read` 失败导致程序退出。
pub fn ensure_config<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        save_config(path, &Config::default())?;
    }
    Ok(())
}

/// 计算 `.tmp` 文件路径：`<path>.tmp`。
fn tmp_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".tmp");
    PathBuf::from(s)
}

/// 备份当前 `path` 到 `path.bak`（可选操作；调用方决定时机）。
///
/// 一般在 `save_config` 前调用，便于故障回滚。
pub fn backup_to_bak<P: AsRef<Path>>(path: P) -> io::Result<Option<PathBuf>> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(None);
    }
    let mut bak = path.as_os_str().to_owned();
    bak.push(".bak");
    let bak = PathBuf::from(bak);
    fs::copy(path, &bak)?;
    Ok(Some(bak))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn tmpfile(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("flux_panel_gost_rs_test");
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    #[test]
    fn save_then_load_round_trip() {
        let path = tmpfile("rt_basic.json");

        let mut cfg = Config::default();
        let mut meta = HashMap::new();
        meta.insert("paused".into(), serde_json::Value::Bool(false));
        cfg.services.push(super::super::types::ServiceConfig {
            name: "svc_1".into(),
            addr: Some("0.0.0.0:7000".into()),
            listener: Some(super::super::types::ListenerConfig {
                r#type: "tcp".into(),
                ..Default::default()
            }),
            handler: Some(super::super::types::HandlerConfig {
                r#type: "tcp".into(),
                ..Default::default()
            }),
            metadata: meta,
            ..Default::default()
        });

        save_config(&path, &cfg).unwrap();
        let loaded = load_config(&path).unwrap();
        assert_eq!(loaded.services.len(), 1);
        assert_eq!(loaded.services[0].name, "svc_1");
        assert_eq!(loaded.services[0].addr.as_deref(), Some("0.0.0.0:7000"));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn ensure_creates_empty_if_missing() {
        let path = tmpfile("ensure_missing.json");
        let _ = fs::remove_file(&path);

        ensure_config(&path).unwrap();
        assert!(path.exists());
        let cfg = load_config(&path).unwrap();
        assert_eq!(cfg.services.len(), 0);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_overwrites_existing() {
        let path = tmpfile("overwrite.json");

        save_config(&path, &Config::default()).unwrap();
        let size1 = fs::metadata(&path).unwrap().len();

        let mut cfg = Config::default();
        cfg.services.push(super::super::types::ServiceConfig {
            name: "x".into(),
            ..Default::default()
        });
        save_config(&path, &cfg).unwrap();
        let size2 = fs::metadata(&path).unwrap().len();

        assert!(size2 > size1, "二次写入应大于空 Config 的体积");
        let loaded = load_config(&path).unwrap();
        assert_eq!(loaded.services.len(), 1);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn tmp_path_appends_suffix() {
        let p = Path::new("/etc/flux_agent/gost.json");
        assert_eq!(tmp_path(p), Path::new("/etc/flux_agent/gost.json.tmp"));
    }

    #[test]
    fn backup_creates_bak() {
        let path = tmpfile("backup_test.json");
        save_config(&path, &Config::default()).unwrap();
        let bak = backup_to_bak(&path).unwrap().unwrap();
        assert!(bak.exists());
        assert_eq!(
            bak,
            PathBuf::from(format!("{}.bak", path.display()))
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&bak);
    }
}