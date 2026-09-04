//! 邮件服务（lettre AsyncSmtpTransport，authentication.md §七层5）。
//!
//! 密码重置低频异步发邮件。mailer.enabled=false 时返 `MailerError::Disabled（端点` 503）。
//! SMTP 发送失败记日志 + 监控告警，不影响 HTTP 服务（邮件是 best-effort）。
//!
//! 发信预算（admin-risk-controls WRFC）：每次发送先经 MailerControl 原子占用
//! 分路日预算（register/resend/reset 分账）；运行时总闸关闭或预算耗尽 →
//! BudgetExhausted/Disabled，调用方按「mailer 关闭」的既有语义降级
//! （注册静默跳过 / resend 静默 / 重置请求 503）。

use std::sync::Arc;

use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::MailerSettings;
use crate::observability::metrics;
use crate::service::mailer_control::{AcquireOutcome, MailPath, MailerControl};

/// 邮件错误。
#[derive(thiserror::Error, Debug)]
pub enum MailerError {
    /// mailer.enabled=false（端点返 503 `SERVICE_UNAVAILABLE`）。
    #[error("mailer disabled")]
    Disabled,
    /// 运行时总闸关闭或当日分路预算耗尽（降级语义与 Disabled 相同）。
    #[error("mailer budget exhausted for path {0}")]
    BudgetExhausted(&'static str),
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
    control: MailerControl,
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
    pub fn new(cfg: &MailerSettings, control: MailerControl) -> Self {
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
            control,
        }
    }

    /// 占用一个发送名额：配置/总闸/预算三重判定。
    fn acquire(&self, path: MailPath) -> Result<(), MailerError> {
        if !self.inner.enabled {
            return Err(MailerError::Disabled);
        }
        match self.control.try_acquire(path) {
            AcquireOutcome::Allowed => {
                metrics::record_email_sent(path.as_str());
                Ok(())
            },
            AcquireOutcome::Disabled => {
                metrics::record_email_blocked(path.as_str(), "disabled");
                Err(MailerError::Disabled)
            },
            AcquireOutcome::Exhausted => {
                metrics::record_email_blocked(path.as_str(), "exhausted");
                Err(MailerError::BudgetExhausted(path.as_str()))
            },
        }
    }

    /// 发送密码重置邮件（HTML + 纯文本 multipart）。总闸关闭 → 503；预算耗尽 → BudgetExhausted。
    pub async fn send_password_reset(
        &self,
        to: &str,
        locale: &str,
        reset_url: &str,
    ) -> Result<(), MailerError> {
        self.acquire(MailPath::Reset)?;
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
    /// `path` 区分注册首信（Register）与用户手动重发（Resend）——分路预算分账。
    pub async fn send_email_verification(
        &self,
        path: MailPath,
        to: &str,
        locale: &str,
        verify_url: &str,
    ) -> Result<(), MailerError> {
        self.acquire(path)?;
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

    /// 是否启用（配置级；运行时总闸见 would_send）。
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
    // 空串视为无凭证：mailpit 等开放中继不宣告 AUTH 机制，若设了（空）Credentials，
    // lettre 会强制 AUTH → "No compatible authentication mechanism was found"（BUG-MAILER）。
    // 仅当 username/password 均非空才设凭证。
    let creds = username
        .filter(|u| !u.is_empty())
        .zip(password.filter(|p| !p.is_empty()));
    if let Some((u, p)) = creds {
        builder = builder.credentials(Credentials::new(u.to_string(), p.to_string()));
    }
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::mailer_control::{AcquireOutcome, MailLimits, MailPath, MailerControl};

    fn tmp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "wakewake_mailsvc_test_{tag}_{}",
            uuid::Uuid::new_v4()
        ))
    }

    fn enabled_cfg() -> MailerSettings {
        // 无 smtp_host：transport 缺失，但 acquire 在 transport 查找之前——
        // 足以覆盖预算/总闸的拒绝路径。
        serde_json::from_str(r#"{"enabled": true, "smtp_port": 587, "from_name": "t"}"#)
            .expect("test mailer settings")
    }

    #[tokio::test]
    async fn send_rejects_with_budget_exhausted_and_counts_blocked() {
        let control = MailerControl::load_or_init(
            MailLimits {
                register: 1,
                ..MailLimits::default()
            },
            tmp_path("exhaust"),
        );
        // 预先占掉唯一的名额
        assert_eq!(
            control.try_acquire(MailPath::Register),
            AcquireOutcome::Allowed
        );
        let svc = MailerService::new(&enabled_cfg(), control.clone());
        let err = svc
            .send_email_verification(MailPath::Register, "a@b.co", "en", "https://x/verify")
            .await
            .expect_err("must be rejected");
        assert!(
            matches!(err, MailerError::BudgetExhausted("register")),
            "got {err:?}"
        );
        let snap = control.snapshot();
        assert_eq!(snap.sent.register, 1, "exhausted attempt must not consume");
        assert_eq!(snap.blocked.register, 1, "rejection is visible");
    }

    #[tokio::test]
    async fn send_rejects_with_disabled_when_config_off() {
        let control = MailerControl::load_or_init(MailLimits::default(), tmp_path("off"));
        let cfg: MailerSettings =
            serde_json::from_str(r#"{"enabled": false, "smtp_port": 587, "from_name": "t"}"#)
                .expect("test mailer settings");
        let svc = MailerService::new(&cfg, control.clone());
        let err = svc
            .send_password_reset("a@b.co", "en", "https://x/reset")
            .await
            .expect_err("must be rejected");
        assert!(matches!(err, MailerError::Disabled), "got {err:?}");
        // 配置态拒绝不进风控计数
        let snap = control.snapshot();
        assert_eq!(snap.blocked.reset, 0);
    }

    #[tokio::test]
    async fn runtime_gate_off_rejects_and_counts() {
        let control = MailerControl::load_or_init(MailLimits::default(), tmp_path("gate"));
        control.set_enabled(false, None);
        let svc = MailerService::new(&enabled_cfg(), control.clone());
        let err = svc
            .send_email_verification(MailPath::Resend, "a@b.co", "en", "https://x/verify")
            .await
            .expect_err("must be rejected");
        assert!(matches!(err, MailerError::Disabled), "got {err:?}");
        assert_eq!(control.snapshot().blocked.resend, 1);
    }
}
