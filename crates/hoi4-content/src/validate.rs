//! Focus tree schema validator.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::focus::{Effect, FocusTree, Trigger};

/// 校验错误。
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// 空树。
    EmptyTree,
    /// 重复 focus id。
    DuplicateId(String),
    /// 前置引用了不存在的 focus。
    MissingPrerequisite { focus: String, missing: String },
    /// 互斥引用了不存在的 focus。
    MissingMutualExclusive { focus: String, missing: String },
    /// 互斥不对称（A 声明与 B 互斥，但 B 未声明与 A 互斥）。
    AsymmetricMutualExclusive { a: String, b: String },
    /// 位置重叠。
    OverlappingPosition {
        a: String,
        b: String,
        pos: (i32, i32),
    },
    /// cost_days 为 0。
    ZeroCost(String),
    /// 前置形成环路。
    CyclicPrerequisite(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTree => write!(f, "focus tree has no focuses"),
            Self::DuplicateId(id) => write!(f, "duplicate focus id: {id}"),
            Self::MissingPrerequisite { focus, missing } => {
                write!(f, "focus '{focus}' prerequisite '{missing}' not found")
            }
            Self::MissingMutualExclusive { focus, missing } => {
                write!(f, "focus '{focus}' mutual_exclusive '{missing}' not found")
            }
            Self::AsymmetricMutualExclusive { a, b } => {
                write!(
                    f,
                    "mutual_exclusive asymmetric: '{a}' lists '{b}' but not vice versa"
                )
            }
            Self::OverlappingPosition { a, b, pos } => {
                write!(
                    f,
                    "focuses '{a}' and '{b}' overlap at ({}, {})",
                    pos.0, pos.1
                )
            }
            Self::ZeroCost(id) => write!(f, "focus '{id}' has cost_days = 0"),
            Self::CyclicPrerequisite(id) => write!(f, "focus '{id}' has cyclic prerequisites"),
        }
    }
}

/// 校验整棵国策树，返回所有发现的错误。
pub fn validate_focus_tree(tree: &FocusTree) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if tree.focuses.is_empty() {
        errors.push(ValidationError::EmptyTree);
        return errors;
    }

    let ids: HashSet<&str> = tree.focuses.iter().map(|f| f.id.as_str()).collect();

    // Duplicate ids
    let mut seen = HashSet::new();
    for f in &tree.focuses {
        if !seen.insert(f.id.as_str()) {
            errors.push(ValidationError::DuplicateId(f.id.clone()));
        }
    }

    // Per-focus checks
    for f in &tree.focuses {
        // Zero cost
        if f.cost_days == 0 {
            errors.push(ValidationError::ZeroCost(f.id.clone()));
        }

        // Prerequisites reference valid ids
        for group in &f.prerequisites {
            for prereq in group {
                if !ids.contains(prereq.as_str()) {
                    errors.push(ValidationError::MissingPrerequisite {
                        focus: f.id.clone(),
                        missing: prereq.clone(),
                    });
                }
            }
        }

        // Mutual exclusives reference valid ids
        for me in &f.mutually_exclusive {
            if !ids.contains(me.as_str()) {
                errors.push(ValidationError::MissingMutualExclusive {
                    focus: f.id.clone(),
                    missing: me.clone(),
                });
            }
        }
    }

    // Symmetric mutual exclusives
    let me_map: HashMap<&str, HashSet<&str>> = tree
        .focuses
        .iter()
        .map(|f| {
            let set: HashSet<&str> = f.mutually_exclusive.iter().map(|s| s.as_str()).collect();
            (f.id.as_str(), set)
        })
        .collect();

    for f in &tree.focuses {
        for me in &f.mutually_exclusive {
            if let Some(other_set) = me_map.get(me.as_str()) {
                if !other_set.contains(f.id.as_str()) {
                    errors.push(ValidationError::AsymmetricMutualExclusive {
                        a: f.id.clone(),
                        b: me.clone(),
                    });
                }
            }
        }
    }

    // Position overlap
    let mut pos_map: HashMap<(i32, i32), &str> = HashMap::new();
    for f in &tree.focuses {
        if let Some(&existing) = pos_map.get(&f.position) {
            errors.push(ValidationError::OverlappingPosition {
                a: existing.to_owned(),
                b: f.id.clone(),
                pos: f.position,
            });
        } else {
            pos_map.insert(f.position, &f.id);
        }
    }

    // Cycle detection (DFS)
    check_cycles(tree, &ids, &mut errors);

    errors
}

fn check_cycles(tree: &FocusTree, ids: &HashSet<&str>, errors: &mut Vec<ValidationError>) {
    // Build adjacency: focus -> set of all prerequisite ids (flattened)
    let prereq_map: HashMap<&str, Vec<&str>> = tree
        .focuses
        .iter()
        .map(|f| {
            let deps: Vec<&str> = f
                .prerequisites
                .iter()
                .flatten()
                .filter(|p| ids.contains(p.as_str()))
                .map(|p| p.as_str())
                .collect();
            (f.id.as_str(), deps)
        })
        .collect();

    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Unvisited,
        InStack,
        Done,
    }

    let mut state: HashMap<&str, State> = ids.iter().map(|&id| (id, State::Unvisited)).collect();

    fn dfs<'a>(
        node: &'a str,
        prereq_map: &HashMap<&str, Vec<&'a str>>,
        state: &mut HashMap<&'a str, State>,
        errors: &mut Vec<ValidationError>,
    ) {
        state.insert(node, State::InStack);
        if let Some(deps) = prereq_map.get(node) {
            for &dep in deps {
                match state.get(dep).copied().unwrap_or(State::Unvisited) {
                    State::InStack => {
                        errors.push(ValidationError::CyclicPrerequisite(node.to_owned()));
                    }
                    State::Unvisited => dfs(dep, prereq_map, state, errors),
                    State::Done => {}
                }
            }
        }
        state.insert(node, State::Done);
    }

    let id_list: Vec<&str> = ids.iter().copied().collect();
    for &id in &id_list {
        if state[id] == State::Unvisited {
            dfs(id, &prereq_map, &mut state, errors);
        }
    }
}

/// 递归校验 trigger 引用的 focus id 是否存在于树中。
pub fn validate_trigger_refs(trigger: &Trigger, ids: &HashSet<&str>) -> Vec<String> {
    let mut missing = Vec::new();
    match trigger {
        Trigger::HasCompletedFocus(f) => {
            if !ids.contains(f.as_str()) {
                missing.push(f.clone());
            }
        }
        Trigger::And(v) | Trigger::Or(v) => {
            for t in v {
                missing.extend(validate_trigger_refs(t, ids));
            }
        }
        Trigger::Not(t) => missing.extend(validate_trigger_refs(t, ids)),
        _ => {}
    }
    missing
}

/// 递归校验 effect 中引用的 focus id（如 UnlockDecision 等不涉及 focus，此处留扩展口）。
pub fn validate_effect_refs(_effects: &[Effect], _ids: &HashSet<&str>) -> Vec<String> {
    // 当前 Effect 枚举中无直接 focus 引用，预留接口。
    Vec::new()
}
