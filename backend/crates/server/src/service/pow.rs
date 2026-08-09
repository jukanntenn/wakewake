//! Proof-of-Work（Anubis 式 SHA-256 前导零，authentication.md §六）。
//!
//! challenge 持久化 + spent 标志一次性消费（防重放），TTL 判过期。
//! 服务端验证只需一次 hash，客户端搜索需几秒 CPU（提高批量自动化攻击经济成本）。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use time::OffsetDateTime;
use tokio::sync::Mutex;

use crate::config::PowSettings;

/// 一个 challenge（Anubis challenge.go:6-15）。
#[derive(Debug, Clone)]
pub struct Challenge {
    pub id: String,
    pub random_data: String,
    pub difficulty: u8,
    pub issued_at: OffsetDateTime,
    pub spent: bool,
}

/// `PoW` 服务（内存 challenge 存储；单机 MVP 足够，横向扩展换 Redis）。
#[derive(Clone)]
pub struct PowService {
    challenges: Arc<DashMap<String, Challenge>>,
    difficulty: u8,
    ttl: Duration,
    /// 简单 GC 锁，避免并发清理竞争。
    _gc_lock: Arc<Mutex<()>>,
}

impl PowService {
    #[must_use]
    pub fn new(cfg: &PowSettings) -> Self {
        Self {
            challenges: Arc::new(DashMap::new()),
            difficulty: cfg.difficulty,
            ttl: humantime::parse_duration(&cfg.challenge_ttl).unwrap_or(Duration::from_mins(10)),
            _gc_lock: Arc::new(Mutex::new(())),
        }
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
            difficulty: self.difficulty,
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
}

// 避免 HashMap 未用导入。
#[allow(dead_code)]
fn _unused() -> HashMap<String, String> {
    HashMap::new()
}
