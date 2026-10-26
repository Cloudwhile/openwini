use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

const NONCE_LEN: usize = 12;
const PREFIX: &str = "v1:";

static APP_KEY: OnceLock<Vec<u8>> = OnceLock::new();

/// 启动时注入应用级密钥，供无 state 上下文的位置使用
pub fn init_app_key(key: Vec<u8>) {
    let _ = APP_KEY.set(key);
}

pub fn current_key() -> Vec<u8> {
    APP_KEY.get().cloned().expect("app key not initialized")
}

/// 应用级密钥加密（AES-256-GCM），输出 "v1:<nonce_hex>:<ct_hex>"
pub fn encrypt(key: &[u8], plain: &str) -> Result<String> {
    let cipher = Aes256Gcm::new_from_slice(key).context("invalid key")?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plain.as_bytes(),
                aad: b"wini-plugins",
            },
        )
        .map_err(|_| anyhow::anyhow!("encrypt failed"))?;
    Ok(format!(
        "{PREFIX}{}:{}",
        hex::encode(nonce_bytes),
        hex::encode(ct)
    ))
}

pub fn decrypt(key: &[u8], stored: &str) -> Result<String> {
    let rest = stored
        .strip_prefix(PREFIX)
        .context("bad ciphertext prefix")?;
    let (nonce_hex, ct_hex) = rest.split_once(':').context("bad ciphertext format")?;
    let nonce_bytes = hex::decode(nonce_hex)?;
    let ct = hex::decode(ct_hex)?;
    let cipher = Aes256Gcm::new_from_slice(key)?;
    let plain = cipher
        .decrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: &ct,
                aad: b"wini-plugins",
            },
        )
        .map_err(|_| anyhow::anyhow!("decrypt failed"))?;
    Ok(String::from_utf8(plain)?)
}

/// 系统安全随机源生成 n 字节并十六进制编码
pub fn random_hex(n: usize) -> String {
    let mut buf = vec![0u8; n];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    hex::encode(buf)
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

pub fn token_hash(token: &str) -> String {
    sha256_hex(token.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_roundtrip() {
        let key = [7u8; 32];
        let c = encrypt(&key, "secret-token").unwrap();
        assert!(c.starts_with("v1:"));
        assert_eq!(decrypt(&key, &c).unwrap(), "secret-token");
        let other = [8u8; 32];
        assert!(decrypt(&other, &c).is_err());
    }

    #[test]
    fn random_hex_unique() {
        assert_ne!(random_hex(32), random_hex(32));
        assert_eq!(random_hex(32).len(), 64);
    }
}
