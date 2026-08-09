//! RSA 密钥对持久化（api-design.md §2.12 / e2e.md §6.7）。
//!
//! Agent 首启生成 2048-bit RSA 密钥对，持久化到 `$WAKEWAKE_HOME/key.pem`（PKCS8 PEM）。
//! 后续启动从文件加载（重启后密钥不变，server 视为同一 agent）。
//! 首次 SSE 连接通过 X-Public-Key 头上报公钥（SPKI PEM）。
//!
//! 替换 §6.7 指出的 `dummy_key()` 瑕疵（每次临时生成、重启变化）。

use std::path::{Path, PathBuf};

use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::{RsaPrivateKey, RsaPublicKey};

/// 密钥文件名（相对 `home_dir`）。
const KEY_FILE: &str = "key.pem";

/// 加载或生成 RSA 密钥对（api-design.md §2.12）。
///
/// - 文件存在 → 加载。
/// - 文件不存在 → 生成 2048-bit + 写文件（0600 权限）。
///
/// 返回 (私钥, 公钥 PEM SPKI)。
pub fn load_or_generate(home_dir: &Path) -> Result<(RsaPrivateKey, String), KeyError> {
    let key_path = key_path(home_dir);
    if key_path.exists() {
        let pem = std::fs::read_to_string(&key_path)?;
        let priv_key = RsaPrivateKey::from_pkcs8_pem(&pem)?;
        let pub_pem = public_key_pem(&priv_key)?;
        Ok((priv_key, pub_pem))
    } else {
        let mut rng = rand::rng();
        let priv_key = RsaPrivateKey::new(&mut rng, 2048)?;
        let pem = priv_key.to_pkcs8_pem(LineEnding::LF)?;
        // 确保父目录存在
        if let Some(parent) = key_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&key_path, pem.as_bytes())?;
        set_owner_only(&key_path)?;
        let pub_pem = public_key_pem(&priv_key)?;
        tracing::info!(path = %key_path.display(), "generated new RSA keypair");
        Ok((priv_key, pub_pem))
    }
}

/// 密钥文件路径。
#[must_use]
pub fn key_path(home_dir: &Path) -> PathBuf {
    home_dir.join(KEY_FILE)
}

/// 导出公钥为 SPKI PEM（X-Public-Key 头用，与前端 crypto.ts 加密的 spki 格式一致）。
pub fn public_key_pem(priv_key: &RsaPrivateKey) -> Result<String, KeyError> {
    let pub_key = RsaPublicKey::from(priv_key);
    Ok(pub_key
        .to_public_key_pem(LineEnding::LF)?
        .trim()
        .to_string())
}

/// Unix 下设 0600（仅 owner 可读），保护私钥。
#[cfg(unix)]
fn set_owner_only(path: &Path) -> Result<(), KeyError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> Result<(), KeyError> {
    Ok(())
}

/// 密钥存储错误。
#[derive(thiserror::Error, Debug)]
pub enum KeyError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// `to_public_key_pem` 返回 `spki::Error`
    #[error("rsa spki error: {0}")]
    Spki(#[from] rsa::pkcs8::spki::Error),
    /// `from_pkcs8_pem` / `to_pkcs8_pem` 返回 `pkcs8::Error（非` `spki::Error`）
    #[error("rsa pkcs8 error: {0}")]
    Pkcs8(#[from] rsa::pkcs8::Error),
    #[error("rsa error: {0}")]
    Rsa(#[from] rsa::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_or_generate_persists_across_calls() {
        let tmp = tempfile();
        let dir = &tmp;
        // 首次：生成
        let (priv1, pub1) = load_or_generate(dir).unwrap();
        assert!(key_path(dir).exists());
        // 二次：加载（同一密钥）
        let (priv2, pub2) = load_or_generate(dir).unwrap();
        assert_eq!(pub1, pub2, "public key must be stable across restarts");
        // 私钥等价（通过公钥比较，RsaPrivateKey 无 Eq）
        assert_eq!(
            RsaPublicKey::from(&priv1),
            RsaPublicKey::from(&priv2),
            "private key must be stable across restarts"
        );
    }

    #[test]
    fn public_key_pem_is_spki_format() {
        let (priv_key, pub_pem) = load_or_generate(&tempfile()).unwrap();
        assert!(pub_pem.contains("BEGIN PUBLIC KEY"));
        assert!(pub_pem.contains("END PUBLIC KEY"));
        // 公钥从私钥派生一致
        let derived = public_key_pem(&priv_key).unwrap();
        assert_eq!(pub_pem, derived);
    }

    /// 临时目录（自动清理）。
    fn tempfile() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wakewake-keystore-test-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
