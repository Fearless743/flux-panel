//! AES + gzip 组合：面板协议下行解密/上行加密。
//!
//! ## 协议细节（与 Java springboot-backend 1:1）
//!
//! - **下行（面板 → 节点）**：
//!   1. WS 收到 binary frame
//!   2. 解析为 `{type, compressed, data, requestId}`（gzip 压缩数据时）
//!   3. 若 `compressed=true`：先 `gunzip(data)` → 再 `decrypt(plaintext, secret)`
//!   4. 解密结果为 UTF-8 JSON `Command`
//!
//! - **上行（节点 → 面板）**：
//!   1. 序列化 `Response` 为 JSON UTF-8 字节
//!   2. `encrypt(json_bytes, secret)` → base64
//!   3. 拼成 `{type, data: "<base64>"}` → WS binary frame
//!
//! - **上行 流量/配置 HTTP**：
//!   `POST /flow/upload`、`/flow/config`：body = `encrypt(json, secret)`
//!   响应：`"ok"`（明文字符串）

use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::util::aes::{decrypt as aes_decrypt, encrypt as aes_encrypt};

use super::compressed::{gunzip, gzip};

/// 解密 + 解压（如有）。
///
/// 输入：base64(nonce || ciphertext)，可能先经过 gzip。
/// 流程：base64_decode → maybe gunzip → aes_decrypt。
pub fn decrypt_payload(b64_or_compressed: &str, secret: &str) -> Result<Vec<u8>> {
    let raw = STANDARD.decode(b64_or_compressed).context("base64 解码失败")?;
    // Java 端：若 compressed=true 先 gzip 解压得到密文，再 AES 解密。
    // 区分 gzip 头：0x1f 0x8b
    let to_decrypt = if raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b {
        gunzip(&raw)?
    } else {
        raw
    };
    aes_decrypt(&STANDARD.encode(&to_decrypt), secret).map_err(Into::into)
}

/// 加密 + 可选压缩。
///
/// 流程：encrypt(plain) → base64 → maybe gzip。
pub fn encrypt_payload(plain: &[u8], secret: &str, compress: bool) -> Result<String> {
    let encrypted = aes_encrypt(plain, secret)?;
    if compress {
        let raw = STANDARD.decode(&encrypted).context("base64 自解码失败")?;
        let gz = gzip(&raw)?;
        Ok(STANDARD.encode(&gz))
    } else {
        Ok(encrypted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_plain() {
        let plain = br#"{"type":"PING","requestId":"abc"}"#;
        let b64 = encrypt_payload(plain, "secret", false).unwrap();
        let out = decrypt_payload(&b64, "secret").unwrap();
        assert_eq!(out, plain);
    }

    #[test]
    fn round_trip_compressed() {
        let plain = br#"{"type":"CFG","data":"long json to compress"}"#;
        let b64 = encrypt_payload(plain, "secret", true).unwrap();
        let out = decrypt_payload(&b64, "secret").unwrap();
        assert_eq!(out, plain);
    }

    #[test]
    fn wrong_secret_fails() {
        let plain = b"secret";
        let b64 = encrypt_payload(plain, "right", false).unwrap();
        assert!(decrypt_payload(&b64, "wrong").is_err());
    }
}