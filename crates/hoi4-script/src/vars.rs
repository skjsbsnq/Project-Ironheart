//! 变量与 flag — country 级 / state 级 / global 级。
//!
//! HOI4 中 flag 是布尔（"set"/"clr"/"check"），variable 是浮点（可加减）。

use std::collections::{HashMap, HashSet};

use hoi4_state::{CountryId, StateId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKind {
    Country,
    State,
    Global,
}

/// 全部变量（数值）
#[derive(Debug, Clone, Default)]
pub struct Variables {
    /// (CountryId, var_name) → value
    pub country: HashMap<(CountryId, String), f64>,
    /// (StateId, var_name) → value
    pub state: HashMap<(StateId, String), f64>,
    /// var_name → value
    pub global: HashMap<String, f64>,
}

impl Variables {
    pub fn get_country(&self, c: CountryId, name: &str) -> f64 {
        self.country
            .get(&(c, name.to_owned()))
            .copied()
            .unwrap_or(0.0)
    }

    pub fn set_country(&mut self, c: CountryId, name: &str, v: f64) {
        self.country.insert((c, name.to_owned()), v);
    }

    pub fn add_country(&mut self, c: CountryId, name: &str, delta: f64) -> f64 {
        let v = self.country.entry((c, name.to_owned())).or_insert(0.0);
        *v += delta;
        *v
    }

    pub fn get_state(&self, s: StateId, name: &str) -> f64 {
        self.state
            .get(&(s, name.to_owned()))
            .copied()
            .unwrap_or(0.0)
    }

    pub fn set_state(&mut self, s: StateId, name: &str, v: f64) {
        self.state.insert((s, name.to_owned()), v);
    }

    pub fn add_state(&mut self, s: StateId, name: &str, delta: f64) -> f64 {
        let v = self.state.entry((s, name.to_owned())).or_insert(0.0);
        *v += delta;
        *v
    }

    pub fn get_global(&self, name: &str) -> f64 {
        self.global.get(name).copied().unwrap_or(0.0)
    }

    pub fn set_global(&mut self, name: &str, v: f64) {
        self.global.insert(name.to_owned(), v);
    }

    pub fn add_global(&mut self, name: &str, delta: f64) -> f64 {
        let v = self.global.entry(name.to_owned()).or_insert(0.0);
        *v += delta;
        *v
    }
}

/// 全部 flag（布尔；HOI4 中带可选 days_remaining，但我们简化为永久；
/// 调用方可用 [`Flags::clear_country_flag`] 自行清除）。
#[derive(Debug, Clone, Default)]
pub struct Flags {
    pub country: HashMap<CountryId, HashSet<String>>,
    pub state: HashMap<StateId, HashSet<String>>,
    pub global: HashSet<String>,
}

impl Flags {
    pub fn has_country_flag(&self, c: CountryId, name: &str) -> bool {
        self.country
            .get(&c)
            .map(|s| s.contains(name))
            .unwrap_or(false)
    }

    pub fn set_country_flag(&mut self, c: CountryId, name: &str) {
        self.country.entry(c).or_default().insert(name.to_owned());
    }

    pub fn clear_country_flag(&mut self, c: CountryId, name: &str) {
        if let Some(s) = self.country.get_mut(&c) {
            s.remove(name);
        }
    }

    pub fn has_state_flag(&self, s: StateId, name: &str) -> bool {
        self.state
            .get(&s)
            .map(|set| set.contains(name))
            .unwrap_or(false)
    }

    pub fn set_state_flag(&mut self, s: StateId, name: &str) {
        self.state.entry(s).or_default().insert(name.to_owned());
    }

    pub fn clear_state_flag(&mut self, s: StateId, name: &str) {
        if let Some(set) = self.state.get_mut(&s) {
            set.remove(name);
        }
    }

    pub fn has_global_flag(&self, name: &str) -> bool {
        self.global.contains(name)
    }

    pub fn set_global_flag(&mut self, name: &str) {
        self.global.insert(name.to_owned());
    }

    pub fn clear_global_flag(&mut self, name: &str) {
        self.global.remove(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_set_get_add() {
        let mut v = Variables::default();
        let c = CountryId(1);
        v.set_country(c, "x", 5.0);
        assert_eq!(v.get_country(c, "x"), 5.0);
        let new_v = v.add_country(c, "x", 3.0);
        assert_eq!(new_v, 8.0);
        assert_eq!(v.get_country(c, "x"), 8.0);
        assert_eq!(v.get_country(c, "missing"), 0.0);
    }

    #[test]
    fn flags_set_clear() {
        let mut f = Flags::default();
        let c = CountryId(1);
        assert!(!f.has_country_flag(c, "war"));
        f.set_country_flag(c, "war");
        assert!(f.has_country_flag(c, "war"));
        f.clear_country_flag(c, "war");
        assert!(!f.has_country_flag(c, "war"));
    }

    #[test]
    fn global_flag() {
        let mut f = Flags::default();
        f.set_global_flag("ww2_started");
        assert!(f.has_global_flag("ww2_started"));
        f.clear_global_flag("ww2_started");
        assert!(!f.has_global_flag("ww2_started"));
    }
}
