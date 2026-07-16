use aes_gcm::{aead::{Aead, KeyInit, OsRng, AeadCore}, Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};

const SALT_LEN: usize = 16;

pub fn encrypt(data: &[u8], password: &str) -> Result<String, String> {
    let salt = {
        let n1 = Aes256Gcm::generate_nonce(&mut OsRng);
        let n2 = Aes256Gcm::generate_nonce(&mut OsRng);
        let mut s = [0u8; SALT_LEN];
        s[..12].copy_from_slice(&n1);
        s[12..].copy_from_slice(&n2[..4]);
        s
    };
    let key = derive_key(password, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher.encrypt(&nonce, data).map_err(|e| e.to_string())?;
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&salt);
    combined.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(&combined))
}

pub fn decrypt(encoded: &str, password: &str) -> Result<Vec<u8>, String> {
    let combined = STANDARD.decode(encoded).map_err(|e| e.to_string())?;
    if combined.len() >= 12 + SALT_LEN {
        let (nonce_bytes, rest) = combined.split_at(12);
        let (salt_bytes, ciphertext) = rest.split_at(SALT_LEN);
        let key = derive_key(password, salt_bytes);
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
        let nonce = Nonce::from_slice(nonce_bytes);
        if let Ok(plaintext) = cipher.decrypt(nonce, ciphertext) {
            return Ok(plaintext);
        }
    }
    decrypt_legacy(&combined, password)
}

fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .expect("Argon2 key derivation failed");
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
