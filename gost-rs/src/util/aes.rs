//! AES-256-GCM 加解密（与 Java springboot-backend 完全兼容）。
//!
//! ## 协议契约（与 `AESCrypto.java` 1:1）
//!
//! 1. **密钥派生**：`SHA-256(secret)` 取前 32 字节作为 AES-256 key。
//! 2. **算法**：`AES/GCM/NoPadding`，tag 长度 16 字节（128 bits）。
//! 3. **Nonce**：12 字节随机（Java `SecureRandom` / Rust `rand::thread_rng`）。
//! 4. **输出格式**：`base64(nonce || ciphertext_with_tag)`。
//!    - Java 端：`base64.encodeToString(iv + cipher.doFinal(plain))`
//!    - Rust 端：等价组装后 `base64::encode(...)`
//!
//! ## 与 gost 差异
//!
//! gost 自带 `x/internal/util/crypto/aes.go` 实现的是同样逻辑；
//! 但 gost 节点端通过 HTTP 上报时**只**对 `data` 字段加密、不对 `requestId` 加密，
//! 与 Java springboot-backend 的解密逻辑完全一致。
//!
//! ## 用法
//!
//! ```
//! use flux_agent::util::aes::{encrypt, decrypt};
//!
//! let ct = encrypt(b"hello world", "my-secret").unwrap();
//! let pt = decrypt(&ct, "my-secret").unwrap();
//! assert_eq!(pt, b"hello world");
//! ```

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// 派生 32 字节 AES-256 key（SHA-256 前 32 字节）。
///
/// Java 端：`MessageDigest.getInstance("SHA-256").digest(secret.getBytes(UTF_8))`
pub fn derive_key(secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let out = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&out[..32]);
    key
}

/// 生成 12 字节随机 nonce。
///
/// Java 端：`new SecureRandom().nextBytes(iv);` 其中 `iv = new byte[12]`
fn random_nonce() -> [u8; 12] {
    let mut n = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut n);
    n
}

/// 加密明文 → base64(nonce || ciphertext_with_tag)。
///
/// `plain` 可以是任意字节（HTTP 上报时是 AES 加密 + gzip 压缩后的 gost.json 字节）。
pub fn encrypt(plain: &[u8], secret: &str) -> Result<String> {
    encrypt_with_aad(plain, &[], secret)
}

/// 加密并附加 AAD（Authenticated Additional Data）。
///
/// gost 与 Java 端默认 **不**使用 AAD；但保留此接口以便未来扩展
/// （例如对 `requestId` 做关联认证）。
pub fn encrypt_with_aad(plain: &[u8], aad: &[u8], secret: &str) -> Result<String> {
    let key = derive_key(secret);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce_bytes = random_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plain,
                aad,
            },
        )
        .map_err(|e| anyhow::anyhow!("AES-GCM 加密失败：{e}"))?;

    // 组装 nonce || ciphertext（tag 已包含在 ct 末尾 16 字节）
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);

    Ok(STANDARD.encode(&out))
}

/// 解密 base64(nonce || ciphertext_with_tag) → 明文。
///
/// 输入格式：base64 字符串；与 [`encrypt`] 输出互逆。
pub fn decrypt(b64: &str, secret: &str) -> Result<Vec<u8>> {
    decrypt_with_aad(b64, &[], secret)
}

/// 解密并校验 AAD。
pub fn decrypt_with_aad(b64: &str, aad: &[u8], secret: &str) -> Result<Vec<u8>> {
    let raw = STANDARD
        .decode(b64)
        .context("base64 解码失败")?;

    if raw.len() < 12 + 16 {
        anyhow::bail!(
            "密文过短：{} 字节（最少需 12 nonce + 16 tag）",
            raw.len()
        );
    }

    let (nonce_bytes, ct) = raw.split_at(12);
    let key = derive_key(secret);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ct,
                aad,
            },
        )
        .map_err(|e| anyhow::anyhow!("AES-GCM 解密失败：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_basic() {
        let ct = encrypt(b"hello world", "secret").unwrap();
        let pt = decrypt(&ct, "secret").unwrap();
        assert_eq!(pt, b"hello world");
    }

    #[test]
    fn round_trip_empty() {
        let ct = encrypt(b"", "secret").unwrap();
        let pt = decrypt(&ct, "secret").unwrap();
        assert_eq!(pt, b"");
    }

    #[test]
    fn round_trip_long() {
        let data: Vec<u8> = (0..4096).map(|i| (i % 251) as u8).collect();
        let ct = encrypt(&data, "p@ssw0rd").unwrap();
        let pt = decrypt(&ct, "p@ssw0rd").unwrap();
        assert_eq!(pt, data);
    }

    #[test]
    fn wrong_secret_fails() {
        let ct = encrypt(b"secret", "right").unwrap();
        let err = decrypt(&ct, "wrong").unwrap_err();
        assert!(
            err.to_string().contains("AES-GCM"),
            "应返回 AES-GCM 错误：{err}"
        );
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let ct = encrypt(b"secret", "k").unwrap();
        // 翻转最后 1 字节（破坏 tag 校验）
        let mut raw = STANDARD.decode(&ct).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0x01;
        let bad = STANDARD.encode(&raw);
        assert!(decrypt(&bad, "k").is_err());
    }

    #[test]
    fn key_derivation_deterministic() {
        // SHA-256 前 32 字节已知向量（RFC 没有这种 vector，自校验）
        let k1 = derive_key("abc");
        let k2 = derive_key("abc");
        assert_eq!(k1, k2);

        // 验证长度
        assert_eq!(k1.len(), 32);
        // 不同 secret 应得到不同 key
        let k3 = derive_key("abd");
        assert_ne!(k1, k3);
    }

    #[test]
    fn nonce_is_random() {
        // 两次加密同样明文应得到不同密文（nonce 随机）
        let c1 = encrypt(b"same", "k").unwrap();
        let c2 = encrypt(b"same", "k").unwrap();
        assert_ne!(c1, c2, "nonce 必须随机，否则破坏语义安全");
    }

    #[test]
    fn base64_output_format() {
        // 输出必须以 base64 字符串呈现，且非空
        let ct = encrypt(b"data", "k").unwrap();
        assert!(!ct.is_empty());
        // base64 字符集
        for c in ct.bytes() {
            assert!(
                c.is_ascii_alphanumeric() || c == b'+' || c == b'/' || c == b'=',
                "非 base64 字符：{c}"
            );
        }
    }

    /// Java AESCrypto.encrypt 兼容性向量。
    ///
    /// 构造方式（Java 等价代码）：
    /// ```java
    /// MessageDigest sha = MessageDigest.getInstance("SHA-256");
    /// byte[] key = sha.digest("test-secret".getBytes(UTF_8));
    /// SecretKeySpec spec = new SecretKeySpec(key, "AES");
    /// Cipher c = Cipher.getInstance("AES/GCM/NoPadding");
    /// byte[] iv = new byte[12];
    /// new SecureRandom().nextBytes(iv);
    /// c.init(Cipher.ENCRYPT_MODE, spec, new GCMParameterSpec(128, iv));
    /// byte[] ct = c.doFinal("hello".getBytes(UTF_8));
    /// String out = Base64.getEncoder().encodeToString(iv + ct);
    /// ```
    /// 我们在测试里用 Rust 解密自己加密的密文来间接验证：
    /// round-trip = encrypt + decrypt = 原文。
    /// 直接 KAT 需要在 Java 端预先生成；这里只验证语义。
    #[test]
    fn java_compat_indirect() {
        let plain = br#"{"type":"PING","requestId":"abc"}"#;
        let ct = encrypt(plain, "test-secret").unwrap();
        let pt = decrypt(&ct, "test-secret").unwrap();
        assert_eq!(pt, plain);
    }
}