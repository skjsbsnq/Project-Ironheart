//! AI 诊断日志：ringbuffer + 每 7 天 per-country 摘要。
//!
//! Phase 0 验证工具：不修任何 AI 逻辑，只加观测点。
//! 每 7 天输出一行 per country：
//!   [TAG] divs=5/24 civ=10 mil=3 stock_inf=1500 mp=80k stance=Defending front[ENEMY]=2divs
//!
//! ringbuffer 保留最近 4096 行日志，可由 app 层读取展示或写文件。

use std::collections::HashMap;

use hoi4_logic::economy::EconomyState;
use hoi4_state::{CountryId, World};

use crate::ground::GroundPosture;

const RINGBUF_CAP: usize = 4096;

pub struct AiLogRingbuf {
    entries: Vec<String>,
    write_pos: usize,
    len: usize,
    total_pushed: usize,
}

impl AiLogRingbuf {
    pub fn new() -> Self {
        Self {
            entries: vec![String::new(); RINGBUF_CAP],
            write_pos: 0,
            len: 0,
            total_pushed: 0,
        }
    }

    pub fn push(&mut self, line: String) {
        self.entries[self.write_pos] = line;
        self.write_pos = (self.write_pos + 1) % RINGBUF_CAP;
        if self.len < RINGBUF_CAP {
            self.len += 1;
        }
        self.total_pushed += 1;
    }

    pub fn recent(&self, n: usize) -> Vec<&str> {
        let n = n.min(self.len);
        if n == 0 {
            return Vec::new();
        }
        if self.len < RINGBUF_CAP {
            let start = self.len.saturating_sub(n);
            self.entries[start..self.len]
                .iter()
                .map(|s| s.as_str())
                .collect()
        } else {
            let start = if self.write_pos >= n {
                self.write_pos - n
            } else {
                RINGBUF_CAP - (n - self.write_pos)
            };
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let idx = (start + i) % RINGBUF_CAP;
                out.push(self.entries[idx].as_str());
            }
            out
        }
    }

    pub fn total_pushed(&self) -> usize {
        self.total_pushed
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn entries_since(&self, prev_total: usize) -> Vec<&str> {
        let new_count = self.total_pushed.saturating_sub(prev_total);
        if new_count == 0 {
            return Vec::new();
        }
        let new_count = new_count.min(self.len);
        self.recent(new_count)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiStance {
    Peacetime,
    Defending,
    Attacking,
    MoppingUp,
}

impl std::fmt::Display for AiStance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiStance::Peacetime => write!(f, "Peace"),
            AiStance::Defending => write!(f, "Defend"),
            AiStance::Attacking => write!(f, "Attack"),
            AiStance::MoppingUp => write!(f, "MopUp"),
        }
    }
}

pub struct AiCountrySummary {
    pub tag: String,
    pub div_count: u32,
    pub div_target: u32,
    pub civ_factories: u32,
    pub mil_factories: u32,
    pub stockpile_infantry: f32,
    pub stockpile_artillery: f32,
    pub manpower: u64,
    pub stance: AiStance,
    pub fronts: Vec<(String, GroundPosture, u32)>,
}

pub fn build_country_summary(
    world: &World,
    econ: &EconomyState,
    country: CountryId,
    last_postures: &HashMap<u16, (GroundPosture, i64)>,
) -> AiCountrySummary {
    let ci = country.0 as usize;
    let tag = world
        .countries
        .tags
        .get(ci)
        .cloned()
        .unwrap_or_else(|| format!("c{}", ci));

    let div_count = world
        .divisions
        .owners
        .iter()
        .take(world.divisions.count)
        .filter(|&&o| o == country)
        .count() as u32;

    let at_war = world.countries.at_war.get(ci).copied().unwrap_or(false);
    let div_target = if at_war { 96u32 } else { 24u32 };

    let civ = 0u32;
    let mil = 0u32;
    let mp = world.manpower(country);

    let inf_stock = econ.stockpile_of(country, "infantry_equipment");
    let art_stock = econ.stockpile_of(country, "artillery_equipment");

    let has_war_with_enemy = !last_postures.is_empty();
    let any_attack = last_postures
        .values()
        .any(|(p, _)| *p == GroundPosture::Attack);

    let stance = if !at_war {
        AiStance::Peacetime
    } else if !has_war_with_enemy {
        AiStance::MoppingUp
    } else if any_attack {
        AiStance::Attacking
    } else {
        AiStance::Defending
    };

    let mut fronts = Vec::new();
    for (&enemy_id, &(posture, _)) in last_postures.iter() {
        let enemy_tag = world
            .countries
            .tags
            .get(enemy_id as usize)
            .cloned()
            .unwrap_or_else(|| format!("c{}", enemy_id));
        let divs_on_front = world
            .divisions
            .owners
            .iter()
            .take(world.divisions.count)
            .filter(|&&o| o == country)
            .count() as u32;
        fronts.push((enemy_tag, posture, divs_on_front));
    }

    AiCountrySummary {
        tag,
        div_count,
        div_target,
        civ_factories: civ,
        mil_factories: mil,
        stockpile_infantry: inf_stock,
        stockpile_artillery: art_stock,
        manpower: mp,
        stance,
        fronts,
    }
}

impl std::fmt::Display for AiCountrySummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] divs={}/{} civ={} mil={} stock_inf={:.0} stock_art={:.0} mp={}",
            self.tag,
            self.div_count,
            self.div_target,
            self.civ_factories,
            self.mil_factories,
            self.stockpile_infantry,
            self.stockpile_artillery,
            format_manpower(self.manpower),
        )?;
        write!(f, " stance={}", self.stance)?;
        for (enemy, posture, divs) in &self.fronts {
            write!(f, " front[{}]={:?}{}divs", enemy, posture, divs)?;
        }
        Ok(())
    }
}

fn format_manpower(mp: u64) -> String {
    if mp >= 1_000_000 {
        format!("{:.1}M", mp as f64 / 1_000_000.0)
    } else if mp >= 1_000 {
        format!("{:.0}k", mp as f64 / 1_000.0)
    } else {
        format!("{}", mp)
    }
}
