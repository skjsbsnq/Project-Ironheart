//! Unit counter utilities — archetype classification and visibility.
//!
//! CR-5: Old `UnitCounterInstance`, `generate_map_symbols`, `decluster_counters`,
//! `CounterGrouping` etc. have been removed. The new HOI3-style counter system
//! lives in [`crate::counter_v3`].
//!
//! Retained:
//! - [`UnitArchetype`] — 16-variant enum for division main archetype
//! - [`classify_subunit`] — map subunit key+group to archetype
//! - [`template_main_archetype`] — majority-vote archetype for a template
//! - [`visibility`] — fog-of-war country/province filtering

/// 主战兵种 archetype。15 种 + Unknown（u8 0..15），决定 counter 上画哪个白色剪影。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum UnitArchetype {
    /// 情报缺失 / 全部 support / 无 regiments → 用 "no_intel"。
    Unknown = 0,
    Infantry = 1,
    Cavalry = 2,
    Motorized = 3,
    Mechanized = 4,
    Mountain = 5,
    Marine = 6,
    Paratrooper = 7,
    Militia = 8,
    LightArmor = 9,
    MediumArmor = 10,
    HeavyArmor = 11,
    ModernArmor = 12,
    Artillery = 13,
    AntiTank = 14,
    AntiAir = 15,
}

impl UnitArchetype {
    /// archetype 总数（含 Unknown）。
    pub const COUNT: u8 = 16;

    /// 把 u8 解码回 enum；越界回 Unknown。
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Infantry,
            2 => Self::Cavalry,
            3 => Self::Motorized,
            4 => Self::Mechanized,
            5 => Self::Mountain,
            6 => Self::Marine,
            7 => Self::Paratrooper,
            8 => Self::Militia,
            9 => Self::LightArmor,
            10 => Self::MediumArmor,
            11 => Self::HeavyArmor,
            12 => Self::ModernArmor,
            13 => Self::Artillery,
            14 => Self::AntiTank,
            15 => Self::AntiAir,
            _ => Self::Unknown,
        }
    }

    /// vanilla `GFX_*` sprite name（在 `interface/subuniticons.gfx` 或
    /// `interface/mapicons.gfx` 注册）。Unknown 走"无情报"占位。
    pub fn onmap_sprite_name(self) -> &'static str {
        match self {
            Self::Unknown => "GFX_onmap_no_intel_icon",
            Self::Infantry => "GFX_unit_infantry_icon_medium_white",
            Self::Cavalry => "GFX_unit_cavalry_icon_medium_white",
            Self::Motorized => "GFX_unit_motorized_icon_medium_white",
            Self::Mechanized => "GFX_unit_mechanized_icon_medium_white",
            Self::Mountain => "GFX_unit_mountaineers_icon_medium_white",
            Self::Marine => "GFX_unit_marine_icon_medium_white",
            Self::Paratrooper => "GFX_unit_paratrooper_icon_medium_white",
            Self::Militia => "GFX_unit_militia_icon_medium_white",
            Self::LightArmor => "GFX_unit_light_armor_icon_medium_white",
            Self::MediumArmor => "GFX_unit_medium_armor_icon_medium_white",
            Self::HeavyArmor => "GFX_unit_heavy_armor_icon_medium_white",
            Self::ModernArmor => "GFX_unit_modern_armor_icon_medium_white",
            Self::Artillery => "GFX_unit_artillery_brigade_icon_medium_white",
            Self::AntiTank => "GFX_unit_anti_tank_brigade_icon_medium_white",
            Self::AntiAir => "GFX_unit_anti_air_brigade_icon_medium_white",
        }
    }

    /// 全部已知 archetype（不含 Unknown，用于纹理预加载）。
    pub fn all_known() -> &'static [UnitArchetype] {
        &[
            Self::Infantry,
            Self::Cavalry,
            Self::Motorized,
            Self::Mechanized,
            Self::Mountain,
            Self::Marine,
            Self::Paratrooper,
            Self::Militia,
            Self::LightArmor,
            Self::MediumArmor,
            Self::HeavyArmor,
            Self::ModernArmor,
            Self::Artillery,
            Self::AntiTank,
            Self::AntiAir,
        ]
    }
}

/// 把 `subunits[regiment_key].group / .types` 映射到 archetype。
///
/// vanilla `common/units/<name>.txt` 里 SubunitDef.group 取值有：
/// `infantry / armor / cavalry / motorized / mechanized / artillery`。
/// 但 archetype 比 group 更细——例如 group=infantry 又分 marine / mountain /
/// paratrooper / militia——所以也要看 key 本身。
pub fn classify_subunit(key: &str, group: &str) -> UnitArchetype {
    let k = key.to_ascii_lowercase();
    // 1. 先按 key 精确分类（更细，覆盖 vanilla 大多数命名）
    if k.contains("marine") {
        return UnitArchetype::Marine;
    }
    if k.contains("mountain") {
        return UnitArchetype::Mountain;
    }
    if k.contains("paratroop") {
        return UnitArchetype::Paratrooper;
    }
    if k.contains("militia") {
        return UnitArchetype::Militia;
    }
    if k.contains("modern_armor") || k.contains("modern_tank") {
        return UnitArchetype::ModernArmor;
    }
    if k.contains("super_heavy_armor") || k.contains("super_heavy_tank") {
        return UnitArchetype::HeavyArmor; // super heavy 复用 heavy 图标
    }
    if k.contains("heavy_armor") || k.contains("heavy_tank") {
        return UnitArchetype::HeavyArmor;
    }
    if k.contains("medium_armor") || k.contains("medium_tank") {
        return UnitArchetype::MediumArmor;
    }
    if k.contains("light_armor") || k.contains("light_tank") {
        return UnitArchetype::LightArmor;
    }
    if k.contains("anti_tank") || k.starts_with("at_") || k == "at" {
        return UnitArchetype::AntiTank;
    }
    if k.contains("anti_air") || k.starts_with("aa_") || k == "aa" {
        return UnitArchetype::AntiAir;
    }
    if k.contains("rocket_artillery") || k.contains("artillery") || k.contains("art_") || k == "art"
    {
        return UnitArchetype::Artillery;
    }
    if k.contains("amphibious_armor") || k.contains("amphibious_tank") {
        return UnitArchetype::MediumArmor;
    }
    if k.contains("amphibious_mech") {
        return UnitArchetype::Mechanized;
    }
    if k.contains("mechanized") {
        return UnitArchetype::Mechanized;
    }
    if k.contains("motorized") || k.contains("motorised") {
        return UnitArchetype::Motorized;
    }
    if k.contains("cavalry") || k.contains("camelry") {
        return UnitArchetype::Cavalry;
    }
    if k.contains("infantry") {
        return UnitArchetype::Infantry;
    }

    // 2. 退回到 SubunitDef.group
    match group {
        "infantry" => UnitArchetype::Infantry,
        "armor" => UnitArchetype::MediumArmor,
        "cavalry" => UnitArchetype::Cavalry,
        "motorized" => UnitArchetype::Motorized,
        "mechanized" => UnitArchetype::Mechanized,
        "artillery" => UnitArchetype::Artillery,
        _ => UnitArchetype::Unknown,
    }
}

/// 汇总一个模板的"主"archetype：所有 regiment（不含 support）的多数票。
/// 平票按枚举值优先 — Armor 系列 > Mechanized > Motorized > Cavalry > Infantry > Mountain > Marine。
pub fn template_main_archetype(
    template: &hoi4_data::military::DivisionTemplate,
    subunits: &std::collections::HashMap<String, hoi4_data::military::SubunitDef>,
) -> UnitArchetype {
    if template.regiments.is_empty() {
        return UnitArchetype::Unknown;
    }
    let mut counts = [0u32; UnitArchetype::COUNT as usize];
    for key in &template.regiments {
        let group = subunits.get(key).map(|d| d.group.as_str()).unwrap_or("");
        let arch = classify_subunit(key, group);
        counts[arch as u8 as usize] += 1;
    }
    // 多数票：扫描，平票时优先级高的 archetype index 大的胜出（armor > inf）
    let mut best_idx = 0usize;
    let mut best_count = 0u32;
    // 跳过 Unknown(0) 除非全部是 Unknown
    for (idx, &c) in counts.iter().enumerate().skip(1) {
        if c > best_count || (c == best_count && c > 0 && idx > best_idx) {
            best_count = c;
            best_idx = idx;
        }
    }
    if best_count == 0 {
        UnitArchetype::Unknown
    } else {
        UnitArchetype::from_u8(best_idx as u8)
    }
}

/// On-map counter visibility — which countries' units are drawn from a given
/// observer's point of view. This mirrors vanilla's fog-of-war on the unit
/// layer: the player only sees their own units, those of faction allies,
/// neighbours, and countries with whom they are at war (where vanilla draws
/// enemy counters in the spotted areas).
///
/// We don't simulate the full vanilla intel pipeline (recon missions / spy
/// agencies / radar) yet, so the **at-war** set is used as a coarse proxy
/// for "you have army intel on this country". A full implementation would
/// also consult `world.intel` once that exists.
///
/// Observer mode (`player.is_none()`) shows every country — useful for
/// development and the implicit pre-country-select state.
///
/// ## Phase 3.12.15.bis.4 — Province-level spotted system
///
/// The spotted system adds **per-province** granularity on top of the
/// country-level visible set. Own and allied units are always shown
/// everywhere. Enemy (at-war) units are only shown in **spotted** provinces:
///
/// 1. Provinces owned or controlled by the player → spotted.
/// 2. Provinces adjacent to the player's owned/controlled provinces → spotted
///    (the "adjacent rule" — you can see across your borders).
/// 3. Provinces containing any division in active combat → spotted
///    (the "battle rule" — combat reveals positions).
/// 4. Provinces containing the player's own divisions → spotted
///    (units on the ground provide intel).
/// 5. Neutral countries (not at war with anyone the player is at war with)
///    do not show counters at all — this is already handled by the
///    country-level `visible_countries` filter.
pub mod visibility {
    use std::collections::HashSet;

    use hoi4_state::{CountryId, World};

    /// Compute the set of countries whose on-map counters should be drawn
    /// for the given observer.
    ///
    /// The returned set is closed: it always contains `player` (when set),
    /// every member of the player's faction, every country that owns a
    /// province bordering one the player owns or controls, and every country
    /// the player is at war with.
    pub fn visible_countries(world: &World, player: CountryId) -> HashSet<CountryId> {
        let mut out = HashSet::new();
        if player.is_none() {
            for ci in 0..world.countries.count {
                out.insert(CountryId(ci as u16));
            }
            return out;
        }
        out.insert(player);

        if let Some(f_idx) = world.diplomacy.faction_of(player) {
            if let Some(faction) = world.diplomacy.faction(f_idx) {
                for &m in &faction.members {
                    out.insert(m);
                }
            }
        }

        for war in world.diplomacy.wars.values() {
            if !war.contains(player) {
                continue;
            }
            let player_is_attacker = war.attackers.contains(&player);
            let opponents = if player_is_attacker {
                &war.defenders
            } else {
                &war.attackers
            };
            for &enemy in opponents {
                out.insert(enemy);
            }
        }

        let max_prov = world.provinces.count;
        if max_prov > 0 {
            let mut player_provs: Vec<usize> = Vec::new();
            for pid in 0..max_prov {
                let owner = world.provinces.owners[pid];
                let ctrl = world.provinces.controllers[pid];
                if owner == player || ctrl == player {
                    player_provs.push(pid);
                }
            }
            for &pid in &player_provs {
                if pid >= world.map.adjacencies.len() {
                    continue;
                }
                for &n_raw in &world.map.adjacencies[pid] {
                    let n = n_raw as usize;
                    if n >= max_prov {
                        continue;
                    }
                    let n_owner = world.provinces.owners[n];
                    let n_ctrl = world.provinces.controllers[n];
                    if !n_owner.is_none() {
                        out.insert(n_owner);
                    }
                    if !n_ctrl.is_none() {
                        out.insert(n_ctrl);
                    }
                }
            }
        }

        out.remove(&CountryId::NONE);
        out
    }

    /// Compute the set of **spotted province IDs** for the given player.
    ///
    /// A province is spotted if:
    /// 1. The player owns or controls it.
    /// 2. It is adjacent to a province the player owns or controls.
    /// 3. Any division located there is in active combat.
    /// 4. The player has a division located there.
    ///
    /// Returns an empty set in observer mode (all provinces visible → no
    /// per-province filter needed). Province IDs are `u16` matching
    /// `world.provinces` indices.
    pub fn spotted_provinces(world: &World, player: CountryId) -> HashSet<u16> {
        let mut spotted = HashSet::new();
        if player.is_none() {
            return spotted;
        }

        let max_prov = world.provinces.count;
        if max_prov == 0 {
            return spotted;
        }

        let mut player_provs: Vec<usize> = Vec::new();

        // Rule 1 & 4: player-owned/controlled provinces + provinces with
        // player's own divisions.
        for pid in 0..max_prov {
            let owner = world.provinces.owners[pid];
            let ctrl = world.provinces.controllers[pid];
            if owner == player || ctrl == player {
                spotted.insert(pid as u16);
                player_provs.push(pid);
            }
        }

        for i in 0..world.divisions.count {
            let owner = world.divisions.owners[i];
            if owner != player {
                continue;
            }
            let loc = world.divisions.locations[i];
            if loc.is_none() {
                continue;
            }
            let pid = loc.0 as usize;
            if pid < max_prov {
                spotted.insert(pid as u16);
                if !spotted.contains(&(pid as u16)) {
                    player_provs.push(pid);
                }
            }
        }

        // Rule 2: adjacent to player-owned/controlled provinces.
        for &pid in &player_provs {
            if pid >= world.map.adjacencies.len() {
                continue;
            }
            for &n_raw in &world.map.adjacencies[pid] {
                let n = n_raw as usize;
                if n < max_prov {
                    spotted.insert(n as u16);
                }
            }
        }

        // Rule 3: provinces with divisions in active combat (both sides).
        for i in 0..world.divisions.count {
            if i >= world.divisions.in_combat.len() || !world.divisions.in_combat[i] {
                continue;
            }
            let loc = world.divisions.locations[i];
            if loc.is_none() {
                continue;
            }
            let pid = loc.0 as usize;
            if pid < max_prov {
                spotted.insert(pid as u16);
            }
        }

        spotted
    }

    /// Determine whether a unit counter should be visible, combining the
    /// country-level visible set with the province-level spotted set.
    ///
    /// - Own / faction-ally / neutral-neighbour counters: visible if the
    ///   country is in the `visible` set (always shown, no province filter).
    /// - Enemy (at-war) counters: visible only if the province is spotted.
    ///   A province is spotted if it's in the `spotted` set **or** the
    ///   counter's owner is in the visible set and is NOT at war with the
    ///   player (i.e. own / ally / neutral neighbour → always show).
    ///
    /// When `spotted` is `None`, falls back to country-level-only filtering
    /// (observer mode / pre-spotted-system code paths).
    #[inline]
    pub fn is_counter_visible(
        visible: &HashSet<CountryId>,
        spotted: Option<&HashSet<u16>>,
        owner: CountryId,
        province_id: u16,
        at_war_with_player: bool,
    ) -> bool {
        if !is_visible(visible, owner) {
            return false;
        }
        if !at_war_with_player {
            return true;
        }
        match spotted {
            Some(sp) => sp.contains(&province_id),
            None => true,
        }
    }

    #[inline]
    pub fn is_visible(set: &HashSet<CountryId>, country: CountryId) -> bool {
        !country.is_none() && set.contains(&country)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_data::military::{DivisionTemplate, SubunitDef};
    use std::collections::HashMap;

    #[test]
    fn classify_by_key_dominates_group() {
        // marine 关键字优先于 group=infantry
        assert_eq!(
            classify_subunit("marines", "infantry"),
            UnitArchetype::Marine
        );
        assert_eq!(
            classify_subunit("mountaineers", "infantry"),
            UnitArchetype::Mountain
        );
        assert_eq!(
            classify_subunit("paratrooper", "infantry"),
            UnitArchetype::Paratrooper
        );
        assert_eq!(
            classify_subunit("militia", "infantry"),
            UnitArchetype::Militia
        );
        assert_eq!(
            classify_subunit("medium_armor", "armor"),
            UnitArchetype::MediumArmor
        );
        assert_eq!(
            classify_subunit("heavy_armor", "armor"),
            UnitArchetype::HeavyArmor
        );
        assert_eq!(
            classify_subunit("super_heavy_armor", "armor"),
            UnitArchetype::HeavyArmor
        );
        assert_eq!(
            classify_subunit("modern_armor", "armor"),
            UnitArchetype::ModernArmor
        );
        assert_eq!(
            classify_subunit("light_tank_brigade", "armor"),
            UnitArchetype::LightArmor
        );
        assert_eq!(
            classify_subunit("anti_tank", "infantry"),
            UnitArchetype::AntiTank
        );
        assert_eq!(
            classify_subunit("anti_air", "infantry"),
            UnitArchetype::AntiAir
        );
        assert_eq!(
            classify_subunit("artillery", "infantry"),
            UnitArchetype::Artillery
        );
        assert_eq!(
            classify_subunit("rocket_artillery", "infantry"),
            UnitArchetype::Artillery
        );
    }

    #[test]
    fn classify_falls_back_to_group() {
        assert_eq!(
            classify_subunit("foo_bar", "armor"),
            UnitArchetype::MediumArmor
        );
        assert_eq!(classify_subunit("xyz", "infantry"), UnitArchetype::Infantry);
        assert_eq!(classify_subunit("xyz", "cavalry"), UnitArchetype::Cavalry);
        assert_eq!(
            classify_subunit("xyz", "motorized"),
            UnitArchetype::Motorized
        );
        assert_eq!(
            classify_subunit("xyz", "mechanized"),
            UnitArchetype::Mechanized
        );
        assert_eq!(
            classify_subunit("xyz", "artillery"),
            UnitArchetype::Artillery
        );
        assert_eq!(classify_subunit("xyz", ""), UnitArchetype::Unknown);
    }

    #[test]
    fn template_main_archetype_majority_wins() {
        let mut subunits = HashMap::new();
        let inf = SubunitDef {
            key: "infantry".into(),
            group: "infantry".into(),
            ..Default::default()
        };
        let armor = SubunitDef {
            key: "medium_armor".into(),
            group: "armor".into(),
            ..Default::default()
        };
        subunits.insert("infantry".to_string(), inf);
        subunits.insert("medium_armor".to_string(), armor);

        let mut tpl = DivisionTemplate {
            name: "Pz".into(),
            country_tag: None,
            regiments: vec!["infantry".into(), "infantry".into(), "medium_armor".into()],
            support: vec![],
            division_names_group: None,
        };
        // 2 inf vs 1 armor → infantry wins
        assert_eq!(
            template_main_archetype(&tpl, &subunits),
            UnitArchetype::Infantry
        );

        tpl.regiments = vec![
            "medium_armor".into(),
            "medium_armor".into(),
            "infantry".into(),
        ];
        assert_eq!(
            template_main_archetype(&tpl, &subunits),
            UnitArchetype::MediumArmor
        );

        // 平票 → enum value 大者胜（armor=10 > infantry=1）
        tpl.regiments = vec!["medium_armor".into(), "infantry".into()];
        assert_eq!(
            template_main_archetype(&tpl, &subunits),
            UnitArchetype::MediumArmor
        );
    }

    #[test]
    fn template_empty_regiments_unknown() {
        let subunits = HashMap::new();
        let tpl = DivisionTemplate {
            name: "Empty".into(),
            country_tag: None,
            regiments: vec![],
            support: vec!["engineer_company".into()],
            division_names_group: None,
        };
        assert_eq!(
            template_main_archetype(&tpl, &subunits),
            UnitArchetype::Unknown
        );
    }

    #[test]
    fn archetype_sprite_names_have_gfx_prefix() {
        for &arch in UnitArchetype::all_known() {
            assert!(arch.onmap_sprite_name().starts_with("GFX_"));
        }
        assert!(UnitArchetype::Unknown
            .onmap_sprite_name()
            .starts_with("GFX_"));
    }

    #[test]
    fn archetype_from_u8_roundtrip() {
        for &arch in UnitArchetype::all_known() {
            let v = arch as u8;
            assert_eq!(UnitArchetype::from_u8(v), arch);
        }
        assert_eq!(UnitArchetype::from_u8(99), UnitArchetype::Unknown);
    }
}

#[cfg(test)]
mod visibility_tests {
    use super::visibility::*;
    use hoi4_state::CountryId;
    use std::collections::HashSet;

    fn cid(v: u16) -> CountryId {
        CountryId(v)
    }

    #[test]
    fn is_visible_none_country_is_invisible() {
        let set: HashSet<CountryId> = [cid(0), cid(1)].into_iter().collect();
        assert!(!is_visible(&set, CountryId::NONE));
    }

    #[test]
    fn is_counter_visible_own_units_always_visible() {
        let player = cid(0);
        let visible: HashSet<CountryId> = [player].into_iter().collect();
        let spotted: HashSet<u16> = [99u16].into_iter().collect();
        // Own units (not at war with self) → always visible
        assert!(is_counter_visible(
            &visible,
            Some(&spotted),
            player,
            42,
            false
        ));
    }

    #[test]
    fn is_counter_visible_ally_always_visible() {
        let player = cid(0);
        let ally = cid(1);
        let visible: HashSet<CountryId> = [player, ally].into_iter().collect();
        let spotted: HashSet<u16> = [10u16].into_iter().collect();
        // Ally not at war → visible regardless of province
        assert!(is_counter_visible(
            &visible,
            Some(&spotted),
            ally,
            999,
            false
        ));
    }

    #[test]
    fn is_counter_visible_enemy_in_spotted_province() {
        let player = cid(0);
        let enemy = cid(1);
        let visible: HashSet<CountryId> = [player, enemy].into_iter().collect();
        let spotted: HashSet<u16> = [5u16, 10u16, 15u16].into_iter().collect();
        // Enemy at war in spotted province → visible
        assert!(is_counter_visible(
            &visible,
            Some(&spotted),
            enemy,
            10,
            true
        ));
    }

    #[test]
    fn is_counter_visible_enemy_in_unspotted_province_invisible() {
        let player = cid(0);
        let enemy = cid(1);
        let visible: HashSet<CountryId> = [player, enemy].into_iter().collect();
        let spotted: HashSet<u16> = [5u16, 10u16].into_iter().collect();
        // Enemy at war in un-spotted province → invisible
        assert!(!is_counter_visible(
            &visible,
            Some(&spotted),
            enemy,
            99,
            true
        ));
    }

    #[test]
    fn is_counter_visible_enemy_no_spotted_filter_shows_all() {
        let player = cid(0);
        let enemy = cid(1);
        let visible: HashSet<CountryId> = [player, enemy].into_iter().collect();
        // spotted = None → no province filtering, enemy visible everywhere
        assert!(is_counter_visible(&visible, None, enemy, 999, true));
    }

    #[test]
    fn is_counter_visible_not_in_visible_set_invisible() {
        let player = cid(0);
        let stranger = cid(5);
        let visible: HashSet<CountryId> = [player].into_iter().collect();
        let spotted: HashSet<u16> = [42u16].into_iter().collect();
        // Stranger not in visible set → invisible even if province is spotted
        assert!(!is_counter_visible(
            &visible,
            Some(&spotted),
            stranger,
            42,
            false
        ));
    }

    #[test]
    fn is_counter_visible_enemy_not_at_war_treated_as_friendly() {
        let player = cid(0);
        let neutral = cid(3);
        let visible: HashSet<CountryId> = [player, neutral].into_iter().collect();
        let spotted: HashSet<u16> = [].into_iter().collect();
        // Neutral country in visible set, not at war → always visible
        // (e.g. a neighbour you're not fighting)
        assert!(is_counter_visible(
            &visible,
            Some(&spotted),
            neutral,
            999,
            false
        ));
    }
}
