//! 事件系统：定义 + 调度 + 触发。

use clausewitz_parser::Block;

/// 事件定义
#[derive(Debug, Clone)]
pub struct EventDef {
    pub id: String,
    pub title: String,
    pub desc: String,
    pub is_triggered_only: bool,
    pub fire_only_once: bool,
    pub hidden: bool,
    /// trigger block（原始 AST；求值时用 TriggerRegistry）
    pub trigger: Option<Block>,
    /// mean_time_to_happen（天）
    pub mtth: Option<MeanTimeToHappen>,
    /// immediate effect block
    pub immediate: Option<Block>,
    /// 选项列表
    pub options: Vec<EventOption>,
}

/// MTTH
#[derive(Debug, Clone)]
pub struct MeanTimeToHappen {
    pub days: f32,
}

/// 事件选项
#[derive(Debug, Clone)]
pub struct EventOption {
    pub name: String,
    /// 选项的 trigger（可选；不满足则灰显）
    pub trigger: Option<Block>,
    /// 选项的 effect block
    pub effect: Option<Block>,
    /// AI 选择权重
    pub ai_chance: f32,
}

/// 事件调度器：管理已注册事件 + 待触发队列。
#[derive(Debug, Clone, Default)]
pub struct EventScheduler {
    /// 所有已注册事件定义
    pub events: Vec<EventDef>,
    /// 待触发队列：(event_id, target_country_idx, fire_at_hour)
    pub pending: Vec<(String, u16, u64)>,
    /// 已触发过的 fire_only_once 事件
    pub fired_once: std::collections::HashSet<String>,
}

impl EventScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, event: EventDef) {
        self.events.push(event);
    }

    /// 排入一个事件（延迟触发）
    pub fn queue(&mut self, event_id: &str, country_idx: u16, fire_at_hour: u64) {
        self.pending
            .push((event_id.to_owned(), country_idx, fire_at_hour));
    }

    /// 立即触发（不排队）
    pub fn fire_immediate(&mut self, event_id: &str) -> Option<&EventDef> {
        let ev = self.events.iter().find(|e| e.id == event_id)?;
        if ev.fire_only_once {
            if self.fired_once.contains(event_id) {
                return None;
            }
            self.fired_once.insert(event_id.to_owned());
        }
        Some(ev)
    }

    /// 取出到期的事件（hour <= current_hour）
    pub fn drain_due(&mut self, current_hour: u64) -> Vec<(String, u16)> {
        let mut due = Vec::new();
        self.pending.retain(|(id, ci, h)| {
            if *h <= current_hour {
                due.push((id.clone(), *ci));
                false
            } else {
                true
            }
        });
        due
    }

    pub fn find(&self, id: &str) -> Option<&EventDef> {
        self.events.iter().find(|e| e.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_queue_and_drain() {
        let mut s = EventScheduler::new();
        s.queue("ev1", 0, 100);
        s.queue("ev2", 1, 200);
        let due = s.drain_due(150);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0, "ev1");
        assert_eq!(s.pending.len(), 1);
    }

    #[test]
    fn fire_only_once() {
        let mut s = EventScheduler::new();
        s.register(EventDef {
            id: "x".into(),
            title: String::new(),
            desc: String::new(),
            is_triggered_only: true,
            fire_only_once: true,
            hidden: false,
            trigger: None,
            mtth: None,
            immediate: None,
            options: vec![],
        });
        assert!(s.fire_immediate("x").is_some());
        assert!(s.fire_immediate("x").is_none());
    }
}
