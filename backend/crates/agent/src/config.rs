//! Agent 配置（config-rs + clap，configuration.md §4 Agent schema）。
//!
//! 三层覆盖：CLI 参数 > 环境变量 > TOML 文件 > 内置默认值。
//! 文件搜索：~/.wakewake/config.toml 或 ./wakewake.toml 或 --config 指定。
//! `env：WAKEWAKE_SERVER_URL、WAKEWAKE_PAIRING_CODE`。

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use config::{Config, Environment, File};
use serde::Deserialize;

/// CLI 参数（最高优先级，覆盖配置文件与环境变量）。
///
/// 完整配置方式（优先级高→低）：
/// 1. 本结构体定义的 CLI 参数
/// 2. 环境变量：WAKEWAKE_SERVER_URL / WAKEWAKE_PAIRING_CODE / WAKEWAKE_HOME
/// 3. 配置文件（搜索 $WAKEWAKE_HOME/config.toml → ./wakewake.toml → --config 指定）
///
/// 详见 config.example.toml 或 README.md。
#[derive(Parser, Debug, Clone)]
#[command(
    name = "wakewake-agent",
    about = "WakeWake agent (SSE client + WoL + Bemfa)"
)]
pub struct Cli {
    /// 配置文件路径（指定时必须存在；跳过默认搜索路径）。
    ///
    /// 不指定时按顺序搜索：$WAKEWAKE_HOME/config.toml → ./wakewake.toml。
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Server URL（如 https://wakewake.app）。
    ///
    /// 也可通过配置文件 `server_url` 或环境变量 `WAKEWAKE_SERVER_URL` 提供。
    #[arg(long)]
    pub server: Option<String>,
    /// Pairing code（16 位十六进制，从 server agents 页获取）。
    ///
    /// 也可通过配置文件 `pairing_code` 或环境变量 `WAKEWAKE_PAIRING_CODE` 提供。
    #[arg(long)]
    pub pairing_code: Option<String>,
}

/// Agent 配置。
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub server_url: String,
    pub pairing_code: String,
    /// Agent home 目录（api-design.md §2.12）。存 config.toml + key.pem。
    /// 默认 ~/.wakewake；env `WAKEWAKE_HOME` 覆盖；容器内 /data（卷 `agent_data:/data`）。
    #[serde(default = "default_home_dir")]
    pub home_dir: PathBuf,
    #[serde(default)]
    pub wol: WolSettings,
    #[serde(default)]
    pub log: LogSettings,
}

/// 默认 home `目录：$WAKEWAKE_HOME` > ~/.wakewake > ./.wakewake。
fn default_home_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("WAKEWAKE_HOME") {
        return PathBuf::from(home);
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".wakewake");
    }
    PathBuf::from(".wakewake")
}

#[derive(Debug, Clone, Deserialize)]
pub struct WolSettings {
    pub broadcast_addr: String,
    pub packet_count: u32,
    pub packet_delay_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogSettings {
    pub level: String,
    pub format: String,
    /// 日志目录（tracing-appender daily rolling 写入）。默认 `<home_dir>/logs`。
    #[serde(default = "default_log_dir")]
    pub dir: Option<String>,
}

impl Default for WolSettings {
    fn default() -> Self {
        Self {
            broadcast_addr: "255.255.255.255:9".to_string(),
            packet_count: 3,
            packet_delay_ms: 50,
        }
    }
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            // 默认 pretty：agent 通常在用户终端/宿主机交互式运行，pretty 输出人类可读
            // （验收时直接看 "state applied"、"command executed" 文本行而非 JSON）。
            // 生产/容器化场景可在 config.toml 设 format="json" 切回结构化日志。
            format: "pretty".to_string(),
            dir: None,
        }
    }
}

/// 默认日志目录：`<home_dir>/logs`（None 时由 `init_tracing` 拼接）。
fn default_log_dir() -> Option<String> {
    None
}

impl WolSettings {
    pub fn broadcast_socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        self.broadcast_addr.parse()
    }
}

impl Settings {
    /// 加载配置。搜索 ~/.wakewake/config.toml → ./wakewake.toml → --config。
    pub fn load(cli: &Cli) -> anyhow::Result<Self> {
        let mut builder = Config::builder()
            .set_default("home_dir", default_home_dir().to_string_lossy().as_ref())?
            .set_default("wol.broadcast_addr", "255.255.255.255:9")?
            .set_default("wol.packet_count", 3i64)?
            .set_default("wol.packet_delay_ms", 50i64)?
            .set_default("log.level", "info")?
            .set_default("log.format", "pretty")?;

        // --config 指定的必须存在；否则搜默认路径
        builder = if let Some(path) = &cli.config {
            builder.add_source(File::with_name(&path.to_string_lossy()).required(true))
        } else {
            builder
                .add_source(
                    File::with_name(&config_dir().join("config").to_string_lossy()).required(false),
                )
                .add_source(File::with_name("wakewake").required(false))
        };

        // env：WAKEWAKE_SERVER_URL、WAKEWAKE_PAIRING_CODE
        builder = builder.add_source(
            Environment::with_prefix("WAKEWAKE")
                .prefix_separator("_")
                .separator("__")
                .try_parsing(true),
        );

        // CLI 最高优先级
        builder = builder
            .set_override_option("server_url", cli.server.clone())?
            .set_override_option("pairing_code", cli.pairing_code.clone())?;

        let config = builder.build()?;
        Ok(config.try_deserialize()?)
    }

    /// 日志目录：`log.dir` 显式配置优先；否则 `<home_dir>/logs`。
    #[must_use]
    pub fn log_dir(&self) -> PathBuf {
        if let Some(dir) = &self.log.dir {
            return PathBuf::from(dir);
        }
        self.home_dir.join("logs")
    }
}

/// `配置文件搜索目录（home_dir，XDG` 风格）。
/// 复用 `default_home_dir（WAKEWAKE_HOME` > ~/.wakewake > ./.wakewake）。
fn config_dir() -> PathBuf {
    default_home_dir()
}
