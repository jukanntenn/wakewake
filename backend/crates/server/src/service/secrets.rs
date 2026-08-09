//! 安全随机（pairing code / reset token 辅助）。
//!
//! pairing code：16 hex 字符（终身凭证，CHAR(16)）。
//! reset token：Django 式无状态 HMAC（authentication.md §四）。
//!
//! HMAC-SHA256 用 sha2 原语手写（HMAC = H((K' ⊕ opad) || H((K' ⊕ ipad) || msg)），
//! 避免 hmac crate 在 sha2 0.11 / digest 0.11 下的 `KeyInit` trait 导入复杂度，
//! 对齐"零依赖精简"哲学（getrandom 取代 rand 同理）。

use sha2::{Digest, Sha256};
use time::OffsetDateTime;

const BLOCK_SIZE: usize = 64; // SHA-256 block size

/// HMAC-SHA256（RFC 2104），返回 32 字节摘要。
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    // K'：key > block_size 则先 hash；不足补零到 block_size
    let mut k0 = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let mut h = Sha256::new();
        h.update(key);
        let digest = h.finalize();
        k0[..digest.len()].copy_from_slice(&digest);
    } else {
        k0[..key.len()].copy_from_slice(key);
    }

    // ipad / opad
    let mut ipad = [0x36u8; BLOCK_SIZE];
    let mut opad = [0x5cu8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        ipad[i] ^= k0[i];
        opad[i] ^= k0[i];
    }

    // inner = H(ipad || message)
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();

    // outer = H(opad || inner)
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    let result = outer.finalize();

    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// 生成 16 hex pairing code（getrandom 安全随机）。
#[must_use]
pub fn generate_pairing_code() -> String {
    use getrandom::fill;
    let mut buf = [0u8; 8];
    let _ = fill(&mut buf);
    hex::encode(buf) // 16 hex 字符
}

/// 无状态密码重置 token：`base64url(user_id).base36(timestamp).truncated_hmac`
/// （Django PasswordResetTokenGenerator，authentication.md §四）。
///
/// HMAC 密钥派生材料含 `PASSWORD_RESET_SECRET || user.password_hash || user.last_login`
/// → 改密/登录自动失效（一次性免费）。token 不含 `password_hash` 可见部分。
#[must_use]
pub fn make_reset_token(
    secret: &str,
    user_id: i64,
    password_hash: &str,
    last_login_ts: Option<i64>,
) -> String {
    let ts = OffsetDateTime::now_utc().unix_timestamp();
    make_stateful_token(
        secret,
        "reset",
        user_id,
        password_hash,
        last_login_ts,
        ts as u64,
    )
}

/// 校验 reset token（HMAC + timestamp 未过期）。返回 `user_id` 或错误。
pub fn verify_reset_token(
    secret: &str,
    token: &str,
    password_hash: &str,
    last_login_ts: Option<i64>,
    max_age_secs: i64,
) -> Result<i64, ResetTokenError> {
    verify_stateful_token(
        secret,
        "reset",
        token,
        password_hash,
        last_login_ts,
        max_age_secs,
    )
}

/// 无状态邮箱验证 token：复用 reset 的 HMAC 机制，但 purpose 标签不同（"verify"），
/// 防止 reset token 被复用为 verify（或反之）。HMAC key 含 password_hash → 改密后失效。
#[must_use]
pub fn make_verify_token(secret: &str, user_id: i64, password_hash: &str) -> String {
    let ts = OffsetDateTime::now_utc().unix_timestamp();
    make_stateful_token(secret, "verify", user_id, password_hash, None, ts as u64)
}

/// 校验邮箱验证 token。
pub fn verify_verify_token(
    secret: &str,
    token: &str,
    password_hash: &str,
    max_age_secs: i64,
) -> Result<i64, ResetTokenError> {
    verify_stateful_token(secret, "verify", token, password_hash, None, max_age_secs)
}

/// 通用无状态 token 构造：`base64url(user_id).base36(timestamp).truncated_hmac`。
/// HMAC key = `secret || purpose || password_hash || last_login`，purpose 隔离不同用途。
fn make_stateful_token(
    secret: &str,
    purpose: &str,
    user_id: i64,
    password_hash: &str,
    last_login_ts: Option<i64>,
    ts: u64,
) -> String {
    let ts_b36 = to_base36(ts);
    let last_login = last_login_ts.map(|t| t.to_string()).unwrap_or_default();
    let key = format!("{secret}{purpose}{password_hash}{last_login}");
    let msg = format!("{user_id}.{ts_b36}");
    let mac = hmac_sha256(key.as_bytes(), msg.as_bytes());
    let truncated = hex::encode(&mac[..10]);
    format!(
        "{}.{}.{}",
        base64url(&user_id.to_string()),
        ts_b36,
        truncated
    )
}

/// 通用无状态 token 校验。
fn verify_stateful_token(
    secret: &str,
    purpose: &str,
    token: &str,
    password_hash: &str,
    last_login_ts: Option<i64>,
    max_age_secs: i64,
) -> Result<i64, ResetTokenError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(ResetTokenError::Malformed);
    }
    let user_id_b64 = parts[0];
    let ts_b36 = parts[1];
    let sig = parts[2];

    let user_id_str = base64url_decode(user_id_b64).ok_or(ResetTokenError::Malformed)?;
    let user_id: i64 = user_id_str
        .parse()
        .map_err(|_| ResetTokenError::Malformed)?;

    let ts = u64::from_str_radix(ts_b36, 36).map_err(|_| ResetTokenError::Malformed)?;
    let now = OffsetDateTime::now_utc().unix_timestamp() as u64;
    if now.saturating_sub(ts) > max_age_secs as u64 {
        return Err(ResetTokenError::Expired);
    }

    let last_login = last_login_ts.map(|t| t.to_string()).unwrap_or_default();
    let key = format!("{secret}{purpose}{password_hash}{last_login}");
    let msg = format!("{user_id}.{ts_b36}");
    let mac = hmac_sha256(key.as_bytes(), msg.as_bytes());
    let expected = hex::encode(&mac[..10]);

    if hmac_ct_eq(&expected, sig) {
        Ok(user_id)
    } else {
        Err(ResetTokenError::Invalid)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResetTokenError {
    #[error("malformed token")]
    Malformed,
    #[error("token expired")]
    Expired,
    #[error("invalid token")]
    Invalid,
}

// ---- helpers ----

fn to_base36(mut n: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_else(|_| "0".into())
}

/// base64url 编码（无 padding）。
fn base64url(input: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(input.as_bytes())
}

fn base64url_decode(input: &str) -> Option<String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(input)
        .ok()?;
    String::from_utf8(bytes).ok()
}

/// 常量时间字符串比较（防时序攻击）。
fn hmac_ct_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_code_is_16_hex() {
        let code = generate_pairing_code();
        assert_eq!(code.len(), 16);
        assert!(code.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hmac_matches_known_vector() {
        // RFC 4231 test case 1: key=0x0b*20, data="Hi There"
        let key = [0x0bu8; 20];
        let msg = b"Hi There";
        let mac = hmac_sha256(&key, msg);
        let hex_mac = hex::encode(mac);
        assert_eq!(
            hex_mac,
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn reset_token_round_trip_and_one_time() {
        let secret = "topsecret";
        let pw = "$2b$10$hashhashhashhashhashhashhashhashhashhashhashhash";
        let token = make_reset_token(secret, 42, pw, None);
        let uid = verify_reset_token(secret, &token, pw, None, 3600).unwrap();
        assert_eq!(uid, 42);

        // 改密后失效（一次性）
        let err =
            verify_reset_token(secret, &token, "$2b$10$CHANGEDCHANGED", None, 3600).unwrap_err();
        assert!(matches!(err, ResetTokenError::Invalid));

        // 登录后（last_login 变）失效
        let err = verify_reset_token(secret, &token, pw, Some(999), 3600).unwrap_err();
        assert!(matches!(err, ResetTokenError::Invalid));
    }

    #[test]
    fn reset_token_expires() {
        let secret = "s";
        let pw = "h";
        // ts=0（很久以前）
        let token = make_stateful_token(secret, "reset", 42, pw, None, 0);
        let err = verify_reset_token(secret, &token, pw, None, 3600).unwrap_err();
        assert!(matches!(err, ResetTokenError::Expired));
    }

    #[test]
    fn verify_token_round_trip_and_purpose_isolation() {
        let secret = "topsecret";
        let pw = "$2b$10$hashhashhashhashhashhashhashhash";
        let token = make_verify_token(secret, 42, pw);
        let uid = verify_verify_token(secret, &token, pw, 3600).unwrap();
        assert_eq!(uid, 42);

        // 改密后失效（一次性）
        let err = verify_verify_token(secret, &token, "$2b$10$CHANGED", 3600).unwrap_err();
        assert!(matches!(err, ResetTokenError::Invalid));

        // 过期失效
        let stale = make_stateful_token(secret, "verify", 42, pw, None, 0);
        let err = verify_verify_token(secret, &stale, pw, 3600).unwrap_err();
        assert!(matches!(err, ResetTokenError::Expired));
    }

    #[test]
    fn reset_and_verify_tokens_not_interchangeable() {
        let secret = "topsecret";
        let pw = "$2b$10$hashhashhashhashhash";
        // reset token 不能用作 verify（purpose 标签不同）
        let reset_tok = make_reset_token(secret, 42, pw, None);
        let err = verify_verify_token(secret, &reset_tok, pw, 3600).unwrap_err();
        assert!(matches!(err, ResetTokenError::Invalid));

        // verify token 不能用作 reset
        let verify_tok = make_verify_token(secret, 42, pw);
        let err = verify_reset_token(secret, &verify_tok, pw, None, 3600).unwrap_err();
        assert!(matches!(err, ResetTokenError::Invalid));
    }
}
