//! systemd 服务安装（`wakewake-agent service install`，specs/backend/agent-distribution.md）。
//!
//! 前台运行 agent 在 ssh 断开即死；本命令把 agent 装成 systemd 服务（enable --now），
//! cloudflared 同款自安装模式。仅 Linux；Docker 路径用户不需要（`--restart unless-stopped`）。

use std::path::{Path, PathBuf};
use std::process::Command;

pub const UNIT_NAME: &str = "wakewake-agent.service";
const UNIT_DIR: &str = "/etc/systemd/system";

/// 安装输入：渲染 unit 与执行 systemctl 所需的全部路径（`install` 收集）。
pub struct InstallPlan {
    /// agent 二进制绝对路径（current_exe）。
    pub bin_path: PathBuf,
    /// 服务运行用户（SUDO_USER 或当前用户——sudo 安装时服务仍以原用户身份跑）。
    pub user: String,
    /// 该用户真实家目录（unit 里 Environment=HOME，保证 config/key.pem 定位与前台一致）。
    pub user_home: PathBuf,
    /// 显式配置文件路径（unit 里 --config 传给 agent）。
    pub config_path: PathBuf,
}

/// 渲染 systemd unit。路径含空格时按 systemd 语法加双引号。
#[must_use]
pub fn render_unit(plan: &InstallPlan) -> String {
    format!(
        "[Unit]\n\
         Description=WakeWake agent (Wake-on-LAN)\n\
         Documentation=https://github.com/jukanntenn/wakewake\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         User={user}\n\
         Environment=HOME={home}\n\
         ExecStart={bin} --config {config}\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        user = plan.user,
        home = systemd_quote(&plan.user_home),
        bin = systemd_quote(&plan.bin_path),
        config = systemd_quote(&plan.config_path),
    )
}

/// systemd ExecStart/Environment 值转义：含空白即包双引号（不含则裸路径）。
#[must_use]
fn systemd_quote(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.contains(char::is_whitespace) {
        format!("\"{s}\"")
    } else {
        s.into_owned()
    }
}

/// 执行安装：写 unit → daemon-reload → enable --now。
///
/// `bin_path` 与 `config_path` 由调用方解析（`install` 用 current_exe 与 home 目录定位），
/// 此函数只做副作用，便于上层先行校验（config 存在性、root、systemd 可用性）。
pub fn install(plan: &InstallPlan) -> anyhow::Result<()> {
    let unit_path = Path::new(UNIT_DIR).join(UNIT_NAME);
    let unit = render_unit(plan);
    std::fs::write(&unit_path, unit).map_err(|e| {
        let path = unit_path.display();
        anyhow::anyhow!("failed to write {path}: {e}")
    })?;

    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "--now", UNIT_NAME])?;
    Ok(())
}

fn systemctl(args: &[&str]) -> anyhow::Result<()> {
    let status = Command::new("systemctl")
        .args(args)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run systemctl (systemd not installed?): {e}"))?;
    if !status.success() {
        anyhow::bail!("systemctl {} failed with {status}", args.join(" "));
    }
    Ok(())
}

/// `service install` 入口：环境校验（root / systemd / config 存在）+ 路径解析 + `install`。
pub fn install_command(config_override: Option<&Path>) -> anyhow::Result<()> {
    if !cfg!(target_os = "linux") {
        anyhow::bail!(
            "service install 仅支持 Linux systemd；macOS/Windows 暂未支持（README planned），\
             或使用 Docker 路径（--restart unless-stopped）"
        );
    }

    if !euid_is_root() {
        anyhow::bail!(
            "需要 sudo：安装系统级 systemd 服务要求 root（sudo wakewake-agent service install）"
        );
    }

    // sudo 下服务仍以原用户身份跑（文件权限与前台运行一致）。
    let user = match std::env::var("SUDO_USER") {
        Ok(u) => u,
        Err(_) => current_user()?,
    };
    let user_home = user_home(&user)?;

    // config 定位：--config 显式优先；否则 <WAKEWAKE_HOME>/config.toml（WAKEWAKE_HOME 未设时
    // 为该用户家目录下的 ~/.wakewake——与前台运行/默认搜索路径一致）。
    let home_dir = config_override.map_or_else(
        || {
            std::env::var_os("WAKEWAKE_HOME")
                .map_or_else(|| user_home.join(".wakewake"), PathBuf::from)
                .join("config.toml")
        },
        std::path::Path::to_path_buf,
    );
    if !home_dir.exists() {
        anyhow::bail!(
            "未找到配置文件 {}；先在 agent 机器上运行 install.sh（或手动创建 config.toml，\
             见 backend/crates/agent/README.md）",
            home_dir.display()
        );
    }

    let bin_path = std::env::current_exe()
        .map_err(|e| anyhow::anyhow!("failed to resolve current executable: {e}"))?;

    let plan = InstallPlan {
        bin_path,
        user,
        user_home,
        config_path: home_dir,
    };
    install(&plan)?;

    println!("服务已安装并启动：systemctl status {UNIT_NAME}");
    println!("日志：journalctl -u {UNIT_NAME} -f");
    Ok(())
}

/// 当前用户名：USER > LOGNAME；都没有时报错（不静默回退 root）。
fn current_user() -> anyhow::Result<String> {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .map_err(|_| anyhow::anyhow!("无法确定当前用户（USER/LOGNAME 均未设置）"))
}

/// 用户真实家目录：getent passwd 解析（NIS/非标准家目录正确），回退 /home/<user>。
fn user_home(user: &str) -> anyhow::Result<PathBuf> {
    let output = Command::new("getent")
        .args(["passwd", user])
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run getent: {e}"))?;
    if output.status.success() {
        let line = String::from_utf8_lossy(&output.stdout);
        // passwd 格式 name:x:uid:gid:gecos:home:shell——第 6 字段是家目录。
        if let Some(home) = line.split(':').nth(5).filter(|h| !h.is_empty()) {
            return Ok(PathBuf::from(home.trim_end_matches('\n')));
        }
    }
    Ok(PathBuf::from("/home").join(user))
}

/// euid 是否为 0：解析 /proc/self/status 的 Uid: 行首字段。
fn euid_is_root() -> bool {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return false;
    };
    status
        .lines()
        .find_map(parse_uid_line)
        .is_some_and(|euid| euid == 0)
}

/// 解析单行 `Uid:\t0\t1000\t...` 的首字段（euid）。
fn parse_uid_line(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("Uid:")?;
    rest.split_whitespace().next().and_then(|f| f.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> InstallPlan {
        InstallPlan {
            bin_path: PathBuf::from("/usr/local/bin/wakewake-agent"),
            user: "alice".into(),
            user_home: PathBuf::from("/home/alice"),
            config_path: PathBuf::from("/home/alice/.wakewake/config.toml"),
        }
    }

    #[test]
    fn unit_contains_required_directives() {
        let unit = render_unit(&plan());
        for expected in [
            "User=alice",
            "Environment=HOME=/home/alice",
            "ExecStart=/usr/local/bin/wakewake-agent --config /home/alice/.wakewake/config.toml",
            "After=network-online.target",
            "Wants=network-online.target",
            "Restart=on-failure",
            "RestartSec=5",
            "WantedBy=multi-user.target",
        ] {
            assert!(
                unit.contains(expected),
                "unit missing `{expected}`:\n{unit}"
            );
        }
    }

    #[test]
    fn quote_wraps_paths_with_spaces() {
        assert_eq!(systemd_quote(Path::new("/opt/app/bin")), "/opt/app/bin");
        assert_eq!(
            systemd_quote(Path::new("/opt/my app/bin")),
            "\"/opt/my app/bin\""
        );
    }

    #[test]
    fn uid_line_parsing() {
        assert_eq!(parse_uid_line("Uid:\t0\t1000\t1000\t0"), Some(0));
        assert_eq!(parse_uid_line("Uid:\t1000\t1000\t1000\t1000"), Some(1000));
        assert_eq!(parse_uid_line("Gid:\t0\t0"), None);
        assert_eq!(parse_uid_line("Uid:\tabc"), None);
    }
}
