//! 限速字符串解析。
//!
//! 与 Go 版 `x/limiter/traffic/traffic.go::parseLimit` 对齐：
//!
//! ```text
//! "$ 1MB 1MB"      → key="$", in="1MB", out="1MB"
//! "$$ 500KB"       → key="$$", in="500KB", out=""
//! "1.2.3.4 1MB 2MB" → key="1.2.3.4", in/out
//! "10.0.0.0/8 ..." → key="10.0.0.0/8"
//! ```
//!
//! 字节单位解析（与 Go `alecthomas/units.ParseBase2Bytes` 一致）：
//! `B` / `KB` / `MB` / `GB` / `TB`（KiB/MiB... 也支持），大小写不敏感。

/// 解析一行限制表达式。
///
/// 返回 `(key, in_str, out_str)`。空行返回 `("", "", "")`。
pub fn parse_limits_line(s: &str) -> (String, String, String) {
    let s = s.replace('\t', " ");
    let s = s.trim();
    if s.is_empty() {
        return (String::new(), String::new(), String::new());
    }
    let parts: Vec<&str> = s.split(' ').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        return (String::new(), String::new(), String::new());
    }
    let key = parts[0].to_string();
    let in_str = parts[1].to_string();
    let out_str = if parts.len() > 2 {
        parts[2].to_string()
    } else {
        String::new()
    };
    (key, in_str, out_str)
}

/// 解析 base2 字节字符串为字节数。
///
/// 支持单位：`B`、`K`/`KB`/`KiB`、`M`/`MB`/`MiB`、`G`/`GB`/`GiB`、`T`/`TB`/`TiB`。
/// 不带单位视为字节数（需为纯数字）。
pub fn parse_base2_bytes(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let s = s.trim();
    let bytes = s.as_bytes();

    // 找到第一个非 [0-9] 字符的位置
    let mut unit_start = bytes.len();
    for (i, b) in bytes.iter().enumerate() {
        if !(b.is_ascii_digit()) {
            unit_start = i;
            break;
        }
    }
    let num_str = &s[..unit_start];
    let unit_str = s[unit_start..].trim().to_ascii_uppercase();

    let num: u64 = num_str.parse().ok()?;
    let mult: u64 = match unit_str.as_str() {
        "" | "B" => 1,
        "K" | "KB" | "KIB" => 1 << 10,
        "M" | "MB" | "MIB" => 1 << 20,
        "G" | "GB" | "GIB" => 1 << 30,
        "T" | "TB" | "TIB" => 1 << 40,
        _ => return None,
    };
    Some(num.saturating_mul(mult))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full() {
        let (k, i, o) = parse_limits_line("$ 1MB 2MB");
        assert_eq!(k, "$");
        assert_eq!(i, "1MB");
        assert_eq!(o, "2MB");
    }

    #[test]
    fn parse_no_out() {
        let (k, i, o) = parse_limits_line("$$ 500KB");
        assert_eq!(k, "$$");
        assert_eq!(i, "500KB");
        assert_eq!(o, "");
    }

    #[test]
    fn parse_tabs_and_spaces() {
        let (k, i, o) = parse_limits_line("1.2.3.4\t10MB   20MB");
        assert_eq!(k, "1.2.3.4");
        assert_eq!(i, "10MB");
        assert_eq!(o, "20MB");
    }

    #[test]
    fn parse_empty() {
        let (k, i, o) = parse_limits_line("");
        assert_eq!(k, "");
        assert_eq!(i, "");
        assert_eq!(o, "");
    }

    #[test]
    fn bytes_units() {
        assert_eq!(parse_base2_bytes("1"), Some(1));
        assert_eq!(parse_base2_bytes("1B"), Some(1));
        assert_eq!(parse_base2_bytes("1KB"), Some(1024));
        assert_eq!(parse_base2_bytes("1MB"), Some(1024 * 1024));
        assert_eq!(parse_base2_bytes("1GB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_base2_bytes("1.5MB"), None); // 浮点不支持
    }

    #[test]
    fn bytes_case_insensitive() {
        assert_eq!(parse_base2_bytes("1mb"), Some(1024 * 1024));
        assert_eq!(parse_base2_bytes("1MiB"), Some(1024 * 1024));
    }
}