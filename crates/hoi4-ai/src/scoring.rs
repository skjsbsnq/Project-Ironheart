//! 评分基础设施：`Scored<T>` + `Scorer` 辅助器。

/// 带评分和调试信息的候选项。
#[derive(Debug, Clone)]
pub struct Scored<T> {
    pub value: T,
    pub score: f32,
    pub reason: String,
}

impl<T> Scored<T> {
    pub fn new(value: T, score: f32, reason: impl Into<String>) -> Self {
        Self {
            value,
            score,
            reason: reason.into(),
        }
    }
}

/// 确定性辅助器：基于 seed 的 hash + jitter。
pub struct Scorer {
    pub seed: u64,
}

impl Scorer {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// 简易 deterministic hash（基于 FNV-1a 变体）
    pub fn hash_u64(&self, key: &str) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325 ^ self.seed;
        for b in key.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    /// 对评分添加 ±5% 确定性噪声以打破平局。
    pub fn jitter(&self, score: f32, key: &str) -> f32 {
        let h = self.hash_u64(key);
        // 映射到 [-0.05, +0.05]
        let noise = ((h % 1000) as f32 / 1000.0 - 0.5) * 0.1;
        score * (1.0 + noise)
    }
}

/// 对 scored 列表排序：分数高的在前。
pub fn rank<T>(mut items: Vec<Scored<T>>) -> Vec<Scored<T>> {
    items.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    items
}
