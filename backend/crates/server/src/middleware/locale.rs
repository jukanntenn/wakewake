//! Accept-Language 解析（backend/i18n.md §6）。
//!
//! 自建 ~20 行，不引第三方 crate（精简哲学）。返回最佳匹配的 supported locale。

/// supported locales（与前端 8 语言对齐）。
pub const SUPPORTED: &[&str] = &["en", "zh", "ja", "ko", "de", "fr", "es", "pt"];

/// 解析 Accept-Language 头，返回最佳匹配的 supported locale（默认 "en"）。
#[must_use]
pub fn parse_accept_language(header: &str) -> String {
    let mut candidates: Vec<(&str, f32)> = header
        .split(',')
        .map(|part| {
            let part = part.trim();
            if let Some((t, q)) = part.split_once(';') {
                let q = q
                    .strip_prefix("q=")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1.0);
                (t, q)
            } else {
                (part, 1.0)
            }
        })
        .collect();
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (tag, _) in candidates {
        let tag = tag.to_lowercase();
        if SUPPORTED.contains(&tag.as_str()) {
            return tag;
        }
        // 主码匹配（zh-CN → zh）
        let primary = tag.split(['-', '_']).next().unwrap_or("en");
        if SUPPORTED.contains(&primary) {
            return primary.to_string();
        }
    }
    "en".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_accept_language() {
        assert_eq!(parse_accept_language("zh-CN,zh;q=0.9,en;q=0.8"), "zh");
        assert_eq!(parse_accept_language("en-US,en;q=0.9"), "en");
        assert_eq!(parse_accept_language("ja"), "ja");
        assert_eq!(parse_accept_language(""), "en");
        assert_eq!(parse_accept_language("fr-FR"), "fr");
    }
}
