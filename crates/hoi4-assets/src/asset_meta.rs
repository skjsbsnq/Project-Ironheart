//! Phase 2.8：`.asset` 元数据解析。
//!
//! `.asset` 文件是 Clausewitz Script 格式，描述音乐/模型/粒子的 metadata。
//! 例如 `music/*.asset` 描述音乐曲目的 name / file / volume / always 等。
//!
//! 本模块解析 `.asset` 文件并索引到 `AssetMetaIndex` 供 audio / mesh / fx 查询。

use std::collections::HashMap;
use std::path::Path;

use clausewitz_parser::{parse, Block, Value};

use crate::db::AssetDb;
use crate::error::AssetError;

/// 音乐条目。
#[derive(Debug, Clone)]
pub struct MusicAsset {
    pub name: String,
    pub file: String,
    pub volume: f32,
    pub always: bool,
    /// 可选：所属 music_station。
    pub music_station: Option<String>,
}

/// 模型条目（pdxmesh 引用）。
#[derive(Debug, Clone)]
pub struct ModelAsset {
    pub name: String,
    pub file: String,
    pub scale: f32,
}

/// `.asset` 元数据索引。
#[derive(Debug, Default)]
pub struct AssetMetaIndex {
    pub music: HashMap<String, MusicAsset>,
    pub models: HashMap<String, ModelAsset>,
    pub files_loaded: usize,
    pub warnings: Vec<String>,
}

impl AssetMetaIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// 加载单个 .asset 文件。
    pub fn load_file(
        &mut self,
        db: &impl AssetDb,
        relative: impl AsRef<Path>,
    ) -> Result<usize, AssetError> {
        let rel = relative.as_ref();
        let bytes = db.open(rel)?;
        let text = String::from_utf8_lossy(&bytes);
        let block = parse(&text);
        let added = self.absorb_block(&block);
        Ok(added)
    }

    /// 批量加载目录下所有 `.asset` 文件。
    pub fn load_dir(
        &mut self,
        db: &impl AssetDb,
        dir_relative: impl AsRef<Path>,
    ) -> Result<(), AssetError> {
        let dir_rel = dir_relative.as_ref();
        let candidate_dirs = db.list(dir_rel);

        for phys_dir in &candidate_dirs {
            let entries = match std::fs::read_dir(phys_dir) {
                Ok(e) => e,
                Err(e) => {
                    self.warnings
                        .push(format!("read_dir {}: {}", phys_dir.display(), e));
                    continue;
                }
            };
            for ent in entries.flatten() {
                let p = ent.path();
                if p.extension().and_then(|s| s.to_str()) != Some("asset") {
                    continue;
                }
                if let Some(file_name) = p.file_name() {
                    let rel = dir_rel.join(file_name);
                    match self.load_file(db, &rel) {
                        Ok(_) => self.files_loaded += 1,
                        Err(e) => self.warnings.push(format!("{}", e)),
                    }
                }
            }
        }
        Ok(())
    }

    fn absorb_block(&mut self, block: &Block) -> usize {
        let mut added = 0;

        for entry in &block.entries {
            let lk = entry.key.to_ascii_lowercase();
            match lk.as_str() {
                "music" | "song" => {
                    if let Value::Block(b) = &entry.value {
                        if let Some(m) = parse_music(b) {
                            self.music.insert(m.name.clone(), m);
                            added += 1;
                        }
                    }
                }
                "objecttypes" | "pdxmesh" => {
                    if let Value::Block(b) = &entry.value {
                        if let Some(m) = parse_model(b) {
                            self.models.insert(m.name.clone(), m);
                            added += 1;
                        }
                        // 递归（objectTypes 是容器）
                        added += self.absorb_block(b);
                    }
                }
                _ => {
                    // 递归进未知容器（某些 .asset 有嵌套结构）
                    if let Value::Block(b) = &entry.value {
                        added += self.absorb_block(b);
                    }
                }
            }
        }
        added
    }
}

fn parse_music(b: &Block) -> Option<MusicAsset> {
    let name = b.get_string("name")?.to_string();
    let file = b.get_string("file").unwrap_or("").to_string();
    let volume = b.get_float("volume").unwrap_or(0.5) as f32;
    let always = b.get_bool("always").unwrap_or(false);
    let music_station = b.get_string("music_station").map(|s| s.to_string());
    Some(MusicAsset {
        name,
        file,
        volume,
        always,
        music_station,
    })
}

fn parse_model(b: &Block) -> Option<ModelAsset> {
    let name = b.get_string("name")?.to_string();
    let file = b.get_string("file").unwrap_or("").to_string();
    let scale = b.get_float("scale").unwrap_or(1.0) as f32;
    Some(ModelAsset { name, file, scale })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_music_asset() {
        let input = r#"music = {
            name = "main_menu_music"
            file = "music/main_menu.ogg"
            volume = 0.65
            always = yes
        }"#;
        let block = parse(input);
        let mut idx = AssetMetaIndex::new();
        let n = idx.absorb_block(&block);
        assert_eq!(n, 1);
        let m = idx.music.get("main_menu_music").unwrap();
        assert_eq!(m.file, "music/main_menu.ogg");
        assert_eq!(m.volume, 0.65);
        assert!(m.always);
    }

    #[test]
    fn parse_model_asset() {
        let input = r#"objectTypes = {
            pdxmesh = {
                name = "building_factory"
                file = "gfx/models/buildings/factory.mesh"
                scale = 1.5
            }
        }"#;
        let block = parse(input);
        let mut idx = AssetMetaIndex::new();
        let n = idx.absorb_block(&block);
        assert_eq!(n, 1);
        let m = idx.models.get("building_factory").unwrap();
        assert_eq!(m.file, "gfx/models/buildings/factory.mesh");
        assert_eq!(m.scale, 1.5);
    }
}
