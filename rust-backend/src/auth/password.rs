use md5::{Digest, Md5};

/// 对齐 Java Md5Util：密码 MD5 十六进制小写存储
pub fn md5_hex(s: &str) -> String {
    let digest = Md5::digest(s.as_bytes());
    let mut out = String::with_capacity(32);
    for b in digest {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

pub fn check_password(plain: &str, hashed: &str) -> bool {
    md5_hex(plain).eq_ignore_ascii_case(hashed)
}
