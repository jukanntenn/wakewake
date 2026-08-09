//! JWT 签发/校验（authentication.md §一）。
//!
//! 双 token（HS256，双独立密钥）：access 15min + refresh 30d。
//! Validation 锁 HS256 + `validate_exp` + `required_spec_claims` exp（防 alg:none）。
//! claim type 区分 access/refresh（防 refresh 当 access 用）。

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::config::JwtSettings;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: i64, // user_id
    pub email: String,
    pub is_superuser: bool,
    pub token_type: String, // "access"
    pub exp: u64,
    pub iat: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: i64,
    pub token_type: String, // "refresh"
    /// JWT ID（RFC 7519 §4.1.7）：保证每次签发的 refresh token 唯一。
    /// 修复同秒为同一用户签发两个 token 完全一致 → hash 冲突违反 `idx_refresh_tokens_hash` 唯一约束。
    /// authentication.md §二 refresh rotation 要求每次签发的 token 必须唯一可吊销。
    pub jti: String,
    pub exp: u64,
    pub iat: u64,
}

/// 签发 access token。
pub fn issue_access(
    jwt: &JwtSettings,
    user_id: i64,
    email: &str,
    is_superuser: bool,
) -> anyhow::Result<String> {
    let now = OffsetDateTime::now_utc().unix_timestamp() as u64;
    let ttl = jwt.access_ttl()?;
    let claims = AccessClaims {
        sub: user_id,
        email: email.to_string(),
        is_superuser,
        token_type: "access".to_string(),
        exp: now + ttl.as_secs(),
        iat: now,
    };
    Ok(encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(jwt.signing_key.as_bytes()),
    )?)
}

/// 签发 refresh token（明文，存 SHA-256 hash）。
pub fn issue_refresh(jwt: &JwtSettings, user_id: i64) -> anyhow::Result<String> {
    let now = OffsetDateTime::now_utc().unix_timestamp() as u64;
    let ttl = jwt.refresh_ttl()?;
    let claims = RefreshClaims {
        sub: user_id,
        token_type: "refresh".to_string(),
        // jti: 8 字节 getrandom → 16 hex，保证 token 唯一（防 hash 冲突）
        jti: crate::service::secrets::generate_pairing_code(),
        exp: now + ttl.as_secs(),
        iat: now,
    };
    Ok(encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(jwt.refresh_signing_key.as_bytes()),
    )?)
}

/// 校验 access token（锁 HS256 + `validate_exp`）。
pub fn verify_access(jwt: &JwtSettings, token: &str) -> anyhow::Result<AccessClaims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.required_spec_claims.insert("exp".to_string());
    let data = decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(jwt.signing_key.as_bytes()),
        &validation,
    )?;
    if data.claims.token_type != "access" {
        anyhow::bail!("not an access token");
    }
    Ok(data.claims)
}

/// 校验 refresh token（独立密钥）。
pub fn verify_refresh(jwt: &JwtSettings, token: &str) -> anyhow::Result<RefreshClaims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.required_spec_claims.insert("exp".to_string());
    let data = decode::<RefreshClaims>(
        token,
        &DecodingKey::from_secret(jwt.refresh_signing_key.as_bytes()),
        &validation,
    )?;
    if data.claims.token_type != "refresh" {
        anyhow::bail!("not a refresh token");
    }
    Ok(data.claims)
}

/// 算 SHA-256(token) 的 `hex（refresh_tokens.token_hash，明文永不入库`）。
#[must_use]
pub fn hash_token(token: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}
