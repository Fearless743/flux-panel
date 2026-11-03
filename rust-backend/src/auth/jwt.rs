use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

const EXPIRE_SECONDS: i64 = 90 * 24 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iat: i64,
    pub exp: i64,
    pub user: String,
    pub name: String,
    pub role_id: i32,
}

#[derive(Serialize)]
struct Header {
    alg: String,
    typ: String,
}

#[derive(Clone)]
pub struct Jwt {
    secret: String,
}

impl Jwt {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }

    pub fn generate(&self, user_id: i64, username: &str, role_id: i32) -> Result<String, String> {
        let now = unix_now();
        let header = Header {
            alg: "HmacSHA256".to_string(),
            typ: "JWT".to_string(),
        };
        let payload = Claims {
            sub: user_id.to_string(),
            iat: now,
            exp: now + EXPIRE_SECONDS,
            user: username.to_string(),
            name: username.to_string(),
            role_id,
        };
        let header_json = serde_json::to_vec(&header).map_err(|e| e.to_string())?;
        let payload_json = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        let enc_header = URL_SAFE_NO_PAD.encode(header_json);
        let enc_payload = URL_SAFE_NO_PAD.encode(payload_json);
        let sig = self.sign(&enc_header, &enc_payload)?;
        Ok(format!("{}.{}.{}", enc_header, enc_payload, sig))
    }

    pub fn validate(&self, token: &str) -> bool {
        self.parse(token).is_ok()
    }

    pub fn parse(&self, token: &str) -> Result<Claims, String> {
        if token.is_empty() {
            return Err("empty token".to_string());
        }
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err("invalid token format".to_string());
        }
        let expected = self.sign(parts[0], parts[1])?;
        if expected != parts[2] {
            return Err("invalid signature".to_string());
        }
        let raw = URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|e| e.to_string())?;
        let claims: Claims = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        if claims.exp <= unix_now() {
            return Err("token expired".to_string());
        }
        Ok(claims)
    }

    pub fn user_id(&self, token: &str) -> Result<i64, String> {
        let claims = self.parse(token)?;
        claims
            .sub
            .parse::<i64>()
            .map_err(|e| e.to_string())
    }

    pub fn role_id(&self, token: &str) -> Result<i32, String> {
        Ok(self.parse(token)?.role_id)
    }

    fn sign(&self, enc_header: &str, enc_payload: &str) -> Result<String, String> {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.secret.as_bytes())
            .map_err(|e| e.to_string())?;
        mac.update(format!("{}.{}", enc_header, enc_payload).as_bytes());
        Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_roundtrip_matches_go_claims() {
        let j = Jwt::new("secret");
        let token = j.generate(7, "alice", 1).unwrap();
        let claims = j.parse(&token).unwrap();
        assert_eq!(claims.sub, "7");
        assert_eq!(claims.user, "alice");
        assert_eq!(claims.name, "alice");
        assert_eq!(claims.role_id, 1);
        assert!(claims.exp > claims.iat);
        // header 与 Go 版一致：{"alg":"HmacSHA256","typ":"JWT"}
        assert_eq!(
            token.split('.').next().unwrap(),
            "eyJhbGciOiJIbWFjU0hBMjU2IiwidHlwIjoiSldUIn0"
        );
    }

    #[test]
    fn jwt_rejects_tampered_payload() {
        let j = Jwt::new("secret");
        let token = j.generate(1, "admin_user", 0).unwrap();
        let mut parts: Vec<String> = token.split('.').map(str::to_string).collect();
        parts[1] = URL_SAFE_NO_PAD.encode(br#"{"sub":"999"}"#);
        assert!(j.parse(&parts.join(".")).is_err());
    }
}
