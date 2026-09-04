//! HTTP client 工厂（sse_client + reporter 共用，e2e.md §6.2 / agent-distribution.md）。
//!
//! TLS 信任三层：
//! 1. 默认——系统信任库（rustls-tls）。
//! 2. `[tls] ca_cert`（配置文件 / `WAKEWAKE_TLS__CA_CERT`）——内网自托管（Caddy `tls
//!    internal`）场景：追加信任用户自己的 root CA（PEM），不关闭校验。
//! 3. `danger-insecure-tls` feature——**编译期**、e2e 专用（`danger_accept_invalid_certs(true)`
//!    信任任何证书）。生产发布二进制绝不携带（CI release 校验）。
//!
//! reqwest 0.12：feature 名 `rustls-tls`（workspace Cargo.toml），方法名
//! `danger_accept_invalid_certs`（非 0.13 的 `tls_danger_accept_invalid_certs`）。

use std::path::Path;

use anyhow::Context as _;

/// 构建 reqwest Client。`ca_cert` 为 `None` 时用系统信任库；
/// 指定但读取/解析失败时返回错误（不静默降级到无自定义 CA）。
pub fn build_http_client(ca_cert: Option<&Path>) -> anyhow::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder();
    if let Some(path) = ca_cert {
        let pem = std::fs::read(path)
            .with_context(|| format!("failed to read TLS CA cert at {}", path.display()))?;
        let cert = reqwest::Certificate::from_pem(&pem)
            .context("failed to parse TLS CA cert (PEM format expected)")?;
        builder = builder.add_root_certificate(cert);
    }
    #[cfg(feature = "danger-insecure-tls")]
    {
        builder = builder.danger_accept_invalid_certs(true);
    }
    builder.build().context("failed to build reqwest client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_client_succeeds_without_ca() {
        let _client = build_http_client(None).expect("default client should build");
    }

    #[test]
    fn missing_ca_file_is_a_read_error() {
        let err = build_http_client(Some(Path::new("/nonexistent/ca.pem")))
            .expect_err("missing file must fail");
        assert!(err.to_string().contains("failed to read TLS CA cert"));
    }
}
