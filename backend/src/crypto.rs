/// 密钥管理：自动生成并存储在 SQLite settings 表
/// PVE 密码等敏感数据用 AES-256-GCM 加密
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use sqlx::SqlitePool;

/// 获取或生成加密密钥（存储在 settings 表）
pub async fn get_or_create_master_key(pool: &SqlitePool) -> anyhow::Result<Vec<u8>> {
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'master_key'"
    )
    .fetch_optional(pool)
    .await?;

    match existing {
        Some(hex) => hex_to_bytes(&hex).map_err(|_| anyhow::anyhow!("密钥格式错误")),
        None => {
            let mut key = [0u8; 32];
            OsRng.fill_bytes(&mut key);
            let hex = bytes_to_hex(&key);
            sqlx::query("INSERT INTO settings (key, value) VALUES ('master_key', ?)")
                .bind(&hex)
                .execute(pool)
                .await?;
            tracing::info!("已生成新的加密密钥");
            Ok(key.to_vec())
        }
    }
}

/// 加密
pub fn encrypt(plaintext: &str, master_key: &[u8]) -> String {
    let cipher = Aes256Gcm::new_from_slice(master_key).expect("密钥长度错误");
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes()).expect("加密失败");
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    B64.encode(&combined)
}

/// 解密
pub fn decrypt(encoded: &str, master_key: &[u8]) -> anyhow::Result<String> {
    let combined = B64.decode(encoded)?;
    if combined.len() < 12 {
        return Err(anyhow::anyhow!("密文格式错误"));
    }
    let cipher = Aes256Gcm::new_from_slice(master_key)
        .map_err(|e| anyhow::anyhow!("密钥错误: {}", e))?;
    let nonce = Nonce::from_slice(&combined[..12]);
    let plaintext = cipher.decrypt(nonce, &combined[12..])
        .map_err(|_| anyhow::anyhow!("解密失败"))?;
    Ok(String::from_utf8(plaintext)?)
}

/// 生成随机 token
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    B64.encode(&bytes)
}

/// Argon2 密码哈希
pub fn hash_password(password: &str) -> String {
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    let salt = SaltString::generate(&mut OsRng);
    argon2::Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("哈希失败")
        .to_string()
}

/// 验证密码
pub fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    argon2::Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_to_bytes(hex: &str) -> anyhow::Result<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return Err(anyhow::anyhow!("hex 长度必须为偶数"));
    }
    Ok((0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect::<Result<Vec<_>, _>>()?)
}
