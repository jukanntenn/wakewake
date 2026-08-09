//! 邮件服务（lettre AsyncSmtpTransport，authentication.md §七层5）。
//!
//! 密码重置低频异步发邮件。mailer.enabled=false 时返 `MailerError::Disabled（端点` 503）。
//! SMTP 发送失败记日志 + 监控告警，不影响 HTTP 服务（邮件是 best-effort）。

use std::sync::Arc;

use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::MailerSettings;

/// 邮件错误。
#[derive(thiserror::Error, Debug)]
pub enum MailerError {
    /// mailer.enabled=false（端点返 503 `SERVICE_UNAVAILABLE`）。
    #[error("mailer disabled")]
    Disabled,
    #[error("mailer not configured (missing smtp_host/from_address)")]
    NotConfigured,
    #[error("smtp send failed: {0}")]
    Send(#[from] lettre::transport::smtp::Error),
    #[error("address parse failed: {0}")]
    Address(#[from] lettre::address::AddressError),
    #[error("message build failed: {0}")]
    Build(String),
}

/// 邮件服务（Clone 廉价，内部 Arc）。
#[derive(Clone)]
pub struct MailerService {
    inner: Arc<Inner>,
}

struct Inner {
    enabled: bool,
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from_mailbox: Option<Mailbox>,
    /// 发件人显示名（已并入 `from_mailbox` 的 name 部分；保留以备未来自定义显示名）。
    #[allow(dead_code)]
    from_name: String,
}

impl MailerService {
    #[must_use]
    pub fn new(cfg: &MailerSettings) -> Self {
        let from_mailbox = cfg
            .from_address
            .as_deref()
            .and_then(|a| a.parse::<Mailbox>().ok());

        let transport = if cfg.enabled {
            cfg.smtp_host.as_deref().map(|host| {
                build_transport(
                    host,
                    cfg.smtp_port,
                    cfg.smtp_username.as_deref(),
                    cfg.smtp_password.as_deref(),
                )
            })
        } else {
            None
        };

        Self {
            inner: Arc::new(Inner {
                enabled: cfg.enabled,
                transport,
                from_mailbox,
                from_name: cfg.from_name.clone(),
            }),
        }
    }

    /// 发送密码重置邮件（HTML + 纯文本 multipart）。mailer.enabled=false → Disabled（端点 503）。
    pub async fn send_password_reset(
        &self,
        to: &str,
        locale: &str,
        reset_url: &str,
    ) -> Result<(), MailerError> {
        if !self.inner.enabled {
            return Err(MailerError::Disabled);
        }
        let transport = self
            .inner
            .transport
            .as_ref()
            .ok_or(MailerError::NotConfigured)?;
        let from = self
            .inner
            .from_mailbox
            .as_ref()
            .ok_or(MailerError::NotConfigured)?;

        let subject = crate::i18n::password_reset_subject(locale);
        let plain = crate::i18n::password_reset_body(locale, reset_url);
        let html = crate::i18n::password_reset_html_body(locale, reset_url);

        self.send_msg(transport, from, to, subject, plain, html)
            .await
    }

    /// 发送邮箱验证邮件（HTML + 纯文本 multipart）。与 reset 同构。
    pub async fn send_email_verification(
        &self,
        to: &str,
        locale: &str,
        verify_url: &str,
    ) -> Result<(), MailerError> {
        if !self.inner.enabled {
            return Err(MailerError::Disabled);
        }
        let transport = self
            .inner
            .transport
            .as_ref()
            .ok_or(MailerError::NotConfigured)?;
        let from = self
            .inner
            .from_mailbox
            .as_ref()
            .ok_or(MailerError::NotConfigured)?;

        let subject = crate::i18n::email_verification_subject(locale);
        let plain = crate::i18n::email_verification_body(locale, verify_url);
        let html = crate::i18n::email_verification_html_body(locale, verify_url);

        self.send_msg(transport, from, to, subject, plain, html)
            .await
    }

    async fn send_msg(
        &self,
        transport: &AsyncSmtpTransport<Tokio1Executor>,
        from: &Mailbox,
        to: &str,
        subject: String,
        plain_body: String,
        html_body: String,
    ) -> Result<(), MailerError> {
        let to_mailbox: Mailbox = to.parse()?;
        let msg = Message::builder()
            .from(from.clone())
            .to(to_mailbox)
            .subject(subject)
            .multipart(lettre::message::MultiPart::alternative_plain_html(
                plain_body, html_body,
            ))
            .map_err(|e| MailerError::Build(e.to_string()))?;
        transport.send(msg).await?;
        Ok(())
    }

    /// 是否启用。
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.inner.enabled
    }
}

/// 构造 AsyncSmtpTransport（生产 STARTTLS 587；测试/mock 用 plain SMTP）。
/// port != 587 → plain relay（E2E mailpit 不支持 STARTTLS）。
fn build_transport(
    host: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> AsyncSmtpTransport<Tokio1Executor> {
    let mut builder = if port == 587 {
        // 生产：STARTTLS relay（587）。credentials 可选。
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .expect("smtp relay")
            .port(port)
    } else {
        // E2E/mailpit：plain SMTP（不支持 STARTTLS）。
        AsyncSmtpTransport::<Tokio1Executor>::relay(host)
            .expect("smtp relay")
            .port(port)
            .tls(lettre::transport::smtp::client::Tls::None)
    };
    if let (Some(u), Some(p)) = (username, password) {
        builder = builder.credentials(Credentials::new(u.to_string(), p.to_string()));
    }
    builder.build()
}
