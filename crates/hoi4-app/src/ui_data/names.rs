// 中文显示名解析迁移目标模块。
// G03 统一玩家可见名称入口，并记录缺失名称供后续补齐。

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayNameKind {
    Country,
    State,
    Province,
    Building,
    Good,
    PopClass,
    Law,
    Technology,
    ProductionMethod,
    Generic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingDisplayName {
    pub kind: DisplayNameKind,
    pub id: String,
    pub fallback: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MissingDisplayNameReport {
    entries: Vec<MissingDisplayName>,
}

impl MissingDisplayNameReport {
    pub fn entries(&self) -> &[MissingDisplayName] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn record(
        &mut self,
        kind: DisplayNameKind,
        id: impl Into<String>,
        fallback: impl Into<String>,
    ) {
        let entry = MissingDisplayName {
            kind,
            id: id.into(),
            fallback: fallback.into(),
        };
        if !self.entries.contains(&entry) {
            self.entries.push(entry);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChineseNameCatalog {
    entries: HashMap<String, String>,
}

impl ChineseNameCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_entries(entries: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    pub fn get(&self, id: &str) -> Option<&str> {
        self.entries.get(id).map(|name| name.as_str())
    }
}

pub struct DisplayNameResolver<'a> {
    loc_catalog: Option<&'a hoi4_ui::loc::LocCatalog>,
    chinese_catalog: ChineseNameCatalog,
    missing: RefCell<MissingDisplayNameReport>,
}

impl<'a> DisplayNameResolver<'a> {
    pub fn new(loc_catalog: Option<&'a hoi4_ui::loc::LocCatalog>) -> Self {
        Self {
            loc_catalog,
            chinese_catalog: ChineseNameCatalog::new(),
            missing: RefCell::new(MissingDisplayNameReport::default()),
        }
    }

    pub fn with_catalog(
        loc_catalog: Option<&'a hoi4_ui::loc::LocCatalog>,
        chinese_catalog: ChineseNameCatalog,
    ) -> Self {
        Self {
            loc_catalog,
            chinese_catalog,
            missing: RefCell::new(MissingDisplayNameReport::default()),
        }
    }

    pub fn missing_report(&self) -> MissingDisplayNameReport {
        self.missing.borrow().clone()
    }

    pub fn content_name(&self, kind: DisplayNameKind, id: &str, fallback: &str) -> String {
        let translated = hoi4_ui::i18n::tr(id);
        if translated != id {
            return translated.to_owned();
        }
        if let Some(name) = self.chinese_catalog.get(id) {
            return name.to_owned();
        }
        if !fallback.trim().is_empty() && fallback != id {
            self.missing
                .borrow_mut()
                .record(kind, id.to_owned(), fallback.to_owned());
            return fallback.to_owned();
        }
        self.missing
            .borrow_mut()
            .record(kind, id.to_owned(), fallback.to_owned());
        generic_missing_name(kind)
    }

    pub fn state_name(&self, raw: &str, state_idx: usize) -> String {
        if raw.trim().is_empty() {
            self.missing.borrow_mut().record(
                DisplayNameKind::State,
                format!("internal_state_{}", state_idx),
                "",
            );
            return generic_missing_name(DisplayNameKind::State);
        }

        if let Some(loc_catalog) = self.loc_catalog {
            let localized = loc_catalog.tr(raw);
            if localized != raw {
                return localized.to_owned();
            }
        }

        if let Some(name) = self.chinese_catalog.get(raw) {
            return name.to_owned();
        }

        let internal_state_prefix = ["STA", "TE_"].concat();
        if raw.starts_with(&internal_state_prefix) || raw.chars().all(|ch| ch.is_ascii_digit()) {
            self.missing.borrow_mut().record(
                DisplayNameKind::State,
                raw.to_owned(),
                format!("internal_state_{}", state_idx),
            );
            return generic_missing_name(DisplayNameKind::State);
        }

        self.missing.borrow_mut().record(
            DisplayNameKind::State,
            raw.to_owned(),
            format!("internal_state_{}", state_idx),
        );
        raw.to_owned()
    }

    pub fn country_name(&self, tag: &str, fallback: &str) -> String {
        let id = tag.trim();
        if id.is_empty() {
            self.missing.borrow_mut().record(
                DisplayNameKind::Country,
                "internal_country",
                fallback.trim(),
            );
            return generic_missing_name(DisplayNameKind::Country);
        }

        let translated = hoi4_ui::i18n::tr(id);
        if translated != id {
            return translated.to_owned();
        }

        if let Some(name) = self.chinese_catalog.get(id) {
            return name.to_owned();
        }

        let clean_fallback = fallback.trim();
        if !clean_fallback.is_empty() && clean_fallback != id {
            self.missing.borrow_mut().record(
                DisplayNameKind::Country,
                id.to_owned(),
                clean_fallback.to_owned(),
            );
            return clean_fallback.to_owned();
        }

        self.missing.borrow_mut().record(
            DisplayNameKind::Country,
            id.to_owned(),
            fallback.to_owned(),
        );
        generic_missing_name(DisplayNameKind::Country)
    }

    pub fn province_name(
        &self,
        province_id: u32,
        victory_point_name: Option<&str>,
        state_name: Option<&str>,
    ) -> String {
        if let Some(name) = curated_province_place_name(province_id) {
            return normalize_visible_place_name(name);
        }

        if let Some(name) = clean_visible_name(victory_point_name) {
            if !name.starts_with("VICTORY_POINTS_") && visible_in_current_language(name) {
                return normalize_visible_place_name(name);
            }
        }

        self.missing.borrow_mut().record(
            DisplayNameKind::Province,
            format!("internal_province_{}", province_id),
            state_name.unwrap_or_default(),
        );
        if let Some(name) =
            clean_visible_name(state_name).filter(|name| visible_in_current_language(name))
        {
            return format!("{name}地块");
        }
        generic_missing_name(DisplayNameKind::Province)
    }

    pub fn pop_class_name(&self, class: hoi4_state::PopClass) -> String {
        match class {
            hoi4_state::PopClass::Peasant => "农民",
            hoi4_state::PopClass::Worker => "工人",
            hoi4_state::PopClass::Clerk => "职员",
            hoi4_state::PopClass::Capitalist => "资本家",
            hoi4_state::PopClass::Aristocrat => "贵族",
            hoi4_state::PopClass::Soldier => "士兵",
        }
        .to_owned()
    }
}

pub fn localized_content_name(id: &str, fallback: &str) -> String {
    DisplayNameResolver::new(None).content_name(DisplayNameKind::Generic, id, fallback)
}

pub fn localized_state_name(
    raw: &str,
    state_idx: usize,
    loc_catalog: &hoi4_ui::loc::LocCatalog,
) -> String {
    DisplayNameResolver::new(Some(loc_catalog)).state_name(raw, state_idx)
}

fn clean_visible_name(name: Option<&str>) -> Option<&str> {
    name.map(str::trim).filter(|name| !name.is_empty())
}

fn normalize_visible_place_name(name: &str) -> String {
    let trimmed = name.trim();
    if matches!(
        hoi4_ui::i18n::current_language(),
        hoi4_ui::i18n::Language::Chinese
    ) {
        if let Some(base) = trimmed.strip_suffix('市') {
            if base.chars().count() > 1 {
                return base.to_owned();
            }
        }
    }
    trimmed.to_owned()
}

fn visible_in_current_language(name: &str) -> bool {
    match hoi4_ui::i18n::current_language() {
        hoi4_ui::i18n::Language::Chinese => contains_cjk(name),
        hoi4_ui::i18n::Language::English => true,
    }
}

fn contains_cjk(name: &str) -> bool {
    name.chars().any(|ch| {
        ('\u{3400}'..='\u{4dbf}').contains(&ch)
            || ('\u{4e00}'..='\u{9fff}').contains(&ch)
            || ('\u{f900}'..='\u{faff}').contains(&ch)
    })
}

#[derive(Clone, Copy)]
struct ProvincePlaceName {
    zh: &'static str,
    en: &'static str,
}

fn curated_province_place_name(province_id: u32) -> Option<&'static str> {
    static CATALOG: OnceLock<HashMap<u32, ProvincePlaceName>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            parse_place_name_table(include_str!(
                "../../assets/province_place_name_overrides.tsv"
            ))
        })
        .get(&province_id)
        .and_then(select_localized_place_name)
}

fn select_localized_place_name(name: &ProvincePlaceName) -> Option<&'static str> {
    match hoi4_ui::i18n::current_language() {
        hoi4_ui::i18n::Language::Chinese => (!name.zh.is_empty()).then_some(name.zh),
        hoi4_ui::i18n::Language::English => (!name.en.is_empty())
            .then_some(name.en)
            .or_else(|| (!name.zh.is_empty()).then_some(name.zh)),
    }
}

fn parse_place_name_table(raw: &'static str) -> HashMap<u32, ProvincePlaceName> {
    let mut out = HashMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let Some(id) = parts.next().and_then(|raw| raw.parse::<u32>().ok()) else {
            continue;
        };
        let zh = parts.next().map(str::trim).unwrap_or("");
        let en = parts.next().map(str::trim).unwrap_or("");
        if zh.is_empty() && en.is_empty() {
            continue;
        }
        out.insert(id, ProvincePlaceName { zh, en });
    }
    out
}

fn generic_missing_name(kind: DisplayNameKind) -> String {
    match kind {
        DisplayNameKind::Country => "未知国家",
        DisplayNameKind::State => "未命名州",
        DisplayNameKind::Province => "省份",
        DisplayNameKind::Building => "未命名建筑",
        DisplayNameKind::Good => "未命名商品",
        DisplayNameKind::PopClass => "未知人群",
        DisplayNameKind::Law => "未知法律",
        DisplayNameKind::Technology => "未知科技",
        DisplayNameKind::ProductionMethod => "未知生产方式",
        DisplayNameKind::Generic => "未命名项目",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_resolver_hides_internal_state_id() {
        let resolver = DisplayNameResolver::new(None);

        assert_eq!(
            resolver.state_name(&["STA", "TE_123"].concat(), 7),
            "未命名州"
        );
        assert!(!resolver.missing_report().is_empty());
    }

    #[test]
    fn content_resolver_prefers_translated_or_fallback_name() {
        let resolver = DisplayNameResolver::new(None);

        assert_eq!(
            resolver.content_name(DisplayNameKind::Good, "missing_good", "测试商品"),
            "测试商品"
        );
        assert_eq!(resolver.missing_report().entries().len(), 1);
    }

    #[test]
    fn country_resolver_hides_untranslated_tag() {
        let resolver = DisplayNameResolver::new(None);

        let name = resolver.country_name("ABC", "ABC");

        assert_ne!(name, "ABC");
        assert_eq!(resolver.missing_report().entries().len(), 1);
    }

    #[test]
    fn province_resolver_uses_curated_historical_override_before_victory_point() {
        let _guard = language_test_lock();
        let resolver = DisplayNameResolver::new(None);
        let previous = hoi4_ui::i18n::current_language();
        hoi4_ui::i18n::set_language(hoi4_ui::i18n::Language::Chinese);

        let name = resolver.province_name(7108, Some("深圳市"), Some("广东"));

        assert_eq!(name, "宝安");
        assert!(resolver.missing_report().is_empty());
        hoi4_ui::i18n::set_language(previous);
    }

    #[test]
    fn province_resolver_uses_english_curated_historical_override_in_english() {
        let _guard = language_test_lock();
        let resolver = DisplayNameResolver::new(None);
        let previous = hoi4_ui::i18n::current_language();
        hoi4_ui::i18n::set_language(hoi4_ui::i18n::Language::English);

        let name = resolver.province_name(7108, Some("Shenzhen"), Some("Guangdong"));

        assert_eq!(name, "Bao'an");
        hoi4_ui::i18n::set_language(previous);
    }

    #[test]
    fn province_resolver_rejects_english_victory_point_in_chinese() {
        let _guard = language_test_lock();
        let resolver = DisplayNameResolver::new(None);
        let previous = hoi4_ui::i18n::current_language();
        hoi4_ui::i18n::set_language(hoi4_ui::i18n::Language::Chinese);

        let name = resolver.province_name(42, Some("Guangzhou"), Some("广东"));

        assert_eq!(name, "广东地块");
        assert_eq!(resolver.missing_report().entries().len(), 1);
        hoi4_ui::i18n::set_language(previous);
    }

    #[test]
    fn province_resolver_normalizes_city_suffix_in_chinese() {
        let _guard = language_test_lock();
        let resolver = DisplayNameResolver::new(None);
        let previous = hoi4_ui::i18n::current_language();
        hoi4_ui::i18n::set_language(hoi4_ui::i18n::Language::Chinese);

        let name = resolver.province_name(1047, Some("广州市"), Some("广州"));

        assert_eq!(name, "广州");
        assert!(resolver.missing_report().is_empty());
        hoi4_ui::i18n::set_language(previous);
    }

    fn language_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap()
    }

    #[test]
    fn province_resolver_uses_generic_label_when_no_place_or_state_name_exists() {
        let resolver = DisplayNameResolver::new(None);

        let name = resolver.province_name(42, None, None);

        assert_eq!(name, "省份");
        assert_eq!(resolver.missing_report().entries().len(), 1);
    }

    #[test]
    fn province_resolver_uses_state_tile_label_when_no_place_name_exists() {
        let resolver = DisplayNameResolver::new(None);

        let name = resolver.province_name(42, None, Some("黑森"));

        assert_eq!(name, "黑森地块");
    }
}
