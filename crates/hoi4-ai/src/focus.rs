//! 国策选择：评估可用国策，选出最高分的执行。

use hoi4_data::GameData;
use hoi4_state::{CountryId, World};

use crate::constants::*;
use crate::scoring::{rank, Scored, Scorer};

/// AI 做出的国策决策。
#[derive(Debug, Clone)]
pub struct FocusDecision {
    pub focus_id: String,
}

/// 单个国策的评分详情。
#[derive(Debug, Clone)]
pub struct FocusScore {
    pub focus_id: String,
    pub score: f32,
    pub reason: String,
}

/// 评估该国所有可用国策，返回按分数降序排列的列表。
///
/// 评分逻辑：
/// 1. 枚举该国 focus tree 中所有 focus
/// 2. 过滤掉已完成、互斥已选、前置未满足、已有 focus 进行中的
/// 3. 对每个可用 focus 的 `completion_reward` block 做关键词启发式评分
pub fn evaluate_focuses(world: &World, country: CountryId) -> Vec<Scored<FocusDecision>> {
    let ci = country.0 as usize;
    if ci >= world.countries.count {
        return Vec::new();
    }

    // 已有正在进行的 focus，不评估
    if world.countries.current_focus[ci].is_some() {
        return Vec::new();
    }

    let tag = &world.countries.tags[ci];

    // 找到该国的 focus tree
    let tree = match find_focus_tree(world, tag) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let completed = &world.countries.completed_focuses[ci];
    let scorer = Scorer::new(world.random_seed.wrapping_add(ci as u64));
    let mut results = Vec::new();

    for (fid, focus) in &tree.focuses {
        // 已完成
        if completed.contains(fid) {
            continue;
        }
        // 互斥
        if focus
            .mutually_exclusive
            .iter()
            .any(|me| completed.contains(me))
        {
            continue;
        }
        // 前置
        if !focus.prereqs_met(completed) {
            continue;
        }

        // 评分
        let mut score = FOCUS_BASE_SCORE;
        let mut reasons = vec!["base".to_string()];

        if let Some(ref reward) = focus.completion_reward {
            let (s, r) = score_block(reward);
            score += s;
            reasons.extend(r);
        }

        score = scorer.jitter(score, fid);
        results.push(Scored::new(
            FocusDecision {
                focus_id: fid.clone(),
            },
            score,
            reasons.join(", "),
        ));
    }

    rank(results)
}

/// 执行国策决策：调用 `politics::assign_focus`。
pub fn apply_focus_decision(
    world: &mut World,
    country: CountryId,
    decision: &FocusDecision,
    data: &GameData,
) {
    let _ = hoi4_logic::politics::assign_focus(world, country, &decision.focus_id, data);
}

/// 查找该国的 focus tree（先按 country_tag 匹配，再 fallback 到 default tree）。
fn find_focus_tree<'a>(world: &'a World, tag: &str) -> Option<&'a hoi4_data::FocusTree> {
    // 优先找该国专属 tree
    for tree in world.data.focus_trees.values() {
        if tree.country_tag.as_deref() == Some(tag) {
            return Some(tree);
        }
    }
    // fallback: default tree
    for tree in world.data.focus_trees.values() {
        if tree.default {
            return Some(tree);
        }
    }
    None
}

/// 从 Value 中提取数值（Integer 或 Float → f64）。
fn value_to_f64(v: &clausewitz_parser::parser::Value) -> Option<f64> {
    match v {
        clausewitz_parser::parser::Value::Integer(n) => Some(*n as f64),
        clausewitz_parser::parser::Value::Float(f) => Some(*f),
        _ => None,
    }
}

/// 对 completion_reward block 做关键词启发式评分。
fn score_block(block: &clausewitz_parser::parser::Block) -> (f32, Vec<String>) {
    let mut score = 0.0;
    let mut reasons = Vec::new();

    for entry in &block.entries {
        let key = entry.key.as_str();
        if let Some(val) = value_to_f64(&entry.value) {
            let v = val as f32;
            match key {
                "add_political_power" => {
                    let bonus = FOCUS_PP_BONUS * (v / 100.0).min(3.0);
                    score += bonus;
                    reasons.push(format!("PP+{:.0}", v));
                }
                "add_stability" => {
                    score += FOCUS_STABILITY_BONUS * (v / 0.1).min(3.0);
                    reasons.push(format!("stab+{:.2}", v));
                }
                "add_war_support" => {
                    score += FOCUS_STABILITY_BONUS * (v / 0.1).min(3.0);
                    reasons.push(format!("ws+{:.2}", v));
                }
                "army_experience" | "navy_experience" | "air_experience" => {
                    score += FOCUS_XP_BONUS * (v / 50.0).min(2.0);
                    reasons.push(format!("{}+{:.0}", key, v));
                }
                "add_manpower" => {
                    score += FOCUS_MANPOWER_BONUS * (v / 100000.0).min(2.0);
                    reasons.push(format!("mp+{:.0}", v));
                }
                _ => {}
            }
        } else {
            // 非数值型 effect
            match key {
                "add_ideas" | "add_idea" => {
                    score += FOCUS_IDEA_BONUS;
                    reasons.push("idea+".to_string());
                }
                "set_politics" => {
                    score += 2.0;
                    reasons.push("politics".to_string());
                }
                _ => {}
            }
        }
    }

    (score, reasons)
}
