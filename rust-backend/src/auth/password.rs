use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use md5::{Digest, Md5};
use rand::rngs::OsRng;

/// 对齐历史 Java：MD5 十六进制小写（仅用于校验存量哈希，新写入一律 Argon2id）
pub fn md5_hex(s: &str) -> String {
    let digest = Md5::digest(s.as_bytes());
    let mut out = String::with_capacity(32);
    for b in digest {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn argon2_hasher() -> Argon2<'static> {
    Argon2::default()
}

/// 新密码哈希（PHC 串，含随机盐）
pub fn hash_password(plain: &str) -> Result<String, String> {
    validate_password_strength(plain)?;
    let salt = SaltString::generate(&mut OsRng);
    argon2_hasher()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

pub fn validate_password_strength(plain: &str) -> Result<(), String> {
    if plain.trim().is_empty() {
        return Err("密码不能为空".to_string());
    }
    if plain.chars().count() < 6 {
        return Err("密码长度至少 6 位".to_string());
    }
    if plain.chars().count() > 128 {
        return Err("密码过长".to_string());
    }
    Ok(())
}

pub fn is_argon2_hash(stored: &str) -> bool {
    stored.starts_with("$argon2")
}

/// 兼容校验：Argon2id 首选，32 位 hex 视为存量 MD5
pub fn check_password(plain: &str, stored: &str) -> bool {
    if stored.is_empty() || plain.is_empty() {
        return false;
    }
    if is_argon2_hash(stored) {
        let Ok(parsed) = PasswordHash::new(stored) else {
            return false;
        };
        return argon2_hasher()
            .verify_password(plain.as_bytes(), &parsed)
            .is_ok();
    }
    // 存量 MD5（忽略大小写，保持历史行为）
    md5_hex(plain).eq_ignore_ascii_case(stored)
}

/// 存量 MD5 登录成功后是否需要升级为 Argon2
pub fn needs_rehash(stored: &str) -> bool {
    !is_argon2_hash(stored)
}
