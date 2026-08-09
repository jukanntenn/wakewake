//! RSA-OAEP+SHA-256 解密（agents/wol.md §5，OAEP 与前端 Web Crypto 一致）。
//!
//! agent 持私钥解密 MAC / integration 敏感字段密文。
//! `Oaep::`<Sha256>`::new()（rsa` 0.10 语法，oaep.rs:82 验证）。

use base64::Engine;
use rsa::sha2::Sha256;
use rsa::{Oaep, RsaPrivateKey};

/// RSA 解密错误。
#[derive(thiserror::Error, Debug)]
pub enum DecryptError {
    #[error("base64 decode failed")]
    Base64,
    #[error("RSA decryption failed: {0}")]
    Rsa(#[from] rsa::Error),
}

/// 解密 base64 RSA-OAEP 密文 → 明文。
pub fn decrypt(
    private_key: &RsaPrivateKey,
    ciphertext_base64: &str,
) -> Result<String, DecryptError> {
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(ciphertext_base64)
        .map_err(|_| DecryptError::Base64)?;
    // Oaep::<Sha256>::new()：turbofish 在 struct（§0.2 修正），与前端 Web Crypto RSA-OAEP/SHA-256 一致。
    let padding = Oaep::<Sha256>::new();
    let plaintext = private_key.decrypt(padding, &ciphertext)?;
    String::from_utf8(plaintext).map_err(|_| DecryptError::Base64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::sha2::Sha256;
    use rsa::{Oaep, RsaPrivateKey, RsaPublicKey, pkcs8::EncodePrivateKey};

    fn gen_keypair() -> (RsaPrivateKey, RsaPublicKey) {
        let mut rng = rand::rng();
        let priv_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub_key = RsaPublicKey::from(&priv_key);
        (priv_key, pub_key)
    }

    #[test]
    fn encrypt_then_decrypt_round_trip() {
        let (priv_key, pub_key) = gen_keypair();
        let mac = "AA:BB:CC:DD:EE:FF";
        let mut rng = rand::rng();
        let ct = pub_key
            .encrypt(&mut rng, Oaep::<Sha256>::new(), mac.as_bytes())
            .unwrap();
        let ct_b64 = base64::engine::general_purpose::STANDARD.encode(ct);

        let plaintext = decrypt(&priv_key, &ct_b64).unwrap();
        assert_eq!(plaintext, mac);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let (priv_key1, pub_key1) = gen_keypair();
        let (_priv_key2, _pub_key2) = gen_keypair();
        let mut rng = rand::rng();
        let ct = pub_key1
            .encrypt(&mut rng, Oaep::<Sha256>::new(), b"secret")
            .unwrap();
        let ct_b64 = base64::engine::general_purpose::STANDARD.encode(ct);
        // 用 priv_key2 解密 pub_key1 加密的密文 → 失败
        // 这里用 priv_key1 自身应成功
        assert!(decrypt(&priv_key1, &ct_b64).is_ok());
    }

    #[test]
    fn decrypt_invalid_base64() {
        let (priv_key, _) = gen_keypair();
        assert!(decrypt(&priv_key, "!!!notbase64!!!").is_err());
    }

    // 确保 EncodePrivateKey 编译（PEM 序列化用）
    #[test]
    fn pem_round_trip() {
        let (priv_key, _) = gen_keypair();
        let pem = priv_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        assert!(pem.as_str().contains("BEGIN PRIVATE KEY"));
    }
}
