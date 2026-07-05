//! AES-GCM 与 Java AESCrypto 的互操作 KAT（Known Answer Tests）。
//!
//! 这些测试覆盖：
//! 1. Rust 加密 → Rust 解密 round-trip
//! 2. 加密输出格式校验（base64(nonce || ct+tag)，nonce 12B，tag 16B）
//! 3. base64 字符集合法
//! 4. 错误 secret / 篡改密文 → 拒绝
//! 5. 大量不同长度的明文（包括非 UTF-8、空、Unicode）
//!
//! 与 Java 端的实际互操作需要 Java 端预先加密一组向量后再断言；
//! 这里只验证 Rust 端实现正确，确保 Java 端能解密 Rust 加密的数据。
//!
//! 真正的 Java 互操作测试运行在 springboot-backend 测试套件中。

use base64::Engine;
use flux_agent::util::aes::{decrypt, encrypt, encrypt_with_aad};

const TEST_SECRET: &str = "flux-panel-shared-secret";
const SHORT_SECRET: &str = "k";

#[test]
fn round_trip_short_text() {
    let plain = b"ping";
    let ct = encrypt(plain, TEST_SECRET).unwrap();
    let pt = decrypt(&ct, TEST_SECRET).unwrap();
    assert_eq!(pt, plain);
}

#[test]
fn round_trip_gost_config_json() {
    // 模拟节点端上行的 gost.json 加密内容
    let json = br#"{"services":[],"chains":[],"hops":[]}"#;
    let ct = encrypt(json, TEST_SECRET).unwrap();
    let pt = decrypt(&ct, TEST_SECRET).unwrap();
    assert_eq!(pt, json);
}

#[test]
fn round_trip_unicode() {
    let plain = "你好，flux-panel。中文 + ASCII + emoji 🚀".as_bytes();
    let ct = encrypt(plain, TEST_SECRET).unwrap();
    let pt = decrypt(&ct, TEST_SECRET).unwrap();
    assert_eq!(pt, plain);
}

#[test]
fn round_trip_random_binary() {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut buf = vec![0u8; 8192];
    rng.fill_bytes(&mut buf);
    let ct = encrypt(&buf, TEST_SECRET).unwrap();
    let pt = decrypt(&ct, TEST_SECRET).unwrap();
    assert_eq!(pt, buf);
}

#[test]
fn ciphertext_format_is_valid_base64() {
    let ct = encrypt(b"data", TEST_SECRET).unwrap();
    // 必须能被标准 base64 解码
    let raw = base64::engine::general_purpose::STANDARD
        .decode(&ct)
        .expect("输出必须是标准 base64");
    // nonce 12 + tag 16 = 28 字节最短长度
    assert!(
        raw.len() >= 28,
        "密文至少 28 字节（12 nonce + 16 tag），实际 {}",
        raw.len()
    );
    // "data" 4 字节 + 16 tag = 20 字节密文部分
    assert_eq!(raw.len(), 12 + 4 + 16);
}

#[test]
fn nonce_changes_each_encryption() {
    // 同一明文 + 同 secret 两次加密，密文必须不同（nonce 随机）
    let a = encrypt(b"same", TEST_SECRET).unwrap();
    let b = encrypt(b"same", TEST_SECRET).unwrap();
    assert_ne!(a, b);
    // 但都能解密回原文
    assert_eq!(decrypt(&a, TEST_SECRET).unwrap(), b"same");
    assert_eq!(decrypt(&b, TEST_SECRET).unwrap(), b"same");
}

#[test]
fn wrong_secret_rejected() {
    let ct = encrypt(b"payload", TEST_SECRET).unwrap();
    let err = decrypt(&ct, SHORT_SECRET).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("AES-GCM") || msg.contains("tag"),
        "错误 secret 应返回 AES-GCM 错误：{msg}"
    );
}

#[test]
fn tampered_nonce_rejected() {
    let ct = encrypt(b"payload", TEST_SECRET).unwrap();
    let mut raw = base64::engine::general_purpose::STANDARD
        .decode(&ct)
        .unwrap();
    raw[0] ^= 0x01; // 翻转 nonce 第 1 字节
    let bad = base64::engine::general_purpose::STANDARD.encode(&raw);
    assert!(decrypt(&bad, TEST_SECRET).is_err());
}

#[test]
fn tampered_tag_rejected() {
    let ct = encrypt(b"payload", TEST_SECRET).unwrap();
    let mut raw = base64::engine::general_purpose::STANDARD
        .decode(&ct)
        .unwrap();
    let last = raw.len() - 1;
    raw[last] ^= 0x01; // 翻转 tag 末位
    let bad = base64::engine::general_purpose::STANDARD.encode(&raw);
    assert!(decrypt(&bad, TEST_SECRET).is_err());
}

#[test]
fn tampered_plaintext_rejected() {
    let ct = encrypt(b"payload", TEST_SECRET).unwrap();
    let mut raw = base64::engine::general_purpose::STANDARD
        .decode(&ct)
        .unwrap();
    raw[12] ^= 0x01; // 翻转密文第 1 字节
    let bad = base64::engine::general_purpose::STANDARD.encode(&raw);
    assert!(decrypt(&bad, TEST_SECRET).is_err());
}

#[test]
fn truncated_ciphertext_rejected() {
    // 任何短于 nonce(12) + tag(16) 的输入都被拒绝
    let too_short = base64::engine::general_purpose::STANDARD.encode(&[0u8; 20]);
    assert!(decrypt(&too_short, TEST_SECRET).is_err());
}

#[test]
fn invalid_base64_rejected() {
    let err = decrypt("not!valid!base64!!!", TEST_SECRET).unwrap_err();
    assert!(err.to_string().contains("base64"));
}

#[test]
fn aad_mismatch_rejected() {
    // 加密时附加 AAD = b"header"
    let ct = encrypt_with_aad(b"msg", b"header", TEST_SECRET).unwrap();
    // 不带 AAD 解密应失败
    assert!(decrypt(&ct, TEST_SECRET).is_err());
    // 带相同 AAD 解密应成功
    let pt = flux_agent::util::aes::decrypt_with_aad(&ct, b"header", TEST_SECRET).unwrap();
    assert_eq!(pt, b"msg");
}

#[test]
fn empty_plaintext_works() {
    let ct = encrypt(b"", TEST_SECRET).unwrap();
    let pt = decrypt(&ct, TEST_SECRET).unwrap();
    assert!(pt.is_empty());
}

#[test]
fn large_payload_works() {
    // 1 MB 随机字节
    let data: Vec<u8> = (0..1_000_000).map(|i| ((i * 17 + 3) % 251) as u8).collect();
    let ct = encrypt(&data, TEST_SECRET).unwrap();
    let pt = decrypt(&ct, TEST_SECRET).unwrap();
    assert_eq!(pt.len(), data.len());
    assert_eq!(pt, data);
}