//! Go `time.Duration` 解析与格式化。
//!
//! ## 协议契约
//!
//! Go 的 `time.Duration` 是 `int64` 纳秒。序列化为 JSON 时存在两种形式：
//!
//! 1. **数字（纳秒）**：`600000000000`
//! 2. **字符串（人类可读）**：`"10m0s"` / `"600s"` / `"1h30m"` / `"300ms"`
//!
//! gost 在 `x/config/parsing/parsing.go` 的 `processDurationInData` 会**预处理**
//! 服务端下发的 JSON，把字符串形式递归转换为纳秒整数再交给业务解析。
//!
//! ## Rust 端方案
//!
//! 由于字段类型固定为 `i64` 纳秒，最简单做法是：
//! 1. 反序列化前用 `serde_json::Value` 中转；
//! 2. 对所有 i64 字段尝试字符串解析；
//! 3. 再用 `#[serde(deserialize_with = "...")]` 转换。
//!
//! 但这样侵入性大。**当前实现**：
//! - **解析侧**：导出 [`parse_duration`] 接收字符串 → 纳秒 i64；
//! - **解析侧**：导出 [`parse_duration_or_nanos`] 接收 `serde_json::Value` → 纳秒；
//! - **格式化侧**：导出 [`format_duration`] 把纳秒 → "600s" / "1h30m" 形式（与 Go 一致）。
//!
//! 阶段 3 在反序列化 gost.json 时，统一调用 [`parse_duration_or_nanos`]。
//!
//! ## 单位支持
//!
//! - `ns` 纳秒
//! - `us` / `µs` 微秒
//! - `ms` 毫秒
//! - `s`  秒
//! - `m`  分钟
//! - `h`  小时

use anyhow::{Context, Result};

/// 把 Go time.Duration 字符串解析为纳秒。
///
/// # 格式
///
/// 形式为 `\[-\]?\[0-9\.\]+\[ns|us|µs|ms|s|m|h\]` 的若干个组合：
///
/// ```
/// use flux_agent::util::duration::parse_duration;
/// assert_eq!(parse_duration("300ms").unwrap(), 300_000_000);
/// assert_eq!(parse_duration("10s").unwrap(), 10_000_000_000);
/// assert_eq!(parse_duration("1h30m").unwrap(), 5_400_000_000_000);
/// ```
///
/// 与 Go `time.ParseDuration` 语义一致：单位必须紧贴数字，
/// 多个单位可以串联（`"1h30m"`、`"2h45m30s"`）。
pub fn parse_duration(s: &str) -> Result<i64> {
    let s = s.trim();
    if s.is_empty() {
        anyhow::bail!("空字符串无法解析为 duration");
    }

    // 处理负号前缀
    let (neg, body) = if let Some(rest) = s.strip_prefix('-') {
        (true, rest)
    } else {
        (false, s)
    };

    let mut total: i64 = 0;
    let mut num_start = 0;
    let mut found_any = false;
    let bytes = body.as_bytes();

    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_digit() || c == b'.' {
            i += 1;
            continue;
        }

        // 找到数字边界：bytes[num_start..i] 是数字部分
        let num_str = &body[num_start..i];
        let num: f64 = num_str
            .parse()
            .with_context(|| format!("duration 数字解析失败：{num_str:?}"))?;

        let unit_str = &body[i..];
        let (unit_value, consumed) = match unit_str.chars().next() {
            Some('n') if unit_str.starts_with("ns") => (1i64, 2),
            Some('u') if unit_str.starts_with("us") => (1_000, 2),
            Some('µ') if unit_str.starts_with("µs") => (1_000, 2),
            Some('m') if unit_str.starts_with("ms") => (1_000_000, 2),
            Some('s') => (1_000_000_000, 1),
            Some('m') => (60 * 1_000_000_000, 1),
            Some('h') => (3600 * 1_000_000_000, 1),
            other => anyhow::bail!("duration 单位未知：{other:?}"),
        };

        total = total
            .checked_add((num * unit_value as f64) as i64)
            .context("duration 溢出")?;

        i += consumed;
        num_start = i;
        found_any = true;
    }

    // 尾部如果没有单位（例如纯数字），把整段当纳秒
    if !found_any && num_start < bytes.len() {
        let num_str = &body[num_start..];
        let num: i64 = num_str
            .parse()
            .with_context(|| format!("duration 数字解析失败：{num_str:?}"))?;
        total = total.checked_add(num).context("duration 溢出")?;
    }

    if neg {
        total = -total;
    }

    Ok(total)
}

/// 把 `serde_json::Value`（字符串或数字）解析为纳秒。
///
/// 用法示例（在自定义 serde `deserialize_with` 中）：
///
/// ```ignore
/// fn de_duration<'de, D: serde::Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
///    use serde::Deserialize;
///    let v = serde_json::Value::deserialize(d)?;
///    crate::util::duration::parse_duration_or_nanos(v).map_err(serde::de::Error::custom)
/// }
/// ```
pub fn parse_duration_or_nanos(v: serde_json::Value) -> Result<i64> {
    match v {
        serde_json::Value::Number(n) => n
            .as_i64()
            .context("duration 数字超出 i64 范围"),
        serde_json::Value::String(s) => parse_duration(&s),
        serde_json::Value::Null => Ok(0),
        other => anyhow::bail!("duration 类型不支持：{other:?}"),
    }
}

/// 把纳秒格式化为 Go 风格字符串。
///
/// 优先选择最大单位，避免 `3600s` 而不是 `1h`。
///
/// # 示例
///
/// ```
/// use flux_agent::util::duration::format_duration;
/// assert_eq!(format_duration(0), "0s");
/// assert_eq!(format_duration(300_000_000), "300ms");
/// assert_eq!(format_duration(10_000_000_000), "10s");
/// assert_eq!(format_duration(5_400_000_000_000), "1h30m");
/// ```
pub fn format_duration(ns: i64) -> String {
    if ns == 0 {
        return "0s".into();
    }

    let neg = ns < 0;
    let mut n = ns.unsigned_abs() as u64;

    let h = n / 3_600_000_000_000;
    n %= 3_600_000_000_000;
    let m = n / 60_000_000_000;
    n %= 60_000_000_000;
    let s = n / 1_000_000_000;
    n %= 1_000_000_000;
    let ms = n / 1_000_000;
    let us = (n % 1_000_000) / 1_000;
    let rest_ns = n % 1_000;

    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if h > 0 {
        out.push_str(&format!("{h}h"));
    }
    if m > 0 {
        out.push_str(&format!("{m}m"));
    }
    if s > 0 {
        out.push_str(&format!("{s}s"));
    }
    if ms > 0 {
        out.push_str(&format!("{ms}ms"));
    }
    if us > 0 {
        out.push_str(&format!("{us}us"));
    }
    if rest_ns > 0 {
        out.push_str(&format!("{rest_ns}ns"));
    }

    if out.is_empty() || out == "-" {
        out.push('0');
        out.push('s');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_basic_units() {
        assert_eq!(parse_duration("300ms").unwrap(), 300_000_000);
        assert_eq!(parse_duration("1s").unwrap(), 1_000_000_000);
        assert_eq!(parse_duration("10s").unwrap(), 10_000_000_000);
        assert_eq!(parse_duration("1m").unwrap(), 60_000_000_000);
        assert_eq!(parse_duration("1h").unwrap(), 3_600_000_000_000);
    }

    #[test]
    fn parse_combined() {
        assert_eq!(
            parse_duration("1h30m").unwrap(),
            3_600_000_000_000 + 30 * 60_000_000_000
        );
        assert_eq!(
            parse_duration("2h45m30s").unwrap(),
            2 * 3_600_000_000_000 + 45 * 60_000_000_000 + 30 * 1_000_000_000
        );
        assert_eq!(
            parse_duration("500ms500us").unwrap(),
            500_000_000 + 500_000
        );
    }

    #[test]
    fn parse_negative() {
        assert_eq!(parse_duration("-1s").unwrap(), -1_000_000_000);
        assert_eq!(parse_duration("-300ms").unwrap(), -300_000_000);
    }

    #[test]
    fn parse_micro_and_nano() {
        assert_eq!(parse_duration("5us").unwrap(), 5_000);
        assert_eq!(parse_duration("100ns").unwrap(), 100);
    }

    #[test]
    fn parse_whitespace() {
        assert_eq!(parse_duration("  10s  ").unwrap(), 10_000_000_000);
    }

    #[test]
    fn parse_invalid() {
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("10xs").is_err());
        assert!(parse_duration("").is_err());
    }

    #[test]
    fn parse_or_nanos_from_number() {
        assert_eq!(
            parse_duration_or_nanos(json!(600_000_000_000_i64)).unwrap(),
            600_000_000_000
        );
    }

    #[test]
    fn parse_or_nanos_from_string() {
        assert_eq!(
            parse_duration_or_nanos(json!("600s")).unwrap(),
            600_000_000_000
        );
    }

    #[test]
    fn parse_or_nanos_null() {
        assert_eq!(parse_duration_or_nanos(json!(null)).unwrap(), 0);
    }

    #[test]
    fn format_zero() {
        assert_eq!(format_duration(0), "0s");
    }

    #[test]
    fn format_single_units() {
        assert_eq!(format_duration(100), "100ns");
        assert_eq!(format_duration(5_000), "5us");
        assert_eq!(format_duration(300_000_000), "300ms");
        assert_eq!(format_duration(10_000_000_000), "10s");
        assert_eq!(format_duration(60_000_000_000), "1m");
        assert_eq!(format_duration(3_600_000_000_000), "1h");
    }

    #[test]
    fn format_combined() {
        // 紧凑形式：省略尾部 0 单位（与 Go 一致：1h30m 而非 1h30m0s）
        assert_eq!(format_duration(5_400_000_000_000), "1h30m");
        assert_eq!(
            format_duration(2 * 3_600_000_000_000 + 45 * 60_000_000_000 + 30_000_000_000),
            "2h45m30s"
        );
    }

    #[test]
    fn format_negative() {
        assert_eq!(format_duration(-1_000_000_000), "-1s");
    }

    #[test]
    fn round_trip_format_parse() {
        let original = 9_999_999_999i64; // 9.999... s
        let s = format_duration(original);
        let parsed = parse_duration(&s).unwrap();
        // format 会拆成 "9s999ms999us999ns"，parse 回来应该相等
        assert_eq!(parsed, original);
    }
}