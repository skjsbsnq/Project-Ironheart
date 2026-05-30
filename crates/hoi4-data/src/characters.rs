//! 政治角色（Character）数据。
//!
//! V5 阶段 J.1：解析 `common/characters/*.txt` 子集。本模块**只关心**
//! `country_leader` 角色（用于政治面板顶部肖像）+ name + portraits.civilian.large
//! + ideology。其余角色（political_advisor / theorist / 战地 / 元帅）保留骨架字段
//! 但不展开成专用类型，等后续阶段（J.5 顾问槽位）再扩展。
//!
//! ## 文件格式（vanilla 节选）
//!
//! ```text
//! characters = {
//!     GER_adolf_hitler = {
//!         name = GER_adolf_hitler                # 本地化 key
//!         portraits = {
//!             civilian = { large = GFX_portrait_GER_adolf_hitler }
//!         }
//!         country_leader = {
//!             ideology = nazism                  # 子意识形态
//!             traits = { GER_der_fuhrer }
//!             expire = "1965.1.1.1"
//!             id = -1
//!         }
//!     }
//!     ...
//! }
//! ```
//!
//! ## 当前限制（J.1）
//!
//! - 只读 `country_leader`。`advisor` / `field_marshal` / `corps_commander` / `theorist`
//!   存在与否仅作为 bool 占位。
//! - 不解析 `expire` 日期（vanilla 1965.1.1.1 等）—— 假设玩家局都在 1936-1945，所有
//!   角色都未过期。
//! - 不读 `traits`（J.5 顾问 / 将领特性时再扩）。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use clausewitz_parser::{parse, Value};
use hoi4_paths::PathConfig;

use crate::country::CountryTag;

/// 单个 character 的轻量定义。
///
/// `tag` 由文件名推导（`common/characters/GER.txt` → "GER"）。如果一个文件包含
/// 多个 tag 的角色（vanilla 极少见），仍按文件名归类。
#[derive(Debug, Clone)]
pub struct CharacterDef {
    /// vanilla token 形式（`GER_adolf_hitler`）。全大小写敏感。
    pub key: String,
    /// 所属 tag（从文件名提取）。
    pub tag: CountryTag,
    /// 本地化 key（通常 == key；偶尔形如 `"Joseph Stalin"` 即字面值）。
    pub name_loc_key: String,
    /// `portraits.civilian.large` 的 GFX 名（如 `GFX_portrait_GER_adolf_hitler`）。
    pub portrait_large: Option<String>,
    /// `country_leader.ideology` 的子类型（`nazism` / `stalinism` / `liberalism` …）。
    /// `None` 表示该角色不是 country_leader（顾问 / 将领 / 元帅）。
    pub country_leader_ideology: Option<String>,
    /// Source-order ordinal inside `common/characters/<TAG>.txt`.
    ///
    /// Vanilla relies on definition order when multiple leaders share the same
    /// ideology, e.g. ENG 1936 democratic starts with Stanley Baldwin before
    /// Neville Chamberlain. Keep this stable after global sorting.
    pub source_order: u32,
}

impl CharacterDef {
    /// 是否可作为某国某顶层意识形态的 country_leader。
    ///
    /// `top_ideology` 是 `democratic` / `communism` / `fascism` / `neutrality` 之一；
    /// `ideology_subtypes` 是 `GameData.ideologies[top_ideology].types`（含 `nazism`
    /// 等子类型）。本字符串列表常驻 GameData，不必每次构造。
    pub fn matches_top_ideology(&self, ideology_subtypes: &[String]) -> bool {
        let Some(sub) = self.country_leader_ideology.as_deref() else {
            return false;
        };
        ideology_subtypes.iter().any(|t| t == sub)
    }
}

/// 加载 `common/characters/*.txt` 全部文件，返回 `(characters, by_tag_index)`。
///
/// `by_tag_index[tag] = Vec<characters 索引>`，便于按 tag 检索；characters 本身按
/// `(tag, key)` 字典序稳定排序，保证存档可复现。
pub fn load_characters(game_path: &Path) -> (Vec<CharacterDef>, HashMap<CountryTag, Vec<usize>>) {
    load_characters_from_paths(&PathConfig::with_game_path(game_path))
}

pub fn load_characters_from_paths(
    paths: &PathConfig,
) -> (Vec<CharacterDef>, HashMap<CountryTag, Vec<usize>>) {
    let dirs = data_dirs(paths, "common/characters");
    let mut characters: Vec<CharacterDef> = Vec::new();
    if dirs.is_empty() {
        return (characters, HashMap::new());
    }

    for dir in dirs {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.extension().map_or(false, |e| e == "txt") {
                continue;
            }
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => continue,
            };
            // Tag 通常由文件名推导；但 vanilla 也有 `Warlords_China.txt` 这类共享文件，
            // 其中角色 key 自带真实 TAG 前缀（如 `YUN_long_yun`）。共享文件按 key 前缀归属。
            let tag_str = stem.to_uppercase();
            let file_tag = if tag_str
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                && (2..=4).contains(&tag_str.len())
            {
                Some(CountryTag::new(&tag_str))
            } else {
                None
            };

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            // 极大文件防御：> 4 MB 跳过（vanilla 单 country 至多 ~ 250 KB）
            if content.len() > 4_000_000 {
                continue;
            }
            let block = parse(&content);
            let chars_block = match block.get_block("characters") {
                Some(b) => b,
                None => continue,
            };

            for (source_order, char_entry) in chars_block.entries.iter().enumerate() {
                let Value::Block(cb) = &char_entry.value else {
                    continue;
                };
                let key = char_entry.key.clone();
                let tag = character_tag_from_key(&key).or_else(|| file_tag.clone());
                let Some(tag) = tag else {
                    continue;
                };

                let instance_block = cb.get_all("instance").into_iter().find_map(|v| match v {
                    Value::Block(b) => Some(b),
                    _ => None,
                });
                let data_block = instance_block.unwrap_or(cb);

                // name 可能是 token（`GER_adolf_hitler`）也可能是带引号字面字符串
                // （`"Tsar Joseph I"`）。vanilla 大多数走 loc key。
                let name_loc_key = data_block
                    .get_string("name")
                    .map(strip_quotes)
                    .unwrap_or_else(|| key.clone());

                // portraits.civilian.large
                let mut portrait_large = None;
                if let Some(pb) = data_block.get_block("portraits") {
                    if let Some(cb2) = pb.get_block("civilian") {
                        if let Some(s) = cb2.get_string("large") {
                            portrait_large = Some(strip_quotes(s));
                        }
                    }
                    // 退化：civilian 不存在但 army.large 存在，借用 army
                    if portrait_large.is_none() {
                        if let Some(ab) = pb.get_block("army") {
                            if let Some(s) = ab.get_string("large") {
                                portrait_large = Some(strip_quotes(s));
                            }
                        }
                    }
                }

                // country_leader = { ideology = X ... }
                //
                // 注意：vanilla 中可能存在多个 country_leader 块（不同 ideology），例如
                // 一个角色既是 fascism 又是 democratic 领袖。简化：取第一个。
                let mut leader_ideology = None;
                for v in data_block.get_all("country_leader") {
                    if let Value::Block(lb) = v {
                        if let Some(ide) = lb.get_string("ideology") {
                            leader_ideology = Some(strip_quotes(ide));
                            break;
                        }
                    }
                }

                characters.push(CharacterDef {
                    key,
                    tag: tag.clone(),
                    name_loc_key,
                    portrait_large,
                    country_leader_ideology: leader_ideology,
                    source_order: source_order as u32,
                });
            }
        }
    }

    append_history_country_leaders_from_paths(paths, &mut characters);

    // 排序保证确定性：按 tag.0 然后 key。
    characters.sort_by(|a, b| {
        a.tag
            .0
            .cmp(&b.tag.0)
            .then(a.source_order.cmp(&b.source_order))
            .then(a.key.cmp(&b.key))
    });

    let mut by_tag: HashMap<CountryTag, Vec<usize>> = HashMap::new();
    for (i, c) in characters.iter().enumerate() {
        by_tag.entry(c.tag.clone()).or_default().push(i);
    }

    (characters, by_tag)
}

fn character_tag_from_key(key: &str) -> Option<CountryTag> {
    let prefix = key.split_once('_')?.0;
    if !(2..=4).contains(&prefix.len()) {
        return None;
    }
    if !prefix
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return None;
    }
    Some(CountryTag::new(prefix))
}

fn append_history_country_leaders_from_paths(
    paths: &PathConfig,
    characters: &mut Vec<CharacterDef>,
) {
    let dirs = data_dirs(paths, "history/countries");
    let mut source_order = 1_000_000u32;
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.extension().map_or(false, |e| e == "txt") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let tag_str = stem.split_whitespace().next().unwrap_or("");
            if tag_str.len() != 3 {
                continue;
            }
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let block = parse(&content);
            for value in block.get_all("create_country_leader") {
                let Value::Block(lb) = value else {
                    continue;
                };
                let Some(name) = lb.get_string("name").map(strip_quotes) else {
                    continue;
                };
                let key = format!("{}_{}", tag_str, leader_key_suffix(&name));
                let portrait_large = lb.get_string("picture").map(strip_quotes);
                let leader_ideology = lb.get_string("ideology").map(strip_quotes);
                characters.push(CharacterDef {
                    key,
                    tag: CountryTag::new(tag_str),
                    name_loc_key: name,
                    portrait_large,
                    country_leader_ideology: leader_ideology,
                    source_order,
                });
                source_order = source_order.saturating_add(1);
            }
        }
    }
}

fn data_dirs(paths: &PathConfig, relative: &str) -> Vec<std::path::PathBuf> {
    let mut dirs = paths.find_all(relative);
    dirs.reverse();
    dirs
}

fn leader_key_suffix(name: &str) -> String {
    let suffix: String = name
        .chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    suffix
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn strip_quotes(s: &str) -> String {
    s.trim().trim_matches('"').to_owned()
}

/// 给一个国家的 `ruling_party`（顶层 ideology key，如 `democratic`）和该顶层意识形态的
/// 子类型集合（`["conservatism", "liberalism", ...]`），从 `characters` 中挑出该国
/// 的 country_leader 索引。多人时取第一个（vanilla 1936 起手通常每国每意识形态 1 人）。
///
/// 返回 `None` = 该 tag / ideology 没有 country_leader。
pub fn select_country_leader(
    characters: &[CharacterDef],
    by_tag: &HashMap<CountryTag, Vec<usize>>,
    tag: &CountryTag,
    ideology_subtypes: &[String],
) -> Option<usize> {
    let indices = by_tag.get(tag)?;
    indices
        .iter()
        .copied()
        .find(|&i| characters[i].matches_top_ideology(ideology_subtypes))
}

/// Like [`select_country_leader`], but first honors the country's initial
/// `recruit_character` order from history files.
pub fn select_country_leader_with_recruits(
    characters: &[CharacterDef],
    by_tag: &HashMap<CountryTag, Vec<usize>>,
    tag: &CountryTag,
    ideology_subtypes: &[String],
    recruited_characters: &[String],
) -> Option<usize> {
    let indices = by_tag.get(tag)?;
    for recruited in recruited_characters {
        if let Some(idx) = indices.iter().copied().find(|&i| {
            characters[i].key == *recruited && characters[i].matches_top_ideology(ideology_subtypes)
        }) {
            return Some(idx);
        }
    }
    select_country_leader(characters, by_tag, tag, ideology_subtypes)
        .or_else(|| first_recruited_country_leader(characters, indices, recruited_characters))
        .or_else(|| {
            indices
                .iter()
                .copied()
                .find(|&i| characters[i].country_leader_ideology.is_some())
        })
}

fn first_recruited_country_leader(
    characters: &[CharacterDef],
    indices: &[usize],
    recruited_characters: &[String],
) -> Option<usize> {
    for recruited in recruited_characters {
        if let Some(idx) = indices.iter().copied().find(|&i| {
            characters[i].key == *recruited && characters[i].country_leader_ideology.is_some()
        }) {
            return Some(idx);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_character(key: &str, ideology: &str, source_order: u32) -> CharacterDef {
        CharacterDef {
            key: key.into(),
            tag: CountryTag::new("ENG"),
            name_loc_key: key.into(),
            portrait_large: None,
            country_leader_ideology: Some(ideology.into()),
            source_order,
        }
    }

    #[test]
    fn matches_top_ideology_basic() {
        let c = CharacterDef {
            key: "GER_adolf_hitler".into(),
            tag: CountryTag::new("GER"),
            name_loc_key: "GER_adolf_hitler".into(),
            portrait_large: Some("GFX_portrait_GER_adolf_hitler".into()),
            country_leader_ideology: Some("nazism".into()),
            source_order: 0,
        };
        let fascism_subtypes = vec![
            "nazism".to_owned(),
            "fascism_ideology".to_owned(),
            "falangism".to_owned(),
        ];
        assert!(c.matches_top_ideology(&fascism_subtypes));
        let democratic_subtypes = vec!["liberalism".to_owned()];
        assert!(!c.matches_top_ideology(&democratic_subtypes));
    }

    #[test]
    fn non_leader_never_matches() {
        let c = CharacterDef {
            key: "ENG_alan_brooke".into(),
            tag: CountryTag::new("ENG"),
            name_loc_key: "ENG_alan_brooke".into(),
            portrait_large: None,
            country_leader_ideology: None, // 仅将领，非 country_leader
            source_order: 0,
        };
        assert!(!c.matches_top_ideology(&["liberalism".to_owned()]));
    }

    #[test]
    fn select_picks_first_match() {
        let chars = vec![
            CharacterDef {
                key: "X_alpha".into(),
                tag: CountryTag::new("XXX"),
                name_loc_key: "X_alpha".into(),
                portrait_large: None,
                country_leader_ideology: Some("liberalism".into()),
                source_order: 0,
            },
            CharacterDef {
                key: "X_beta".into(),
                tag: CountryTag::new("XXX"),
                name_loc_key: "X_beta".into(),
                portrait_large: None,
                country_leader_ideology: Some("nazism".into()),
                source_order: 1,
            },
        ];
        let mut by_tag: HashMap<CountryTag, Vec<usize>> = HashMap::new();
        by_tag.insert(CountryTag::new("XXX"), vec![0, 1]);

        // democratic 顶层 → liberalism 是子类型 → 命中 alpha
        let idx = select_country_leader(
            &chars,
            &by_tag,
            &CountryTag::new("XXX"),
            &["liberalism".to_owned(), "conservatism".to_owned()],
        );
        assert_eq!(idx, Some(0));

        // fascism 顶层 → nazism 是子类型 → 命中 beta
        let idx = select_country_leader(
            &chars,
            &by_tag,
            &CountryTag::new("XXX"),
            &["nazism".to_owned()],
        );
        assert_eq!(idx, Some(1));

        // unknown 顶层 → 全 None
        let idx = select_country_leader(
            &chars,
            &by_tag,
            &CountryTag::new("XXX"),
            &["nonexistent".to_owned()],
        );
        assert_eq!(idx, None);
    }

    #[test]
    fn select_preserves_source_order_after_global_sort() {
        let mut chars = vec![
            CharacterDef {
                key: "ENG_neville_chamberlain".into(),
                tag: CountryTag::new("ENG"),
                name_loc_key: "ENG_neville_chamberlain".into(),
                portrait_large: None,
                country_leader_ideology: Some("liberalism".into()),
                source_order: 184,
            },
            CharacterDef {
                key: "ENG_stanley_baldwin".into(),
                tag: CountryTag::new("ENG"),
                name_loc_key: "ENG_stanley_baldwin".into(),
                portrait_large: None,
                country_leader_ideology: Some("liberalism".into()),
                source_order: 79,
            },
        ];
        chars.sort_by(|a, b| {
            a.tag
                .0
                .cmp(&b.tag.0)
                .then(a.source_order.cmp(&b.source_order))
                .then(a.key.cmp(&b.key))
        });
        let mut by_tag: HashMap<CountryTag, Vec<usize>> = HashMap::new();
        by_tag.insert(CountryTag::new("ENG"), vec![0, 1]);

        let idx = select_country_leader(
            &chars,
            &by_tag,
            &CountryTag::new("ENG"),
            &["liberalism".to_owned()],
        )
        .unwrap();

        assert_eq!(chars[idx].key, "ENG_stanley_baldwin");
    }

    #[test]
    fn select_prefers_first_recruited_matching_leader() {
        let chars = vec![
            test_character("ENG_stanley_baldwin", "liberalism", 0),
            test_character("ENG_winston_churchill", "conservatism", 1),
            test_character("ENG_neville_chamberlain", "liberalism", 2),
        ];
        let mut by_tag: HashMap<CountryTag, Vec<usize>> = HashMap::new();
        by_tag.insert(CountryTag::new("ENG"), vec![0, 1, 2]);

        let idx = select_country_leader_with_recruits(
            &chars,
            &by_tag,
            &CountryTag::new("ENG"),
            &["conservatism".to_owned(), "liberalism".to_owned()],
            &[
                "ENG_stanley_baldwin".to_owned(),
                "ENG_winston_churchill".to_owned(),
                "ENG_neville_chamberlain".to_owned(),
            ],
        )
        .unwrap();

        assert_eq!(chars[idx].key, "ENG_stanley_baldwin");
    }

    #[test]
    fn load_characters_reads_instance_wrapped_leader() {
        let root = std::env::temp_dir().join(format!(
            "ironheart-character-instance-test-{}",
            std::process::id()
        ));
        let dir = root.join("common/characters");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("TUR.txt"),
            r#"
characters = {
    TUR_mustafa_kemal_ataturk = {
        instance = {
            allowed = { has_dlc = "Battle for the Bosporus" }
            name = TUR_mustafa_kemal_ataturk
            portraits = { civilian = { large = GFX_portrait_TUR_mustafa_kemal_ataturk } }
            country_leader = { ideology = despotism id = -1 }
        }
    }
}
"#,
        )
        .unwrap();

        let (characters, by_tag) = load_characters(&root);
        let leader = characters
            .iter()
            .find(|character| character.key == "TUR_mustafa_kemal_ataturk")
            .unwrap();
        assert_eq!(leader.name_loc_key, "TUR_mustafa_kemal_ataturk");
        assert_eq!(
            leader.portrait_large.as_deref(),
            Some("GFX_portrait_TUR_mustafa_kemal_ataturk")
        );
        assert_eq!(leader.country_leader_ideology.as_deref(), Some("despotism"));
        assert!(by_tag.contains_key(&CountryTag::new("TUR")));

        let _ = std::fs::remove_dir_all(root);
    }
}
