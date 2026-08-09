//! MAC 地址解析（agents/wol.md §3）。纯函数，宽容格式。

/// 解析 MAC。接受 `:`/`-`/无分隔符/混合大小写。
/// 去掉分隔符后正好 12 个 hex 字符。不强制多播位校验。
pub fn parse_mac(input: &str) -> Result<[u8; 6], MacError> {
    let hex: Vec<char> = input
        .chars()
        .filter(|&c| c != ':' && c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if hex.len() != 12 {
        return Err(MacError::Invalid(input.to_string()));
    }
    let mut mac = [0u8; 6];
    for (i, pair) in hex.chunks(2).enumerate() {
        let hi = pair[0]
            .to_digit(16)
            .ok_or_else(|| MacError::Invalid(input.to_string()))?;
        let lo = pair[1]
            .to_digit(16)
            .ok_or_else(|| MacError::Invalid(input.to_string()))?;
        mac[i] = (hi as u8) << 4 | lo as u8;
    }
    Ok(mac)
}

#[derive(thiserror::Error, Debug)]
pub enum MacError {
    #[error("invalid MAC address: {0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colon_format() {
        assert_eq!(
            parse_mac("AA:BB:CC:DD:EE:FF").unwrap(),
            [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]
        );
    }

    #[test]
    fn parses_hyphen_format() {
        assert_eq!(
            parse_mac("AA-BB-CC-DD-EE-FF").unwrap(),
            [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]
        );
    }

    #[test]
    fn parses_no_separator() {
        assert_eq!(
            parse_mac("AABBCCDDEEFF").unwrap(),
            [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]
        );
    }

    #[test]
    fn parses_lowercase() {
        assert_eq!(
            parse_mac("aa:bb:cc:dd:ee:ff").unwrap(),
            [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]
        );
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(parse_mac("AA:BB:CC").is_err());
        assert!(parse_mac("AABBCCDDEEFF00").is_err());
    }

    #[test]
    fn rejects_non_hex() {
        assert!(parse_mac("GG:BB:CC:DD:EE:FF").is_err());
    }
}
