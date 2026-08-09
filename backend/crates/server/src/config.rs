//! 后端配置（config-rs + clap，configuration.md）。
//!
//! 三层覆盖模型（优先级从高到低）：CLI 参数 > 环境变量 > TOML 文件 > 内置默认值。
//! 注意 config-rs 的 `set_default/set_override` 返回 Result（需 `?`，源码验证修正）。

use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use config::{Config, Environment, File};
use garde::Validate;
use serde::Deserialize;

/// CLI 参数（最高优先级）。clap derive + env feature。
///
/// 子命令体系（Django manage.py 模式）：不传子命令 = `serve`（向后兼容）。
#[derive(Parser, Debug, Clone)]
#[command(name = "wakewake-server", about = "WakeWake backend server")]
pub struct Cli {
    /// 配置文件路径（指定时必须存在）
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// 顶层子命令。
#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// 启动 HTTP server（默认行为，不传子命令时等同）。
    Serve {
        /// 监听地址（覆盖配置文件）
        #[arg(long)]
        host: Option<String>,
        /// 监听端口（覆盖配置文件）
        #[arg(long)]
        port: Option<u16>,
    },
    /// 打印版本号。
    Version,
    /// 管理员账户管理。
    Admin {
        #[command(subcommand)]
        action: AdminCommand,
    },
}

/// `admin` 二级子命令。
#[derive(Subcommand, Debug, Clone)]
pub enum AdminCommand {
    /// 创建新 admin 用户（交互式输入密码）。
    Create { email: String },
    /// 提升现有用户为 admin。
    Promote { email: String },
    /// 降级 admin（至少保留 1 个 admin）。
    Demote { email: String },
    /// 列出所有 admin 用户。
    List,
}

/// Server 配置（garde 校验语义约束，加载后调用 validate）。
#[derive(Debug, Clone, Deserialize, garde::Validate)]
#[garde(allow_unvalidated)]
pub struct Settings {
    #[garde(dive)]
    pub app: AppSettings,
    #[garde(skip)]
    pub server: ServerSettings,
    #[garde(skip)]
    pub database: DatabaseSettings,
    #[garde(dive)]
    pub jwt: JwtSettings,
    #[garde(skip)]
    pub password_reset: PasswordResetSettings,
    #[garde(skip)]
    pub pow: PowSettings,
    #[garde(skip)]
    pub mailer: MailerSettings,
    #[garde(skip)]
    pub rate_limit: RateLimitSettings,
    #[garde(skip)]
    pub security: SecuritySettings,
    #[garde(skip)]
    pub log: LogSettings,
}

/// 应用级配置（与 server 监听解耦的对外公网地址）。
/// 用于拼接邮件验证/重置链接、agent 文档示例地址等——必须是可信的固定绝对 URL，
/// 不从请求 Host 推导（反代场景 Host 可伪造 → 钓鱼/窃取 token）。
#[derive(Debug, Clone, Deserialize, garde::Validate)]
#[garde(allow_unvalidated)]
pub struct AppSettings {
    /// 应用对外公网地址（含 scheme+host[+port]，如 `https://wakewake.app`）。
    /// 必填：缺失则启动失败（fail-fast），避免发出残缺的相对路径邮件链接。
    #[garde(url)]
    pub public_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    /// `PostgreSQL` 连接串，无默认（生产必填）。
    pub dsn: String,
}

#[derive(Debug, Clone, Deserialize, garde::Validate)]
#[garde(allow_unvalidated)]
pub struct JwtSettings {
    /// access token HMAC 密钥（≥32 字节）。
    #[garde(length(min = 32))]
    pub signing_key: String,
    /// refresh token HMAC 密钥（与 access 独立，≥32 字节）。
    #[garde(length(min = 32))]
    pub refresh_signing_key: String,
    /// access TTL，humantime 字符串（"15m"）。
    pub access_expire: String,
    /// refresh TTL（"720h" = 30d）。
    pub refresh_expire: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PasswordResetSettings {
    /// HMAC 密钥（无状态 reset token）。
    pub secret: String,
    /// token TTL（"1h"）。
    pub expire: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PowSettings {
    /// 前导零个数（默认 4）。
    pub difficulty: u8,
    /// challenge 有效期（"10m"）。
    pub challenge_ttl: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MailerSettings {
    pub enabled: bool,
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub from_address: Option<String>,
    pub from_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogSettings {
    pub level: String,
    /// 日志目录（tracing-appender daily rolling 写入此目录）。
    pub dir: String,
}

/// 限流配置（e2e.md §2.5/§4.6 + load.md §9.2）。
///
/// `disabled=true` 仅旁路 axum-governor 速率限制（per-IP/per-user 频率），
/// **不旁路业务配额**（`MAX_DEVICES_PER_USER` 等硬编码 const）。
/// 用于 E2E/压测环境避免速率限制干扰（登录失败锁定、5k SSE 并发等）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RateLimitSettings {
    /// true = 跳过挂载所有 governor layer（auth/password-reset/global）。
    #[serde(default)]
    pub disabled: bool,
}

/// 安全/bootstrap 配置：默认 admin 账户（启动时幂等创建）。
///
/// 新部署开箱即有一个可用 admin，类比 Grafana 默认 admin/admin。
/// 仅当账户不存在时创建（已存在则跳过，不覆盖密码/状态）。生产务必改默认值。
#[derive(Debug, Clone, Deserialize)]
pub struct SecuritySettings {
    /// 默认 admin 邮箱。
    #[serde(default = "default_bootstrap_admin_email")]
    pub bootstrap_admin_email: String,
    /// 默认 admin 密码（明文，启动时 bcrypt 哈希入库）。
    #[serde(default = "default_bootstrap_admin_password")]
    pub bootstrap_admin_password: String,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            bootstrap_admin_email: default_bootstrap_admin_email(),
            bootstrap_admin_password: default_bootstrap_admin_password(),
        }
    }
}

fn default_bootstrap_admin_email() -> String {
    "admin@wakewake.local".into()
}

fn default_bootstrap_admin_password() -> String {
    "wakewake123".into()
}

impl Settings {
    /// 加载并校验配置（fail-fast：校验失败启动失败）。
    pub fn load(cli: &Cli) -> anyhow::Result<Self> {
        let mut builder = Config::builder()
            // 1. 内置默认值（set_default 返回 Result，需 ?）
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080i64)?
            .set_default("jwt.access_expire", "15m")?
            .set_default("jwt.refresh_expire", "720h")?
            .set_default("password_reset.expire", "1h")?
            .set_default("pow.difficulty", 4i64)?
            .set_default("pow.challenge_ttl", "10m")?
            .set_default("mailer.enabled", false)?
            .set_default("mailer.smtp_port", 587i64)?
            .set_default("mailer.from_name", "WakeWake")?
            .set_default("rate_limit.disabled", false)?
            .set_default("security.bootstrap_admin_email", "admin@wakewake.local")?
            .set_default("security.bootstrap_admin_password", "wakewake123")?
            .set_default("log.level", "info")?
            .set_default("log.dir", "data/logs")?;

        // 2. TOML 文件（可选）
        builder = if let Some(path) = &cli.config {
            builder.add_source(File::with_name(&path.to_string_lossy()).required(true))
        } else {
            builder.add_source(File::with_name("config").required(false))
        };

        // 3. 环境变量（WAKEWAKE_ 前缀，__ 嵌套分隔）
        builder = builder.add_source(
            Environment::with_prefix("WAKEWAKE")
                .prefix_separator("_")
                .separator("__")
                .try_parsing(true),
        );

        // 4. CLI 参数最高优先级（set_override_option：None 不覆盖）。
        // host/port 来自 `serve` 子命令（向后兼容：不传子命令时 None）。
        let (serve_host, serve_port) = match &cli.command {
            Some(Command::Serve { host, port }) => (host.clone(), *port),
            _ => (None, None),
        };
        builder = builder
            .set_override_option("server.host", serve_host)?
            .set_override_option("server.port", serve_port.map(i64::from))?;

        let config = builder.build()?;
        let settings: Self = config.try_deserialize()?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn access_ttl(&self) -> anyhow::Result<Duration> {
        self.jwt.access_ttl()
    }

    pub fn refresh_ttl(&self) -> anyhow::Result<Duration> {
        self.jwt.refresh_ttl()
    }

    /// 日志目录（observability 文件轮转根）。绝对或相对当前工作目录。
    #[must_use]
    pub fn log_dir(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.log.dir)
    }
}

impl JwtSettings {
    pub fn access_ttl(&self) -> anyhow::Result<Duration> {
        Ok(humantime::parse_duration(&self.access_expire)?)
    }

    pub fn refresh_ttl(&self) -> anyhow::Result<Duration> {
        Ok(humantime::parse_duration(&self.refresh_expire)?)
    }
}
