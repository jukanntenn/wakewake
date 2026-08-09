//! 登录失败锁定（authentication.md §八）。
//!
//! 同一 email 连续登录失败 ≥5 次 → 锁定该账号登录 15min。
//! 成功登录后清除该 email 的失败计数。
//! 单机 MVP：内存 DashMap（email → 计数 + 首次失败时间）。横向扩展换 Redis 计数器。

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

/// 锁定阈值（authentication.md §八：≥5 次）。
const MAX_ATTEMPTS: u32 = 5;
/// 锁定时长（15min）。
const LOCK_DURATION: Duration = Duration::from_mins(15);

#[derive(Clone)]
pub struct LoginLockout {
    /// email → (失败次数, 首次失败时间)。
    attempts: Arc<DashMap<String, (u32, Instant)>>,
}

impl LoginLockout {
    #[must_use]
    pub fn new() -> Self {
        Self {
            attempts: Arc::new(DashMap::new()),
        }
    }

    /// 该 email 是否被锁定（失败 ≥5 次且在 15min 窗口内）。
    #[must_use]
    pub fn is_locked(&self, email: &str) -> bool {
        if let Some(entry) = self.attempts.get(&email.to_lowercase()) {
            let (count, first_fail) = *entry;
            if count >= MAX_ATTEMPTS {
                // 窗口内锁定
                if first_fail.elapsed() < LOCK_DURATION {
                    return true;
                }
                // 窗口过期 → 重置（允许重试）
                drop(entry);
                self.attempts.remove(&email.to_lowercase());
            }
        }
        false
    }

    /// 记录一次失败。返回当前失败次数。
    #[must_use]
    pub fn record_failure(&self, email: &str) -> u32 {
        let key = email.to_lowercase();
        let mut entry = self.attempts.entry(key).or_insert((0, Instant::now()));
        entry.0 = entry.0.saturating_add(1);
        // 重置首次失败时间（若超过窗口，重新计数）
        if entry.1.elapsed() >= LOCK_DURATION {
            entry.0 = 1;
            entry.1 = Instant::now();
        }
        entry.0
    }

    /// 登录成功 → 清除该 email 的失败计数。
    pub fn record_success(&self, email: &str) {
        self.attempts.remove(&email.to_lowercase());
    }

    /// 清理过期条目（定时 task 调用，防内存无限增长）。
    pub fn gc(&self) {
        let now = Instant::now();
        self.attempts
            .retain(|_, (_, first_fail)| now.duration_since(*first_fail) < LOCK_DURATION);
    }
}

impl Default for LoginLockout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_after_5_failures() {
        let lockout = LoginLockout::new();
        let email = "test@example.com";
        for _ in 0..4 {
            let _ = lockout.record_failure(email);
            assert!(!lockout.is_locked(email));
        }
        let _ = lockout.record_failure(email); // 5th
        assert!(lockout.is_locked(email));
    }

    #[test]
    fn success_resets_count() {
        let lockout = LoginLockout::new();
        let email = "test@example.com";
        let _ = lockout.record_failure(email);
        let _ = lockout.record_failure(email);
        lockout.record_success(email);
        assert!(!lockout.is_locked(email));
        assert_eq!(lockout.record_failure(email), 1);
    }

    #[test]
    fn different_emails_independent() {
        let lockout = LoginLockout::new();
        for _ in 0..5 {
            let _ = lockout.record_failure("a@example.com");
        }
        assert!(lockout.is_locked("a@example.com"));
        assert!(!lockout.is_locked("b@example.com"));
    }

    #[test]
    fn case_insensitive() {
        let lockout = LoginLockout::new();
        for _ in 0..5 {
            let _ = lockout.record_failure("User@Example.COM");
        }
        assert!(lockout.is_locked("user@example.com"));
    }
}
