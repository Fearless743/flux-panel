use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use crate::error::{AppError, AppResult};

const GCM_IV_LENGTH: usize = 12;
const GCM_TAG_LENGTH: usize = 16;

pub struct AesCrypto {
    cipher: Aes256Gcm,
}

impl AesCrypto {
    pub fn new(secret: &str) -> AppResult<Self> {
        if secret.is_empty() {
            return Err(AppError::new("密钥不能为空"));
        }
        let key = Sha256::digest(secret.as_bytes());
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| AppError::new(e.to_string()))?;
        Ok(Self { cipher })
    }

    pub fn encrypt(&self, data: &[u8]) -> AppResult<String> {
        if data.is_empty() {
            return Err(AppError::new("待加密数据不能为空"));
        }
        let mut nonce = [0u8; GCM_IV_LENGTH];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), data)
            .map_err(|e| AppError::new(e.to_string()))?;
        let mut out = Vec::with_capacity(GCM_IV_LENGTH + ciphertext.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(STANDARD.encode(out))
    }

    pub fn encrypt_string(&self, s: &str) -> AppResult<String> {
        self.encrypt(s.as_bytes())
    }

    pub fn decrypt(&self, encrypted: &str) -> AppResult<Vec<u8>> {
        if encrypted.is_empty() {
            return Err(AppError::new("加密数据不能为空"));
        }
        let raw = STANDARD
            .decode(encrypted)
            .map_err(|e| AppError::new(e.to_string()))?;
        if raw.len() < GCM_IV_LENGTH + GCM_TAG_LENGTH {
            return Err(AppError::new("加密数据长度不足"));
        }
        let (nonce, ciphertext) = raw.split_at(GCM_IV_LENGTH);
        self.cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|e| AppError::new(e.to_string()))
    }

    pub fn decrypt_string(&self, encrypted: &str) -> AppResult<String> {
        let b = self.decrypt(encrypted)?;
        String::from_utf8(b).map_err(|e| AppError::new(e.to_string()))
    }
}

static CACHE: LazyLock<Mutex<HashMap<String, Arc<AesCrypto>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static CACHE_ORDER: LazyLock<Mutex<std::collections::VecDeque<String>>> =
    LazyLock::new(|| Mutex::new(std::collections::VecDeque::new()));
/// 单 secret 缓存上限，防止 /flow/* 被随机 secret 打爆内存
const CACHE_MAX: usize = 512;

pub fn get_or_create(secret: &str) -> Option<Arc<AesCrypto>> {
    if secret.is_empty() || secret.len() > 256 {
        return None;
    }
    {
        let cache = CACHE.lock().unwrap();
        if let Some(c) = cache.get(secret) {
            return Some(c.clone());
        }
    }
    let c = Arc::new(AesCrypto::new(secret).ok()?);
    let mut cache = CACHE.lock().unwrap();
    let mut order = CACHE_ORDER.lock().unwrap();
    if cache.len() >= CACHE_MAX {
        if let Some(old) = order.pop_front() {
            cache.remove(&old);
        }
    }
    order.push_back(secret.to_string());
    Some(cache.entry(secret.to_string()).or_insert(c).clone())
}

pub fn clear(secret: &str) {
    CACHE.lock().unwrap().remove(secret);
    CACHE_ORDER.lock().unwrap().retain(|s| s != secret);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_gcm_roundtrip() {
        let c = AesCrypto::new("secret").unwrap();
        let enc = c.encrypt(b"hello rust").unwrap();
        assert_eq!(c.decrypt(&enc).unwrap(), b"hello rust");
    }

    #[test]
    fn aes_gcm_rejects_short_ciphertext() {
        let c = AesCrypto::new("secret").unwrap();
        assert!(c.decrypt("AAAA").is_err());
    }
}
