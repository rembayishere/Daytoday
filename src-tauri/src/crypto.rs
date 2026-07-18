use aes_gcm::{aead::{Aead, KeyInit, OsRng, AeadCore}, Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaNonce};

const SALT_LEN: usize = 16;
const PREFIX: &str = "DT1";

// 支持的加密算法标识
pub const ALG_AES_GCM: &str = "aes256-gcm";
pub const ALG_CHACHA: &str = "chacha20-poly1305";
pub const ALG_AES_GCM_PBKDF2: &str = "aes256-gcm-pbkdf2";

pub fn default_algorithm() -> &'static str {
    ALG_AES_GCM
}

pub fn is_valid_algorithm(alg: &str) -> bool {
    matches!(alg, ALG_AES_GCM | ALG_CHACHA | ALG_AES_GCM_PBKDF2)
}

// 头部格式： "DT1:<algorithm>:" + base64(nonce|salt|ciphertext)
pub fn encrypt(data: &[u8], password: &str, algorithm: &str) -> Result<String, String> {
    let alg = if is_valid_algorithm(algorithm) { algorithm } else { ALG_AES_GCM };
    let salt = {
        let n1 = Aes256Gcm::generate_nonce(&mut OsRng);
        let n2 = Aes256Gcm::generate_nonce(&mut OsRng);
        let mut s = [0u8; SALT_LEN];
        s[..12].copy_from_slice(&n1);
        s[12..].copy_from_slice(&n2[..4]);
        s
    };
    let key = derive_key(password, &salt, alg);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = match alg {
        ALG_AES_GCM | ALG_AES_GCM_PBKDF2 => {
            let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
            cipher.encrypt(&nonce, data).map_err(|e| e.to_string())?
        }
        ALG_CHACHA => {
            let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| e.to_string())?;
            let cnonce = ChaNonce::from_slice(&nonce);
            cipher.encrypt(cnonce, data).map_err(|e| e.to_string())?
        }
        _ => return Err("不支持的加密算法".into()),
    };
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&salt);
    combined.extend_from_slice(&ciphertext);
    let body = STANDARD.encode(&combined);
    Ok(format!("{}:{}:{}", PREFIX, alg, body))
}

pub fn decrypt(encoded: &str, password: &str, algorithm: &str) -> Result<Vec<u8>, String> {
    // 新格式：以 "DT1:<algorithm>:" 开头
    if let Some(stripped) = encoded.strip_prefix(&format!("{}:", PREFIX)) {
        let (alg, body) = match stripped.split_once(':') {
            Some((a, b)) => (a, b),
            None => return Err("加密数据头部格式错误".into()),
        };
        let combined = STANDARD.decode(body).map_err(|e| e.to_string())?;
        return decrypt_inner(&combined, password, alg);
    }
    // 兼容旧数据（无前缀）：按 aes256-gcm + Argon2 解密
    let combined = STANDARD.decode(encoded).map_err(|e| e.to_string())?;
    let alg = if is_valid_algorithm(algorithm) { algorithm } else { ALG_AES_GCM };
    decrypt_inner(&combined, password, alg)
}

fn decrypt_inner(combined: &[u8], password: &str, alg: &str) -> Result<Vec<u8>, String> {
    if combined.len() < 12 + SALT_LEN {
        return decrypt_legacy(combined, password);
    }
    let (nonce_bytes, rest) = combined.split_at(12);
    let (salt_bytes, ciphertext) = rest.split_at(SALT_LEN);
    let key = derive_key(password, salt_bytes, alg);
    match alg {
        ALG_AES_GCM | ALG_AES_GCM_PBKDF2 => {
            let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
            let nonce = Nonce::from_slice(nonce_bytes);
            cipher.decrypt(nonce, ciphertext).map_err(|_| "解密失败（密码错误或数据损坏）".into())
        }
        ALG_CHACHA => {
            let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| e.to_string())?;
            let cnonce = ChaNonce::from_slice(nonce_bytes);
            cipher.decrypt(cnonce, ciphertext).map_err(|_| "解密失败（密码错误或数据损坏）".into())
        }
        _ => Err("不支持的加密算法".into()),
    }
}

fn derive_key(password: &str, salt: &[u8], alg: &str) -> [u8; 32] {
    let mut key = [0u8; 32];
    if alg == ALG_AES_GCM_PBKDF2 {
        use pbkdf2::pbkdf2;
        use hmac::Hmac;
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;
        pbkdf2::<HmacSha256>(password.as_bytes(), salt, 100_000, &mut key).ok();
    } else {
        argon2::Argon2::default()
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .expect("Argon2 key derivation failed");
    }
    key
}

fn decrypt_legacy(combined: &[u8], password: &str) -> Result<Vec<u8>, String> {
    if combined.len() < 12 {
        return Err("数据太短".into());
    }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "解密失败（密码错误或数据损坏）".into())
}
