//! 决议系统：实例状态 + 目录。
//!
//! `DecisionDef`（决议定义）在 `hoi4_data::politics` 中定义，由 `GameData::load` 加载。
//! 本模块只管理运行时实例状态（某国对某决议的倒计时/冷却/已完成）。

/// 决议实例（某国对某决议的运行时状态）
#[derive(Debug, Clone)]
pub struct DecisionInstance {
    pub decision_id: String,
    pub country_idx: u16,
    /// 是否已激活（正在倒计时）
    pub active: bool,
    /// 剩余天数（days_remove 倒计时）
    pub days_remaining: u32,
    /// 冷却中（不可再次执行）
    pub on_cooldown: bool,
    pub cooldown_remaining: u32,
}

/// 决议目录
#[derive(Debug, Clone, Default)]
pub struct DecisionCatalog {
    pub instances: Vec<DecisionInstance>,
}

impl DecisionCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// 激活一个决议（扣 PP 由调用方处理）
    pub fn activate(&mut self, decision_id: &str, country_idx: u16, days_remove: u32) {
        self.instances.push(DecisionInstance {
            decision_id: decision_id.to_owned(),
            country_idx,
            active: true,
            days_remaining: days_remove,
            on_cooldown: false,
            cooldown_remaining: 0,
        });
    }

    /// 每日 tick：倒计时 + 到期收集
    pub fn tick_daily(&mut self) -> Vec<(String, u16)> {
        let mut completed = Vec::new();
        for inst in self.instances.iter_mut() {
            if !inst.active {
                continue;
            }
            if inst.days_remaining > 0 {
                inst.days_remaining -= 1;
            }
            if inst.days_remaining == 0 {
                inst.active = false;
                completed.push((inst.decision_id.clone(), inst.country_idx));
            }
        }
        completed
    }

    /// 清理已完成的实例
    pub fn purge_completed(&mut self) {
        self.instances.retain(|i| i.active || i.on_cooldown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_and_tick() {
        let mut cat = DecisionCatalog::new();
        cat.activate("test_dec", 0, 3);
        assert_eq!(cat.instances.len(), 1);
        assert!(cat.instances[0].active);

        let c1 = cat.tick_daily();
        assert!(c1.is_empty());
        assert_eq!(cat.instances[0].days_remaining, 2);

        let c2 = cat.tick_daily();
        assert!(c2.is_empty());
        let c3 = cat.tick_daily();
        assert_eq!(c3.len(), 1);
        assert_eq!(c3[0].0, "test_dec");
        assert!(!cat.instances[0].active);
    }
}
