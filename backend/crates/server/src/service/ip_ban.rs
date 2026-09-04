//! 应用层 IP 封禁存储（admin-risk-controls WRFC）。
//!
//! 与 MaintenanceHandle 同模式的运行时句柄：内存真源 + data/ip_bans.json 持久化。
//! 条目两种形态：精确 IP（HashMap 直查）与 CIDR 前缀（条目量级在几十，线性扫描）。
//! 过期判定在读路径惰性进行（过期 = 视同未封）；compact() 由每日 housekeeping 清尸体。
//!
//! 拓扑前提：client-IP 口径复用 util::client_ip_from_headers（CF-Connecting-IP
//! 权威头仅在 Caddy remote_ip CF CIDR 守卫生效时可信），直连伪造头到不了后端。
//! 自封守卫在路由层（需要请求者 IP），存储层提供 would_cover 判定。

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

/// 封禁目标：精确 IP 或 CIDR（规范化存储）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BanTarget {
    Ip(IpAddr),
    Cidr(Cidr),
}

impl BanTarget {
    /// 解析 "1.2.3.4" / "10.0.0.0/8" / "2001:db8::/32"；族与前缀长度不匹配 → None。
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if let Some(addr) = s.strip_prefix('/') {
            return addr.parse::<IpAddr>().ok().map(Self::Ip);
        }
        if s.contains('/') {
            let (addr, prefix) = s.split_once('/')?;
            let addr: IpAddr = addr.parse().ok()?;
            let prefix: u8 = prefix.parse().ok()?;
            let cidr = Cidr::new(addr, prefix)?;
            return Some(Self::Cidr(cidr));
        }
        s.parse::<IpAddr>().ok().map(Self::Ip)
    }

    fn contains(&self, ip: IpAddr) -> bool {
        match self {
            Self::Ip(a) => *a == ip,
            Self::Cidr(c) => c.contains(ip),
        }
    }

    /// 展示形式（列表/审计用，规范化）。
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Ip(a) => a.to_string(),
            Self::Cidr(c) => c.to_string(),
        }
    }
}

/// CIDR 匹配（v4/v6 各自掩码；前缀 0 = 全匹配）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    addr: IpAddr,
    prefix: u8,
}

impl Cidr {
    #[must_use]
    pub fn new(addr: IpAddr, prefix: u8) -> Option<Self> {
        let max = match addr {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        if prefix > max {
            return None;
        }
        Some(Self { addr, prefix })
    }

    #[must_use]
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.addr, ip) {
            (IpAddr::V4(net), IpAddr::V4(target)) => {
                let mask = v4_mask(self.prefix);
                u32::from(net) & mask == u32::from(target) & mask
            },
            (IpAddr::V6(net), IpAddr::V6(target)) => {
                let mask = v6_mask(self.prefix);
                u128::from(net) & mask == u128::from(target) & mask
            },
            // v4/v6 不互通（无 NAT 语义）
            _ => false,
        }
    }
}

impl std::fmt::Display for Cidr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.addr, self.prefix)
    }
}

fn v4_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix))
    }
}

fn v6_mask(prefix: u8) -> u128 {
    if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - u32::from(prefix))
    }
}

/// 封禁条目。
#[derive(Debug, Clone, Serialize)]
pub struct BanEntry {
    pub id: Uuid,
    /// 规范化目标（IP 或 CIDR 字符串）。
    pub target: String,
    /// "ip" | "cidr"。
    pub kind: &'static str,
    pub reason: String,
    pub created_by: i64,
    pub created_at: OffsetDateTime,
    /// None = 永久（直至手动解除）。
    pub expires_at: Option<OffsetDateTime>,
    /// 展示派生：是否已过期（过期条目不再生效，但保留到 compact）。
    pub expired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BanFileEntry {
    id: Uuid,
    target: String,
    kind: String,
    reason: String,
    created_by: i64,
    created_at: String,
    expires_at: Option<String>,
}

/// add 失败原因（路由层映射 422/409）。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BanAddError {
    #[error("invalid ip or cidr")]
    Invalid,
    #[error("target already banned")]
    Duplicate,
}

/// 内存索引 + 条目表。
#[derive(Default)]
struct Inner {
    entries: HashMap<Uuid, BanEntry>,
    exact: HashMap<IpAddr, Uuid>,
    cidrs: Vec<(Cidr, Uuid)>,
}

/// 共享句柄。
#[derive(Clone)]
pub struct IpBanStore {
    inner: Arc<RwLock<Inner>>,
    path: PathBuf,
}

fn is_expired(e: &BanEntry, now: OffsetDateTime) -> bool {
    e.expires_at.is_some_and(|t| t <= now)
}

#[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
impl IpBanStore {
    /// 从持久化文件恢复（损坏则空表 + warn——封禁丢失是可用性问题，报警可见即可）。
    #[must_use]
    pub fn load_or_init(path: PathBuf) -> Self {
        let mut inner = Inner::default();
        if path.exists() {
            if let Some(files) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|c| serde_json::from_str::<Vec<BanFileEntry>>(&c).ok())
            {
                for f in files {
                    if let Some(entry) = file_to_entry(&f) {
                        index_entry(&mut inner, entry);
                    }
                }
            } else {
                tracing::warn!("ip_bans.json parse failed, starting with empty ban list");
            }
        }
        let store = Self {
            inner: Arc::new(RwLock::new(inner)),
            path,
        };
        tracing::info!(
            bans = store.inner.read().map_or(0, |i| i.entries.len()),
            "ip ban store loaded"
        );
        store
    }

    /// 读路径判定（纳秒级：一次读锁 + 精确查 + 少量 CIDR 线性扫描）。
    /// 锁 poison 时放行（fail-open）：封禁是增强层，不能因它打死全站。
    #[must_use]
    pub fn is_banned(&self, ip: IpAddr) -> bool {
        let now = OffsetDateTime::now_utc();
        let Ok(g) = self.inner.read() else {
            return false;
        };
        if let Some(id) = g.exact.get(&ip) {
            if let Some(e) = g.entries.get(id) {
                if !is_expired(e, now) {
                    return true;
                }
            }
        }
        g.cidrs
            .iter()
            .any(|(c, id)| c.contains(ip) && g.entries.get(id).is_some_and(|e| !is_expired(e, now)))
    }

    /// 判定目标是否覆盖某 IP（自封守卫：请求者 IP 命中 → 拒绝封禁）。
    /// 解析失败按不覆盖处理（路由层已先验证过格式，这里是双保险）。
    #[must_use]
    pub fn would_cover(&self, target: &str, ip: IpAddr) -> bool {
        BanTarget::parse(target).is_some_and(|t| t.contains(ip))
    }

    /// 添加封禁。TTL 秒数（None = 永久）。
    pub fn add(
        &self,
        target: &str,
        reason: &str,
        ttl_secs: Option<i64>,
        created_by: i64,
    ) -> Result<BanEntry, BanAddError> {
        let parsed = BanTarget::parse(target).ok_or(BanAddError::Invalid)?;
        let now = OffsetDateTime::now_utc();
        let entry = BanEntry {
            id: Uuid::new_v4(),
            target: parsed.display(),
            kind: match parsed {
                BanTarget::Ip(_) => "ip",
                BanTarget::Cidr(_) => "cidr",
            },
            reason: reason.to_string(),
            created_by,
            created_at: now,
            expires_at: ttl_secs.map(|s| now + time::Duration::seconds(s)),
            expired: false,
        };
        let mut g = self.inner.write().map_err(|_| BanAddError::Invalid)?; // poison：写锁不可得属内部错误，Invalid 占位不理想但路径不可达
        // 重复判定（按规范化目标字符串）
        if g.entries.values().any(|e| e.target == entry.target) {
            return Err(BanAddError::Duplicate);
        }
        index_entry(&mut g, entry.clone());
        drop(g);
        self.persist();
        Ok(entry)
    }

    /// 解除封禁（按 id）。返回被移除条目（None = 不存在）。
    #[must_use]
    pub fn remove(&self, id: Uuid) -> Option<BanEntry> {
        let mut g = self.inner.write().ok()?;
        let entry = g.entries.remove(&id)?;
        match BanTarget::parse(&entry.target) {
            Some(BanTarget::Ip(ip)) => {
                g.exact.remove(&ip);
            },
            Some(BanTarget::Cidr(_)) => g.cidrs.retain(|(_, cid)| *cid != id),
            None => {},
        }
        drop(g);
        self.persist();
        Some(entry)
    }

    /// 列表（含已过期条目，前端按 expired 标记展示/过滤）。
    #[must_use]
    pub fn list(&self) -> Vec<BanEntry> {
        let now = OffsetDateTime::now_utc();
        let Ok(g) = self.inner.read() else {
            return Vec::new();
        };
        let mut out: Vec<BanEntry> = g.entries.values().cloned().collect();
        for e in &mut out {
            e.expired = is_expired(e, now);
        }
        out.sort_by_key(|e| e.created_at);
        out
    }

    /// 有效条目数（risk 面板）。
    #[must_use]
    pub fn active_count(&self) -> usize {
        let now = OffsetDateTime::now_utc();
        let Ok(g) = self.inner.read() else {
            return 0;
        };
        g.entries.values().filter(|e| !is_expired(e, now)).count()
    }

    /// 清理过期条目（每日 housekeeping 调用）。
    pub fn compact(&self) {
        let now = OffsetDateTime::now_utc();
        let expired_ids: Vec<Uuid> = {
            let Ok(g) = self.inner.read() else {
                return;
            };
            g.entries
                .values()
                .filter(|e| is_expired(e, now))
                .map(|e| e.id)
                .collect()
        };
        if expired_ids.is_empty() {
            return;
        }
        {
            let mut g = match self.inner.write() {
                Ok(g) => g,
                Err(_) => return,
            };
            for id in &expired_ids {
                if let Some(entry) = g.entries.remove(id) {
                    match BanTarget::parse(&entry.target) {
                        Some(BanTarget::Ip(ip)) => {
                            g.exact.remove(&ip);
                        },
                        Some(BanTarget::Cidr(_)) => g.cidrs.retain(|(_, cid)| cid != id),
                        None => {},
                    }
                }
            }
        }
        tracing::info!(count = expired_ids.len(), "expired ip bans compacted");
        self.persist();
    }

    /// best-effort 持久化。
    fn persist(&self) {
        let files = {
            let Ok(g) = self.inner.read() else {
                return;
            };
            g.entries
                .values()
                .map(|e| BanFileEntry {
                    id: e.id,
                    target: e.target.clone(),
                    kind: e.kind.to_string(),
                    reason: e.reason.clone(),
                    created_by: e.created_by,
                    created_at: e.created_at.format(&Rfc3339).unwrap_or_default(),
                    expires_at: e.expires_at.and_then(|t| t.format(&Rfc3339).ok()),
                })
                .collect::<Vec<_>>()
        };
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(&files) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.path, json) {
                    tracing::warn!(error = ?e, "failed to persist ip_bans.json");
                }
            },
            Err(e) => tracing::warn!(error = ?e, "ip_bans.json serialize failed"),
        }
    }
}

fn index_entry(inner: &mut Inner, entry: BanEntry) {
    match BanTarget::parse(&entry.target) {
        Some(BanTarget::Ip(ip)) => {
            inner.exact.insert(ip, entry.id);
        },
        Some(BanTarget::Cidr(c)) => inner.cidrs.push((c, entry.id)),
        None => return,
    }
    inner.entries.insert(entry.id, entry);
}

fn file_to_entry(f: &BanFileEntry) -> Option<BanEntry> {
    let target = BanTarget::parse(&f.target)?;
    Some(BanEntry {
        id: f.id,
        target: target.display(),
        kind: match target {
            BanTarget::Ip(_) => "ip",
            BanTarget::Cidr(_) => "cidr",
        },
        reason: f.reason.clone(),
        created_by: f.created_by,
        created_at: OffsetDateTime::parse(&f.created_at, &Rfc3339).ok()?,
        expires_at: f
            .expires_at
            .as_deref()
            .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok()),
        expired: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path() -> PathBuf {
        std::env::temp_dir().join(format!("wakewake_ipban_test_{}.json", Uuid::new_v4()))
    }

    #[test]
    fn parse_targets() {
        assert_eq!(
            BanTarget::parse("192.0.2.1"),
            Some(BanTarget::Ip("192.0.2.1".parse().unwrap()))
        );
        assert_eq!(
            BanTarget::parse("2001:db8::1"),
            Some(BanTarget::Ip("2001:db8::1".parse().unwrap()))
        );
        assert_eq!(
            BanTarget::parse("10.1.0.0/16"),
            Some(BanTarget::Cidr(
                Cidr::new("10.1.0.0".parse().unwrap(), 16).unwrap()
            ))
        );
        // 前缀越界 / 非法 IP
        assert_eq!(BanTarget::parse("10.0.0.0/33"), None);
        assert_eq!(BanTarget::parse("2001:db8::/129"), None);
        assert_eq!(BanTarget::parse("not-an-ip"), None);
        assert_eq!(BanTarget::parse(""), None);
    }

    #[test]
    fn cidr_matching() {
        let c = Cidr::new("203.0.113.0".parse().unwrap(), 24).unwrap();
        assert!(c.contains("203.0.113.77".parse().unwrap()));
        assert!(!c.contains("203.0.114.1".parse().unwrap()));
        assert!(!c.contains("2001:db8::1".parse().unwrap()), "no v4/v6 mix");

        let zero = Cidr::new("203.0.113.9".parse().unwrap(), 0).unwrap();
        assert!(
            zero.contains("198.51.100.1".parse().unwrap()),
            "/0 matches all v4"
        );
        assert!(!zero.contains("2001:db8::1".parse().unwrap()));

        let v6 = Cidr::new("2001:db8::".parse().unwrap(), 32).unwrap();
        assert!(v6.contains("2001:db8:ffff::1".parse().unwrap()));
        assert!(!v6.contains("2001:db9::1".parse().unwrap()));

        let full = Cidr::new("192.0.2.5".parse().unwrap(), 32).unwrap();
        assert!(full.contains("192.0.2.5".parse().unwrap()));
        assert!(!full.contains("192.0.2.6".parse().unwrap()));
    }

    #[test]
    fn add_match_remove_round_trip() {
        let s = IpBanStore::load_or_init(tmp_path());
        let e1 = s.add("203.0.113.5", "stuffed", Some(3600), 1).unwrap();
        let e2 = s.add("198.51.100.0/24", "botnet", None, 1).unwrap();

        assert!(s.is_banned("203.0.113.5".parse().unwrap()));
        assert!(s.is_banned("198.51.100.200".parse().unwrap()));
        assert!(!s.is_banned("203.0.113.6".parse().unwrap()));
        assert!(!s.is_banned("192.0.2.1".parse().unwrap()));

        // 重复目标拒绝
        assert_eq!(
            s.add("203.0.113.5", "dup", None, 1).unwrap_err(),
            BanAddError::Duplicate
        );
        // 非法目标拒绝
        assert_eq!(
            s.add("999.1.1.1", "bad", None, 1).unwrap_err(),
            BanAddError::Invalid
        );

        assert!(s.remove(e1.id).is_some());
        assert!(!s.is_banned("203.0.113.5".parse().unwrap()));
        assert!(
            s.is_banned("198.51.100.1".parse().unwrap()),
            "e2 unaffected"
        );
        assert!(s.remove(e2.id).is_some());
        assert_eq!(s.active_count(), 0);
    }

    #[test]
    fn expired_entry_stops_matching_until_compacted() {
        let s = IpBanStore::load_or_init(tmp_path());
        let e = s.add("203.0.113.9", "short", Some(0), 1).unwrap(); // 立即过期
        assert!(
            !s.is_banned("203.0.113.9".parse().unwrap()),
            "expired ban must not match"
        );
        assert_eq!(s.list().len(), 1, "still listed until compact");
        s.compact();
        assert!(s.list().is_empty());
        assert!(s.remove(e.id).is_none(), "already compacted");
    }

    #[test]
    fn would_cover_for_self_ban_guard() {
        let s = IpBanStore::load_or_init(tmp_path());
        let admin_ip: IpAddr = "203.0.113.5".parse().unwrap();
        assert!(s.would_cover("203.0.113.5", admin_ip));
        assert!(s.would_cover("203.0.113.0/24", admin_ip));
        assert!(!s.would_cover("198.51.0.0/16", admin_ip));
    }

    #[test]
    fn persists_and_restores() {
        let path = tmp_path();
        {
            let s = IpBanStore::load_or_init(path.clone());
            let _ = s.add("203.0.113.1", "r1", None, 7).unwrap();
            let _ = s.add("2001:db8:a::/48", "r2", None, 7).unwrap();
        }
        let s2 = IpBanStore::load_or_init(path.clone());
        assert!(s2.is_banned("203.0.113.1".parse().unwrap()));
        assert!(s2.is_banned("2001:db8:a::99".parse().unwrap()));
        assert_eq!(s2.list().len(), 2);
        let _ = std::fs::remove_file(&path);
    }
}
