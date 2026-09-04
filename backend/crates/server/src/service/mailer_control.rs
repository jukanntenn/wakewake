//! 邮件发送运行态（admin-risk-controls WRFC）。
//!
//! Settings.mailer 是启动配置（不可变 Arc<Settings>）；此处提供运行时可变的
//! MailerControl（Arc<RwLock>，与 MaintenanceHandle 同模式）：
//! - `enabled`：运行时总闸（POST /admin/mailer 实时启停，免重启）。
//! - 分路日预算：register / resend / reset 三路独立计数，UTC 日翻零。
//!   限额 0 = 不限。预算耗尽时该路径降级为与 mailer 关闭相同的行为
//!   （注册静默跳过 / resend 静默 / 重置请求 503），复用既有降级语义。
//! - 计数随状态持久化到 data/mailer.json（重启不清零——攻击期间重启不重开闸门）。
//!
//! 三路分账的根因：注册路径是唯一「收件人地址由攻击者任意指定」的发信路径，
//! 独立预算保证注册洪水烧不干密码重置的份额（auth-DoS 防护）。

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// 发信路径（预算分账粒度）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailPath {
    /// 注册时的验证邮件（攻击面：收件人任意指定）。
    Register,
    /// 未验证用户手动重发验证邮件。
    Resend,
    /// 密码重置邮件（仅既有账号 + 15min 冷却）。
    Reset,
}

impl MailPath {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::Resend => "resend",
            Self::Reset => "reset",
        }
    }
}

/// 分路日限额（0 = 不限）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MailLimits {
    pub register: u32,
    pub resend: u32,
    pub reset: u32,
}

impl MailLimits {
    fn get(&self, path: MailPath) -> u32 {
        match path {
            MailPath::Register => self.register,
            MailPath::Resend => self.resend,
            MailPath::Reset => self.reset,
        }
    }
}

/// 单日分路计数（sent = 预算占用；blocked = 被总闸/预算拒绝次数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DailyCounters {
    pub register: u32,
    pub resend: u32,
    pub reset: u32,
}

impl DailyCounters {
    fn get(&self, path: MailPath) -> u32 {
        match path {
            MailPath::Register => self.register,
            MailPath::Resend => self.resend,
            MailPath::Reset => self.reset,
        }
    }

    fn incr(&mut self, path: MailPath) {
        match path {
            MailPath::Register => self.register = self.register.saturating_add(1),
            MailPath::Resend => self.resend = self.resend.saturating_add(1),
            MailPath::Reset => self.reset = self.reset.saturating_add(1),
        }
    }
}

/// try_acquire 的三态结果（区分拒绝原因，供 warn 日志与错误映射）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquireOutcome {
    /// 占用成功（sent 计数 +1）。
    Allowed,
    /// 运行时总闸关闭。
    Disabled,
    /// 当日预算耗尽。
    Exhausted,
}

/// 运行时状态（内存真源 + 持久化镜像）。
#[derive(Debug, Clone)]
struct ControlState {
    enabled: bool,
    limits: MailLimits,
    /// UTC 日（"YYYY-MM-DD"），计数归属日；跨日翻零。
    day: String,
    sent: DailyCounters,
    blocked: DailyCounters,
    updated_at: OffsetDateTime,
    updated_by: Option<i64>,
}

/// admin GET /admin/mailer 与 risk 面板的快照。
#[derive(Debug, Clone, Serialize)]
pub struct MailerSnapshot {
    pub enabled: bool,
    pub limits: MailLimits,
    pub day: String,
    pub sent: DailyCounters,
    pub blocked: DailyCounters,
}

#[derive(Debug, Serialize, Deserialize)]
struct ControlFile {
    enabled: bool,
    limits: MailLimits,
    day: String,
    sent: DailyCounters,
    blocked: DailyCounters,
    updated_at: String,
    updated_by: Option<i64>,
}

/// 共享句柄（Clone 廉价，内部 Arc<RwLock>）。clock 可注入（测试跨日翻零）。
#[derive(Clone)]
pub struct MailerControl {
    inner: Arc<RwLock<ControlState>>,
    path: PathBuf,
    #[allow(clippy::type_complexity)]
    clock: Arc<dyn Fn() -> OffsetDateTime + Send + Sync>,
}

fn utc_today(t: OffsetDateTime) -> String {
    // time crate 手写格式化（仅此处用，不进 i18n 模块）。
    let (y, m, d) = (t.year(), u8::from(t.month()), t.day());
    format!("{y:04}-{m:02}-{d:02}")
}

#[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
impl MailerControl {
    /// 从配置初始值创建并尝试恢复持久化文件（文件值覆盖配置默认）。
    /// 初始 enabled=true：总闸语义是「暂停一个已启用的 mailer」；
    /// 配置级 mailer.enabled=false 由 MailerService 持有的 transport 缺失表达。
    #[must_use]
    pub fn load_or_init(limits: MailLimits, path: PathBuf) -> Self {
        Self::load_or_init_with_clock(limits, path, Arc::new(OffsetDateTime::now_utc))
    }

    #[must_use]
    pub fn load_or_init_with_clock(
        limits: MailLimits,
        path: PathBuf,
        clock: Arc<dyn Fn() -> OffsetDateTime + Send + Sync>,
    ) -> Self {
        let now = clock();
        let default_state = || ControlState {
            enabled: true,
            limits,
            day: utc_today(now),
            sent: DailyCounters::default(),
            blocked: DailyCounters::default(),
            updated_at: now,
            updated_by: None,
        };
        let state = if path.exists() {
            if let Some(f) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|c| serde_json::from_str::<ControlFile>(&c).ok())
            {
                ControlState {
                    enabled: f.enabled,
                    limits: f.limits,
                    day: f.day,
                    sent: f.sent,
                    blocked: f.blocked,
                    updated_at: OffsetDateTime::parse(&f.updated_at, &Rfc3339).unwrap_or(now),
                    updated_by: f.updated_by,
                }
            } else {
                tracing::warn!("mailer.json parse failed, using config defaults");
                default_state()
            }
        } else {
            default_state()
        };
        tracing::info!(
            enabled = state.enabled,
            day = %state.day,
            "mailer control state loaded"
        );
        Self {
            inner: Arc::new(RwLock::new(state)),
            path,
            clock,
        }
    }

    /// 原子「检查 + 占用」：跨日翻零 → 总闸/预算判定 → sent+1 → best-effort 持久化。
    /// 返回拒绝原因时 blocked 计数 +1（可见性：拒绝不静默）。
    #[must_use]
    pub fn try_acquire(&self, path: MailPath) -> AcquireOutcome {
        let today = utc_today((self.clock)());
        let outcome;
        let persist_needed;
        {
            let Ok(mut g) = self.inner.write() else {
                return AcquireOutcome::Allowed; // poison：宁可放行也不误杀（发送层还有兜底）
            };
            persist_needed = if g.day == today {
                false
            } else {
                g.day = today;
                g.sent = DailyCounters::default();
                g.blocked = DailyCounters::default();
                true
            };
            if g.enabled {
                let limit = g.limits.get(path);
                if limit != 0 && g.sent.get(path) >= limit {
                    g.blocked.incr(path);
                    outcome = AcquireOutcome::Exhausted;
                } else {
                    g.sent.incr(path);
                    outcome = AcquireOutcome::Allowed;
                }
            } else {
                g.blocked.incr(path);
                outcome = AcquireOutcome::Disabled;
            }
        }
        if persist_needed || outcome != AcquireOutcome::Allowed {
            self.persist();
        }
        outcome
    }

    /// 记一次「被预检拦截的请求」：reset 路由 503 分支调用。
    /// try_acquire 覆盖发送层的拒绝（含静默降级路径），本方法覆盖只读预检的
    /// 显式拒绝——两者合起来兑现「拒绝不静默」：任何因总闸/预算被挡的
    /// 请求都在 blocked 计数与 OTel 指标上可见。
    pub fn record_rejected(&self, path: MailPath) {
        let today = utc_today((self.clock)());
        let changed = {
            let Ok(mut g) = self.inner.write() else {
                return;
            };
            if g.day != today {
                g.day = today;
                g.sent = DailyCounters::default();
                g.blocked = DailyCounters::default();
            }
            g.blocked.incr(path);
            true
        };
        if changed {
            self.persist();
        }
    }

    /// 只读预检（不计数）：reset 路由的 503 判定 / admin 展示用。
    /// 与 try_acquire 之间无原子性（单请求内 peek→acquire 的 TOCTOU 可接受：
    /// 竞态窗口内预算耗尽时发送层返回 BudgetExhausted，走既有 best-effort 失败路径）。
    #[must_use]
    pub fn would_send(&self, path: MailPath) -> bool {
        let today = utc_today((self.clock)());
        let Ok(g) = self.inner.read() else {
            return true;
        };
        if !g.enabled {
            return false;
        }
        let limit = g.limits.get(path);
        // 跨日后计数概念上归零：按 0 判定。
        let sent = if g.day == today { g.sent.get(path) } else { 0 };
        limit == 0 || sent < limit
    }

    /// 快照（跨日则先翻零，展示与真源一致）。
    #[must_use]
    pub fn snapshot(&self) -> MailerSnapshot {
        let today = utc_today((self.clock)());
        let mut g = self
            .inner
            .write()
            .expect("mailer control lock poisoned (no panic path)");
        if g.day != today {
            g.day = today;
            g.sent = DailyCounters::default();
            g.blocked = DailyCounters::default();
        }
        MailerSnapshot {
            enabled: g.enabled,
            limits: g.limits,
            day: g.day.clone(),
            sent: g.sent,
            blocked: g.blocked,
        }
    }

    /// 运行时总闸（内存 + 持久化）。
    pub fn set_enabled(&self, enabled: bool, updated_by: Option<i64>) {
        self.mutate(updated_by, |g| g.enabled = enabled);
    }

    /// 运行时改限额。
    pub fn set_limits(&self, limits: MailLimits, updated_by: Option<i64>) {
        self.mutate(updated_by, |g| g.limits = limits);
    }

    fn mutate(&self, updated_by: Option<i64>, f: impl FnOnce(&mut ControlState)) {
        if let Ok(mut g) = self.inner.write() {
            f(&mut g);
            g.updated_at = (self.clock)();
            g.updated_by = updated_by;
        }
        self.persist();
    }

    /// best-effort 持久化（写失败仅 warn，不阻塞发信/管理路径）。
    fn persist(&self) {
        let file = {
            let Ok(g) = self.inner.read() else {
                return;
            };
            ControlFile {
                enabled: g.enabled,
                limits: g.limits,
                day: g.day.clone(),
                sent: g.sent,
                blocked: g.blocked,
                updated_at: g.updated_at.format(&Rfc3339).unwrap_or_default(),
                updated_by: g.updated_by,
            }
        };
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(&file) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.path, json) {
                    tracing::warn!(error = ?e, "failed to persist mailer.json");
                }
            },
            Err(e) => tracing::warn!(error = ?e, "mailer.json serialize failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    /// 每测试独立的可推进时钟（并行测试不共享状态；store(1) 即 +1 天）。
    fn stepping_clock() -> (
        Arc<AtomicI64>,
        Arc<dyn Fn() -> OffsetDateTime + Send + Sync>,
    ) {
        let offset = Arc::new(AtomicI64::new(0));
        let cell = offset.clone();
        let clock: Arc<dyn Fn() -> OffsetDateTime + Send + Sync> = Arc::new(move || {
            OffsetDateTime::now_utc() + time::Duration::days(cell.load(Ordering::SeqCst))
        });
        (offset, clock)
    }

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "wakewake_mailctl_test_{name}_{}",
            uuid::Uuid::new_v4()
        ))
    }

    fn limits() -> MailLimits {
        MailLimits {
            register: 2,
            resend: 1,
            reset: 1,
        }
    }

    #[test]
    fn acquire_within_limit_then_exhausted() {
        let (_offset, clock) = stepping_clock();
        let c = MailerControl::load_or_init_with_clock(limits(), tmp_path("cap"), clock);
        assert_eq!(c.try_acquire(MailPath::Register), AcquireOutcome::Allowed);
        assert_eq!(c.try_acquire(MailPath::Register), AcquireOutcome::Allowed);
        assert_eq!(c.try_acquire(MailPath::Register), AcquireOutcome::Exhausted);
        // 分路独立：reset 预算未被注册路径占用
        assert_eq!(c.try_acquire(MailPath::Reset), AcquireOutcome::Allowed);
        // exhausted 计入 blocked
        let snap = c.snapshot();
        assert_eq!(snap.blocked.register, 1);
        assert_eq!(snap.sent.register, 2);
    }

    #[test]
    fn zero_limit_means_unlimited() {
        let (_offset, clock) = stepping_clock();
        let c =
            MailerControl::load_or_init_with_clock(MailLimits::default(), tmp_path("unlim"), clock);
        for _ in 0..10 {
            assert_eq!(c.try_acquire(MailPath::Register), AcquireOutcome::Allowed);
        }
    }

    #[test]
    fn day_rollover_resets_counters() {
        let (offset, clock) = stepping_clock();
        let c = MailerControl::load_or_init_with_clock(limits(), tmp_path("roll"), clock);
        assert_eq!(c.try_acquire(MailPath::Register), AcquireOutcome::Allowed);
        assert_eq!(c.try_acquire(MailPath::Register), AcquireOutcome::Allowed);
        assert_eq!(c.try_acquire(MailPath::Register), AcquireOutcome::Exhausted);
        // 推进一天：预算翻零
        offset.store(1, Ordering::SeqCst);
        assert_eq!(c.try_acquire(MailPath::Register), AcquireOutcome::Allowed);
        let snap = c.snapshot();
        assert_eq!(snap.sent.register, 1);
    }

    #[test]
    fn runtime_disable_blocks_and_counts() {
        let (_offset, clock) = stepping_clock();
        let c = MailerControl::load_or_init_with_clock(limits(), tmp_path("off"), clock);
        c.set_enabled(false, Some(7));
        assert_eq!(c.try_acquire(MailPath::Reset), AcquireOutcome::Disabled);
        assert!(!c.would_send(MailPath::Reset));
        c.set_enabled(true, Some(7));
        assert!(c.would_send(MailPath::Reset));
    }

    #[test]
    fn record_rejected_counts_precheck_denials() {
        let (offset, clock) = stepping_clock();
        let c = MailerControl::load_or_init_with_clock(limits(), tmp_path("rej"), clock);
        // 预检 503 分支：不消耗 sent，只累计 blocked
        c.record_rejected(MailPath::Reset);
        let snap = c.snapshot();
        assert_eq!(snap.blocked.reset, 1);
        assert_eq!(
            snap.sent.reset, 0,
            "precheck denial must not consume budget"
        );
        // 跨日翻零后计数归零
        offset.store(1, Ordering::SeqCst);
        c.record_rejected(MailPath::Reset);
        let snap = c.snapshot();
        assert_eq!(snap.blocked.reset, 1, "rolled over to a fresh day");
    }

    #[test]
    fn would_send_does_not_consume() {
        let (_offset, clock) = stepping_clock();
        let c = MailerControl::load_or_init_with_clock(
            MailLimits {
                resend: 1,
                ..MailLimits::default()
            },
            tmp_path("peek"),
            clock,
        );
        assert!(c.would_send(MailPath::Resend));
        assert!(c.would_send(MailPath::Resend));
        assert_eq!(c.snapshot().sent.resend, 0, "peek must not count");
        assert_eq!(c.try_acquire(MailPath::Resend), AcquireOutcome::Allowed);
        assert!(!c.would_send(MailPath::Resend));
    }

    #[test]
    fn persists_and_restores_across_restart() {
        let path = tmp_path("persist");
        {
            let (_offset, clock) = stepping_clock();
            let c = MailerControl::load_or_init_with_clock(limits(), path.clone(), clock);
            let _ = c.try_acquire(MailPath::Register);
            c.set_enabled(false, Some(42));
        }
        let (_offset, clock) = stepping_clock();
        let c2 = MailerControl::load_or_init_with_clock(limits(), path.clone(), clock);
        let snap = c2.snapshot();
        assert!(!snap.enabled);
        assert_eq!(snap.sent.register, 1, "counters survive restart");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let path = tmp_path("corrupt");
        std::fs::write(&path, "{not json").unwrap();
        let (_offset, clock) = stepping_clock();
        let c = MailerControl::load_or_init_with_clock(limits(), path.clone(), clock);
        assert!(c.snapshot().enabled, "defaults re-enable the gate");
        let _ = std::fs::remove_file(&path);
    }
}
