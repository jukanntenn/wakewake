//! 维护模式运行态（ui-ux-risk-control §8.4/§9.10/§11.D.3）。
//!
//! Settings.maintenance 是启动配置（不可变 Arc<Settings>）；
//! 此处提供运行时可变的 MaintenanceState（Arc<RwLock>），支持 POST /admin/maintenance 实时切换。
//! 持久化到 data/maintenance.json（docker volume 挂载），server 启动时恢复。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use time::OffsetDateTime;

use crate::config::MaintenanceMode;

/// 运行时维护状态（可变，与 Settings.maintenance 解耦）。
#[derive(Debug, Clone)]
pub struct MaintenanceState {
    pub enabled: bool,
    pub mode: MaintenanceMode,
    pub message: String,
    pub updated_at: OffsetDateTime,
    pub updated_by: Option<i64>,
}

/// 共享句柄（Clone 廉价，内部 Arc<RwLock>）。
#[derive(Clone)]
pub struct MaintenanceHandle {
    inner: Arc<RwLock<MaintenanceState>>,
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct MaintenanceFile {
    enabled: bool,
    mode: String,
    message: String,
    updated_at: String,
    updated_by: Option<i64>,
}

#[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
impl MaintenanceHandle {
    /// 从配置初始值创建，并尝试从持久化文件恢复（文件值覆盖配置默认值）。
    pub fn load_or_init(
        initial_enabled: bool,
        initial_mode: MaintenanceMode,
        initial_msg: &str,
        path: PathBuf,
    ) -> Self {
        let state = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<MaintenanceFile>(&content) {
                    Ok(f) => MaintenanceState {
                        enabled: f.enabled,
                        mode: MaintenanceMode::parse(&f.mode).unwrap_or(initial_mode),
                        message: f.message,
                        updated_at: parse_ts(&f.updated_at).unwrap_or_else(OffsetDateTime::now_utc),
                        updated_by: f.updated_by,
                    },
                    Err(e) => {
                        tracing::warn!(error = ?e, "maintenance.json parse failed, using config defaults");
                        MaintenanceState {
                            enabled: initial_enabled,
                            mode: initial_mode,
                            message: initial_msg.to_string(),
                            updated_at: OffsetDateTime::now_utc(),
                            updated_by: None,
                        }
                    },
                },
                Err(e) => {
                    tracing::warn!(error = ?e, "maintenance.json read failed, using config defaults");
                    MaintenanceState {
                        enabled: initial_enabled,
                        mode: initial_mode,
                        message: initial_msg.to_string(),
                        updated_at: OffsetDateTime::now_utc(),
                        updated_by: None,
                    }
                },
            }
        } else {
            MaintenanceState {
                enabled: initial_enabled,
                mode: initial_mode,
                message: initial_msg.to_string(),
                updated_at: OffsetDateTime::now_utc(),
                updated_by: None,
            }
        };

        tracing::info!(
            enabled = state.enabled,
            mode = state.mode.as_str(),
            "maintenance state loaded"
        );

        Self {
            inner: Arc::new(RwLock::new(state)),
            path,
        }
    }

    /// 读当前快照。poison 时回退到默认（禁用）——维护锁不会 poison（无 panic 路径）。
    #[must_use]
    pub fn snapshot(&self) -> MaintenanceState {
        self.inner.read().map_or_else(
            |_| MaintenanceState {
                enabled: false,
                mode: MaintenanceMode::RegistrationDisabled,
                message: String::new(),
                updated_at: OffsetDateTime::now_utc(),
                updated_by: None,
            },
            |g| g.clone(),
        )
    }

    #[must_use]
    pub fn enabled(&self) -> bool {
        self.snapshot().enabled
    }

    #[must_use]
    pub fn mode(&self) -> MaintenanceMode {
        self.snapshot().mode
    }

    #[must_use]
    pub fn message(&self) -> String {
        self.snapshot().message
    }

    /// 更新维护状态（内存 + 持久化文件）。best-effort 持久化：写失败仅 warn。
    pub fn set(
        &self,
        enabled: bool,
        mode: MaintenanceMode,
        message: String,
        updated_by: Option<i64>,
    ) -> Result<(), std::io::Error> {
        let new_state = MaintenanceState {
            enabled,
            mode,
            message: message.clone(),
            updated_at: OffsetDateTime::now_utc(),
            updated_by,
        };
        let file = MaintenanceFile {
            enabled,
            mode: mode.as_str().to_string(),
            message,
            updated_at: format_ts(new_state.updated_at),
            updated_by,
        };
        if let Err(e) = self.persist(&file) {
            tracing::warn!(error = ?e, "failed to persist maintenance.json");
        }
        if let Ok(mut g) = self.inner.write() {
            *g = new_state;
        }
        Ok(())
    }

    fn persist(&self, file: &MaintenanceFile) -> Result<(), std::io::Error> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(file).map_err(std::io::Error::other)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }
}

fn format_ts(t: OffsetDateTime) -> String {
    // RFC3339
    use time::format_description::well_known::Rfc3339;
    t.format(&Rfc3339).unwrap_or_else(|_| String::new())
}

fn parse_ts(s: &str) -> Option<OffsetDateTime> {
    use time::format_description::well_known::Rfc3339;
    OffsetDateTime::parse(s, &Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MaintenanceMode;

    #[test]
    fn mode_parse_round_trip() {
        for m in [
            MaintenanceMode::RegistrationDisabled,
            MaintenanceMode::Readonly,
            MaintenanceMode::Full,
        ] {
            assert_eq!(MaintenanceMode::parse(m.as_str()), Some(m));
        }
        assert_eq!(MaintenanceMode::parse("invalid"), None);
    }

    #[test]
    fn handle_set_updates_state() {
        let path = unique_tmp();
        let h = MaintenanceHandle::load_or_init(
            false,
            MaintenanceMode::RegistrationDisabled,
            "initial",
            path.clone(),
        );
        assert!(!h.enabled());
        h.set(true, MaintenanceMode::Full, "down".to_string(), Some(1))
            .unwrap();
        assert!(h.enabled());
        assert_eq!(h.mode(), MaintenanceMode::Full);
        assert_eq!(h.message(), "down");
        let snap = h.snapshot();
        assert_eq!(snap.updated_by, Some(1));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn handle_persists_and_restores() {
        let path = unique_tmp();
        {
            let h = MaintenanceHandle::load_or_init(
                false,
                MaintenanceMode::RegistrationDisabled,
                "",
                path.clone(),
            );
            h.set(
                true,
                MaintenanceMode::Readonly,
                "ro mode".to_string(),
                Some(42),
            )
            .unwrap();
        }
        // 重新加载：应恢复持久化的值
        let h2 = MaintenanceHandle::load_or_init(
            false,
            MaintenanceMode::RegistrationDisabled,
            "",
            path.clone(),
        );
        assert!(h2.enabled());
        assert_eq!(h2.mode(), MaintenanceMode::Readonly);
        assert_eq!(h2.message(), "ro mode");
        assert_eq!(h2.snapshot().updated_by, Some(42));
        let _ = std::fs::remove_file(&path);
    }

    fn unique_tmp() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("wakewake_maint_test_{pid}_{n}.json"))
    }

    #[test]
    fn handle_uses_config_defaults_when_file_missing() {
        let path = std::env::temp_dir().join("nonexistent_maintenance_test.json");
        let _ = std::fs::remove_file(&path);
        let h = MaintenanceHandle::load_or_init(
            false,
            MaintenanceMode::RegistrationDisabled,
            "cfg msg",
            path.clone(),
        );
        assert!(!h.enabled());
        assert_eq!(h.message(), "cfg msg");
        let _ = std::fs::remove_file(&path);
    }
}
