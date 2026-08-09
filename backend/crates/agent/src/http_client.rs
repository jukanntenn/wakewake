//! HTTP client `工厂（sse_client` + reporter 共用，e2e.md §6.2）。
//!
//! `danger-insecure-tls` feature 启用时，加 `danger_accept_invalid_certs(true)`
//! 信任 Caddy `tls internal` 自签名证书。生产 agent 不带此 feature，零影响。
//!
//! reqwest 0.12：feature 名 `rustls-tls`（workspace Cargo.toml），方法名
//! `danger_accept_invalid_certs`（非 0.13 的 `tls_danger_accept_invalid_certs`）。

/// 构建 reqwest Client。feature gate `danger-insecure-tls` 控制是否信任自签名证书。
pub fn build_http_client() -> reqwest::Client {
    let builder = reqwest::Client::builder();
    #[cfg(feature = "danger-insecure-tls")]
    let builder = builder.danger_accept_invalid_certs(true);
    match builder.build() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = ?e, "reqwest client build failed");
            panic!("reqwest client build failed: {e}")
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_client_succeeds() {
        // 默认（无 feature）应成功构建
        let _client = build_http_client();
    }
}
