//! 纯内核：feature-based domain 模型 + 业务规则常量。
//!
//! 零 IO，零项目内依赖，不 import sqlx/axum（onion-architecture 强制）。
//! 时间戳用 `time::OffsetDateTime（§0.5：sqlx` 原生支持，JSON 需 rfc3339 serde）。

pub mod agent;
pub mod command;
pub mod device;
pub mod integration;
pub mod sync;
pub mod user;
pub mod wake;

// ---- 业务规则常量（database-design.md / api-design.md）----

/// 每用户设备配额上限（代码层 FOR UPDATE 校验）。
/// 系统限制：每个用户最多 2 台设备（e2e 配额测试按此断言）。
pub const MAX_DEVICES_PER_USER: i64 = 2;
/// 每用户 agent 配额上限（1:1 模型，代码层校验；DB 层 1:N 预留）。
pub const MAX_AGENTS_PER_USER: i64 = 1;

/// bcrypt cost（显式传 `10；DEFAULT_COST=12` 太重，§0.2）。
pub const BCRYPT_COST: u32 = 10;
/// 密码最小长度（NIST 800-63B：长度 > 复杂度）。
pub const PASSWORD_MIN_LEN: usize = 8;
/// 密码最大长度（bcrypt 算法上限，字节级）。
pub const PASSWORD_MAX_LEN: usize = 72;

/// pairing code 长度（16 hex 字符）。
pub const PAIRING_CODE_LEN: usize = 16;

/// 命令内存 TTL（60s 超时后 expired）。
pub const COMMAND_TTL_SECS: u64 = 60;
