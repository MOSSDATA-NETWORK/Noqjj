/// TOTP 身份验证器支持
use totp_rs::{Algorithm, Secret, TOTP};

const ISSUER: &str = "ChickenDetect";

/// 生成 TOTP 密钥（base32）
pub fn generate_secret() -> String {
    let secret = Secret::generate_secret();
    secret.to_string()
}

/// 生成 otpauth URI（用于二维码）
pub fn get_otpauth_uri(username: &str, secret: &str) -> String {
    format!("otpauth://totp/{}:{}?secret={}&issuer={}&digits=6&period=30&algorithm=SHA1",
            ISSUER, username, secret, ISSUER)
}

/// 验证 TOTP code
pub fn verify_code(secret: &str, code: &str) -> bool {
    let secret_bytes = match Secret::Encoded(secret.to_string()).to_bytes() {
        Ok(b) => b,
        Err(_) => return false,
    };
    let totp = match TOTP::new(Algorithm::SHA1, 6, 1, 30, secret_bytes, Some(ISSUER.to_string()), "user".to_string()) {
        Ok(t) => t,
        Err(_) => return false,
    };
    totp.check_current(code).unwrap_or(false)
}
