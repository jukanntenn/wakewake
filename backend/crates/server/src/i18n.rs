//! 后端 i18n（rust-i18n，仅邮件模板，backend/i18n.md）。
//!
//! 编译期加载 locales/*.toml（i18n! 宏在 lib.rs 顶层）。
//! 邮件渲染用 `user.preferred_locale（异步场景，无请求上下文，backend/i18n.md` §5）。
//! 错误 message 不经过后端 i18n（走 `ErrorCode::fallback_message，前端翻译`）。
//!
//! t! 返回 Cow<'_, str>，邮件渲染转 String。

/// 渲染邮件主题（per-locale，backend/i18n.md §5）。
#[must_use]
pub fn password_reset_subject(locale: &str) -> String {
    rust_i18n::t!("email.password_reset_subject", locale = locale).to_string()
}

/// 渲染密码重置邮件纯文本正文（插入 `reset_url`）。
#[must_use]
pub fn password_reset_body(locale: &str, reset_url: &str) -> String {
    rust_i18n::t!(
        "email.password_reset_body",
        locale = locale,
        reset_url = reset_url
    )
    .to_string()
}

/// 渲染密码重置邮件 HTML 正文：统一布局（内联 CSS 卡片）+ i18n 文案插槽。
/// `reset_url` 在插入 `href` 前做 HTML 属性转义（防 token 含特殊字符破坏标签）。
#[must_use]
pub fn password_reset_html_body(locale: &str, reset_url: &str) -> String {
    let heading = rust_i18n::t!("email.password_reset_heading", locale = locale).to_string();
    let body_text = rust_i18n::t!("email.password_reset_html_body", locale = locale).to_string();
    let button = rust_i18n::t!("email.password_reset_button", locale = locale).to_string();
    render_html_email(&heading, &body_text, &button, reset_url)
}

/// 渲染密码已更改通知主题。
#[must_use]
pub fn password_changed_subject(locale: &str) -> String {
    rust_i18n::t!("email.password_changed_subject", locale = locale).to_string()
}

/// 渲染密码已更改通知正文。
#[must_use]
pub fn password_changed_body(locale: &str) -> String {
    rust_i18n::t!("email.password_changed_body", locale = locale).to_string()
}

/// 渲染邮箱验证邮件主题。
#[must_use]
pub fn email_verification_subject(locale: &str) -> String {
    rust_i18n::t!("email.email_verification_subject", locale = locale).to_string()
}

/// 渲染邮箱验证邮件纯文本正文（插入 verify_url）。
#[must_use]
pub fn email_verification_body(locale: &str, verify_url: &str) -> String {
    rust_i18n::t!(
        "email.email_verification_body",
        locale = locale,
        verify_url = verify_url
    )
    .to_string()
}

/// 渲染邮箱验证邮件 HTML 正文：统一布局 + i18n 文案插槽。
/// `verify_url` 在插入 `href` 前做 HTML 属性转义。
#[must_use]
pub fn email_verification_html_body(locale: &str, verify_url: &str) -> String {
    let heading = rust_i18n::t!("email.email_verification_heading", locale = locale).to_string();
    let body_text =
        rust_i18n::t!("email.email_verification_html_body", locale = locale).to_string();
    let button = rust_i18n::t!("email.email_verification_button", locale = locale).to_string();
    render_html_email(&heading, &body_text, &button, verify_url)
}

/// 对字符串做 HTML 文本转义（`&` `<` `>` `"` `'`）。
fn escape_html_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 对字符串做 HTML 属性值转义（文本转义 + `"` `'`）。
fn escape_html_attr(s: &str) -> String {
    escape_html_text(s)
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// 统一 HTML 邮件布局（内联 CSS 极简卡片，兼容各邮件客户端）。
/// 插槽：heading（标题）/ body_text（引导语）/ button_text（按钮文案）/ url（按钮链接）。
fn render_html_email(heading: &str, body_text: &str, button_text: &str, url: &str) -> String {
    let h = escape_html_text(heading);
    let b = escape_html_text(body_text);
    let btn = escape_html_text(button_text);
    let href = escape_html_attr(url);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="margin:0;padding:0;background:#f4f4f5;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
  <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="padding:24px 0;">
    <tr><td align="center">
      <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:480px;background:#ffffff;border-radius:8px;border:1px solid #e4e4e7;overflow:hidden;">
        <tr><td style="padding:24px 24px 0;">
          <h1 style="margin:0 0 12px;font-size:18px;font-weight:600;color:#18181b;">{h}</h1>
          <p style="margin:0 0 20px;font-size:14px;line-height:1.6;color:#52525b;">{b}</p>
        </td></tr>
        <tr><td style="padding:0 24px 24px;">
          <a href="{href}" style="display:inline-block;padding:10px 20px;background:#2563eb;color:#ffffff;text-decoration:none;font-size:14px;font-weight:500;border-radius:6px;">{btn}</a>
        </td></tr>
      </table>
      <p style="margin:16px 0 0;font-size:12px;color:#a1a1aa;">WakeWake</p>
    </td></tr>
  </table>
</body>
</html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_password_reset_subject() {
        assert_eq!(password_reset_subject("en"), "Reset your WakeWake password");
    }

    #[test]
    fn zh_password_reset_subject() {
        assert_eq!(password_reset_subject("zh"), "重置您的 WakeWake 密码");
    }

    #[test]
    fn en_password_reset_body_interpolation() {
        let body = password_reset_body("en", "https://app/reset?token=abc");
        assert!(body.contains("https://app/reset?token=abc"));
    }

    #[test]
    fn missing_key_falls_back_to_en() {
        // 未知 locale fallback 到 en
        let s = password_reset_subject("xx");
        assert_eq!(s, "Reset your WakeWake password");
    }

    #[test]
    fn en_email_verification_html_contains_link_and_button() {
        let html = email_verification_html_body("en", "https://app/verify-email?token=abc");
        // href 含转义后的完整 URL
        assert!(html.contains(r#"href="https://app/verify-email?token=abc""#));
        // 有可点击按钮文案
        assert!(html.contains("Verify"));
        // 是合法 HTML 文档骨架
        assert!(html.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn html_escapes_ampersand_in_url() {
        // URL 含 & 应在 href 属性里被转义为 &amp;（HTML 属性安全）
        let html = email_verification_html_body("en", "https://app/v?a=1&b=2");
        assert!(html.contains(r#"href="https://app/v?a=1&amp;b=2""#));
        // 不应出现未转义的裸 & 在属性里
        assert!(!html.contains(r#"href="https://app/v?a=1&b=2""#));
    }

    #[test]
    fn html_escapes_text_content() {
        // heading/body 文案里若有 < > 应被转义，防注入
        let html = render_html_email("a<b", "x>y", "btn", "https://app/x");
        assert!(html.contains("a&lt;b"));
        assert!(html.contains("x&gt;y"));
        assert!(!html.contains("<b>"));
    }

    #[test]
    fn escape_html_attr_quotes() {
        assert_eq!(escape_html_attr(r#"a"b'c"#), "a&quot;b&#x27;c");
    }
}
