//! Proof-of-Work（Anubis 式 SHA-256 前导零，authentication.md §六）。
//!
//! challenge 持久化 + spent 标志一次性消费（防重放），TTL 判过期。
//! 服务端验证只需一次 hash，客户端搜索需几秒 CPU（提高批量自动化攻击经济成本）。
//!
//! difficulty 是运行时旋钮（admin-risk-controls WRFC）：POST /admin/pow 可实时调
//! （攻击时 4 → 6/7，形成「开放 → 加压 → 关闭」三档中间档），持久化 data/pow.json。
//! 每个 challenge 内嵌签发时的 difficulty，验证按 challenge 自带值——
//! 调整旋钮不影响已签发 challenge（TTL 10min 的自然过渡窗口）。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use dashmap::DashMap;
use time::OffsetDateTime;
use tokio::sync::Mutex;

use crate::config::PowSettings;

/// difficulty 运行时上限（16^10 ≈ 10^12 次哈希，浏览器不可解；防误操作设死服务）。
pub const MAX_DIFFICULTY: u8 = 10;

/// 一个 challenge（Anubis challenge.go:6-15）。
#[derive(Debug, Clone)]
pub struct Challenge {
    pub id: String,
    pub random_data: String,
    pub difficulty: u8,
    pub issued_at: OffsetDateTime,
    pub spent: bool,
}

/// difficulty 运行态（持久化镜像 + admin 元数据）。
#[derive(Debug, Clone)]
struct DifficultyState {
    difficulty: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DifficultyFile {
    difficulty: u8,
}

/// `PoW` 服务（内存 challenge 存储；单机 MVP 足够，横向扩展换 Redis）。
#[derive(Clone)]
pub struct PowService {
    challenges: Arc<DashMap<String, Challenge>>,
    difficulty: Arc<RwLock<DifficultyState>>,
    difficulty_path: Option<PathBuf>,
    ttl: Duration,
    /// 简单 GC 锁，避免并发清理竞争。
    _gc_lock: Arc<Mutex<()>>,
}

impl PowService {
    /// `difficulty_path = None`：难度固定为配置值（CLI 子命令等无 data 目录场景）。
    #[must_use]
    pub fn new(cfg: &PowSettings, difficulty_path: Option<PathBuf>) -> Self {
        // 持久化文件覆盖配置默认（旋钮跨重启保持）。
        let difficulty = if let Some(path) = &difficulty_path {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|c| serde_json::from_str::<DifficultyFile>(&c).ok())
                .map_or(cfg.difficulty, |f| f.difficulty)
        } else {
            cfg.difficulty
        };
        tracing::info!(difficulty, "pow service initialized");
        Self {
            challenges: Arc::new(DashMap::new()),
            difficulty: Arc::new(RwLock::new(DifficultyState { difficulty })),
            difficulty_path,
            ttl: humantime::parse_duration(&cfg.challenge_ttl).unwrap_or(Duration::from_mins(10)),
            _gc_lock: Arc::new(Mutex::new(())),
        }
    }

    /// 当前难度（签发/展示用）。锁 poison 回退配置值不可得——保守用 4（默认档）。
    #[must_use]
    pub fn difficulty(&self) -> u8 {
        self.difficulty.read().map_or(4, |g| g.difficulty)
    }

    /// 运行时调难度（0..=MAX_DIFFICULTY，越界拒绝），best-effort 持久化。
    pub fn set_difficulty(&self, difficulty: u8) -> Result<(), PowError> {
        if difficulty > MAX_DIFFICULTY {
            return Err(PowError::InvalidDifficulty);
        }
        if let Ok(mut g) = self.difficulty.write() {
            g.difficulty = difficulty;
        }
        if let Some(path) = &self.difficulty_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = serde_json::to_string(&DifficultyFile { difficulty })
                .map_err(std::io::Error::other)
                .and_then(|json| std::fs::write(path, json))
            {
                tracing::warn!(error = ?e, "failed to persist pow.json");
            }
        }
        tracing::info!(difficulty, "pow difficulty updated");
        Ok(())
    }

    /// 签发一个新 challenge。
    #[must_use]
    pub fn issue(&self) -> Challenge {
        use getrandom::fill;
        let mut id_buf = [0u8; 16];
        let _ = fill(&mut id_buf);
        let mut data_buf = [0u8; 32];
        let _ = fill(&mut data_buf);

        let challenge = Challenge {
            id: hex::encode(id_buf),
            random_data: hex::encode(data_buf),
            difficulty: self.difficulty(),
            issued_at: OffsetDateTime::now_utc(),
            spent: false,
        };
        self.challenges
            .insert(challenge.id.clone(), challenge.clone());
        challenge
    }

    /// 验证 PoW：challenge 存在且未 spent 且未过期，response = sha256(challenge+nonce) 前 N 位为 '0'。
    /// 通过则标记 spent（一次性）。
    pub fn verify(&self, challenge_id: &str, nonce: &str) -> Result<(), PowError> {
        use sha2::Digest;

        // 先读校验（不可变 Ref），通过后再用 get_mut 标记 spent（避免死锁）。
        let (random_data, difficulty, issued_at, spent) = {
            let entry = self
                .challenges
                .get(challenge_id)
                .ok_or(PowError::NotFound)?;
            (
                entry.random_data.clone(),
                entry.difficulty,
                entry.issued_at,
                entry.spent,
            )
        };
        if spent {
            return Err(PowError::AlreadySpent);
        }
        let now = OffsetDateTime::now_utc();
        if now - issued_at > self.ttl {
            return Err(PowError::Expired);
        }

        // response = sha256(random_data + nonce)，验证前 difficulty 位为 '0'
        let mut hasher = sha2::Sha256::new();
        hasher.update(random_data.as_bytes());
        hasher.update(nonce.as_bytes());
        let sum = hasher.finalize();
        let hex_str = hex::encode(sum);

        if hex_str.len() < difficulty as usize
            || !hex_str.as_bytes()[..difficulty as usize]
                .iter()
                .all(|c| *c == b'0')
        {
            return Err(PowError::InvalidProof);
        }
        // 标记 spent（防重放）
        if let Some(mut entry) = self.challenges.get_mut(challenge_id) {
            entry.spent = true;
        }
        Ok(())
    }

    /// 清理过期 challenge（定时任务调用）。
    pub async fn gc(&self) {
        let _g = self._gc_lock.lock().await;
        let now = OffsetDateTime::now_utc();
        let to_remove: Vec<String> = self
            .challenges
            .iter()
            .filter(|e| now - e.issued_at > self.ttl || e.spent)
            .map(|e| e.id.clone())
            .collect();
        for id in to_remove {
            self.challenges.remove(&id);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PowError {
    #[error("challenge not found")]
    NotFound,
    #[error("challenge already spent")]
    AlreadySpent,
    #[error("challenge expired")]
    Expired,
    #[error("invalid proof of work")]
    InvalidProof,
    #[error("difficulty out of range")]
    InvalidDifficulty,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> PowSettings {
        PowSettings {
            difficulty: 4,
            challenge_ttl: "10m".into(),
        }
    }

    fn tmp_path() -> PathBuf {
        std::env::temp_dir().join(format!("wakewake_pow_test_{}.json", uuid::Uuid::new_v4()))
    }

    #[test]
    fn issue_uses_current_difficulty_and_set_updates() {
        let s = PowService::new(&cfg(), None);
        assert_eq!(s.difficulty(), 4);
        assert_eq!(s.issue().difficulty, 4);

        s.set_difficulty(7).unwrap();
        assert_eq!(s.difficulty(), 7);
        assert_eq!(s.issue().difficulty, 7, "new challenges use new difficulty");
    }

    #[test]
    fn set_difficulty_rejects_out_of_range() {
        let s = PowService::new(&cfg(), None);
        assert!(s.set_difficulty(MAX_DIFFICULTY).is_ok());
        assert!(matches!(
            s.set_difficulty(MAX_DIFFICULTY + 1),
            Err(PowError::InvalidDifficulty)
        ));
    }

    #[test]
    fn difficulty_persists_and_restores() {
        let path = tmp_path();
        {
            let s = PowService::new(&cfg(), Some(path.clone()));
            s.set_difficulty(6).unwrap();
        }
        let s2 = PowService::new(&cfg(), Some(path.clone()));
        assert_eq!(s2.difficulty(), 6, "runtime knob survives restart");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn verify_uses_challenge_embedded_difficulty() {
        // 已签发 challenge 按其自带 difficulty 验证：旋钮上调不溯及既往。
        let s = PowService::new(&cfg(), None);
        let c = s.issue(); // difficulty 4
        s.set_difficulty(8).unwrap();
        // 暴力搜 4 前导零（16^4 次内必中，随机数据下期望 ~6.5 万次）
        use sha2::Digest;
        let mut nonce: u64 = 0;
        loop {
            let mut h = sha2::Sha256::new();
            h.update(c.random_data.as_bytes());
            h.update(nonce.to_string().as_bytes());
            if hex::encode(h.finalize()).starts_with("0000") {
                break;
            }
            nonce += 1;
        }
        assert!(s.verify(&c.id, &nonce.to_string()).is_ok());
    }
}
