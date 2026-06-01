//! Clausewitz `.gfx` entity / pdxmesh metadata used by map object rendering.
//!
//! This is intentionally narrower than the old GUI sprite parser: Phase 9 only
//! needs `objectTypes = { pdxmesh = { ... } entity = { ... } }` records so the
//! renderer can resolve vanilla entity names to `.mesh` files and material
//! overrides.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use clausewitz_parser::{parse, Block, Value};

use crate::db::AssetDb;
use crate::error::AssetError;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GfxMeshSettings {
    pub name: Option<String>,
    pub index: Option<u32>,
    pub shader: Option<String>,
    pub texture_diffuse: Option<String>,
    pub texture_normal: Option<String>,
    pub texture_specular: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GfxMeshDef {
    pub name: String,
    pub file: String,
    pub scale: f32,
    pub meshsettings: Vec<GfxMeshSettings>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GfxEntityDef {
    pub name: String,
    pub pdxmesh: String,
    pub scale: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PostEffectValues {
    pub name: String,
    pub inherit: Option<String>,
    pub lut: Option<String>,
    pub tonemap_middlegrey: Option<f32>,
    pub bloom_width: Option<f32>,
    pub bloom_scale: Option<f32>,
    pub bright_threshold: Option<f32>,
    pub hdr_min_adjustment: Option<f32>,
    pub hdr_max_adjustment: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PostEffectHeightVolume {
    pub name: String,
    pub values_day: Option<String>,
    pub values_night: Option<String>,
    pub height: f32,
    pub fade_distance: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PostEffectVolume {
    pub name: String,
    pub values_day: Option<String>,
    pub values_night: Option<String>,
    pub values_day_winter: Option<String>,
    pub values_night_winter: Option<String>,
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub fade_distance: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PostEffectVolumeIndex {
    pub values: HashMap<String, PostEffectValues>,
    pub height_volumes: Vec<PostEffectHeightVolume>,
    pub volumes: Vec<PostEffectVolume>,
    pub files_loaded: usize,
    pub warnings: Vec<String>,
}

impl PostEffectVolumeIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(db: &impl AssetDb) -> Result<Self, AssetError> {
        let mut index = Self::new();
        index.load_file(db, "gfx/posteffect_volumes.txt")?;
        Ok(index)
    }

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
        self.files_loaded += 1;
        Ok(added)
    }

    pub fn default_values(&self) -> Option<&PostEffectValues> {
        self.values
            .get("default")
            .or_else(|| self.values.get("standard"))
    }

    pub fn default_lut(&self) -> Option<&str> {
        self.resolved_lut_for_values("default")
            .or_else(|| self.resolved_lut_for_values("standard"))
    }

    pub fn lut_paths(&self) -> Vec<String> {
        let mut paths = BTreeSet::new();
        for name in self.values.keys() {
            if let Some(path) = self.resolved_lut_for_values(name) {
                paths.insert(normalize_path(path));
            }
        }
        paths.into_iter().collect()
    }

    pub fn effective_values(&self, name: &str) -> Option<PostEffectValues> {
        self.effective_values_inner(name, &mut HashSet::new())
    }

    pub fn resolved_lut_for_values(&self, name: &str) -> Option<&str> {
        self.values.get(name).and_then(|values| {
            values.lut.as_deref().or_else(|| {
                values
                    .inherit
                    .as_deref()
                    .and_then(|parent| self.resolved_lut_for_values(parent))
            })
        })
    }

    pub fn load_lut_images(
        &self,
        db: &impl AssetDb,
    ) -> Vec<Result<(String, crate::TgaImage), AssetError>> {
        self.lut_paths()
            .into_iter()
            .map(|path| match crate::load_tga(db, &path) {
                Ok(image) => Ok((path, image)),
                Err(err) => Err(err.with_referrer_path("gfx/posteffect_volumes.txt")),
            })
            .collect()
    }

    fn absorb_block(&mut self, block: &Block) -> usize {
        let mut added = 0;
        for entry in &block.entries {
            let key = entry.key.to_ascii_lowercase();
            match (key.as_str(), &entry.value) {
                ("posteffect_values", Value::Block(b)) => {
                    if let Some(values) = parse_posteffect_values(b) {
                        self.values.insert(values.name.clone(), values);
                        added += 1;
                    }
                }
                ("posteffect_volumes", Value::Block(b)) => {
                    added += self.absorb_posteffect_volumes(b);
                }
                (_, Value::Block(b)) => {
                    added += self.absorb_block(b);
                }
                _ => {}
            }
        }
        added
    }

    fn absorb_posteffect_volumes(&mut self, block: &Block) -> usize {
        let mut added = 0;
        for entry in &block.entries {
            let key = entry.key.to_ascii_lowercase();
            match (key.as_str(), &entry.value) {
                ("posteffect_height_volume", Value::Block(b)) => {
                    if let Some(volume) = parse_posteffect_height_volume(b) {
                        self.height_volumes.push(volume);
                        added += 1;
                    }
                }
                ("posteffect_volume", Value::Block(b)) => {
                    if let Some(volume) = parse_posteffect_volume(b) {
                        self.volumes.push(volume);
                        added += 1;
                    }
                }
                (_, Value::Block(b)) => {
                    added += self.absorb_posteffect_volumes(b);
                }
                _ => {}
            }
        }
        added
    }

    fn effective_values_inner(
        &self,
        name: &str,
        visited: &mut HashSet<String>,
    ) -> Option<PostEffectValues> {
        if !visited.insert(name.to_string()) {
            return self.values.get(name).cloned();
        }
        let own = self.values.get(name)?.clone();
        let mut resolved = own
            .inherit
            .as_deref()
            .and_then(|parent| self.effective_values_inner(parent, visited))
            .unwrap_or_default();
        resolved.name = own.name.clone();
        resolved.inherit = own.inherit.clone();
        if own.lut.is_some() {
            resolved.lut = own.lut;
        }
        if own.tonemap_middlegrey.is_some() {
            resolved.tonemap_middlegrey = own.tonemap_middlegrey;
        }
        if own.bloom_width.is_some() {
            resolved.bloom_width = own.bloom_width;
        }
        if own.bloom_scale.is_some() {
            resolved.bloom_scale = own.bloom_scale;
        }
        if own.bright_threshold.is_some() {
            resolved.bright_threshold = own.bright_threshold;
        }
        if own.hdr_min_adjustment.is_some() {
            resolved.hdr_min_adjustment = own.hdr_min_adjustment;
        }
        if own.hdr_max_adjustment.is_some() {
            resolved.hdr_max_adjustment = own.hdr_max_adjustment;
        }
        visited.remove(name);
        Some(resolved)
    }
}

#[derive(Debug, Default)]
pub struct GfxIndex {
    pub meshes: HashMap<String, GfxMeshDef>,
    pub entities: HashMap<String, GfxEntityDef>,
    pub files_loaded: usize,
    pub warnings: Vec<String>,
}

impl GfxIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_file(
        &mut self,
        db: &impl AssetDb,
        relative: impl AsRef<Path>,
    ) -> Result<usize, AssetError> {
        let rel = relative.as_ref();
        let bytes = db.open(rel)?;
        let text = String::from_utf8_lossy(&bytes);
        let block = parse(&text);
        Ok(self.absorb_block(&block))
    }

    /// Load all `.gfx` files under `dir_relative`.
    ///
    /// `AssetDb::list` returns high-priority mod directories first and vanilla
    /// last. We read them in reverse physical order so later inserts preserve
    /// normal override semantics: vanilla base first, mods on top.
    pub fn load_dir(
        &mut self,
        db: &impl AssetDb,
        dir_relative: impl AsRef<Path>,
    ) -> Result<(), AssetError> {
        let dir_rel = dir_relative.as_ref();
        let candidate_dirs = db.list(dir_rel);
        for phys_dir in candidate_dirs.iter().rev() {
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
                if p.extension().and_then(|s| s.to_str()) != Some("gfx") {
                    continue;
                }
                match std::fs::read_to_string(&p) {
                    Ok(text) => {
                        let block = parse(&text);
                        self.absorb_block(&block);
                        self.files_loaded += 1;
                    }
                    Err(e) => self.warnings.push(format!("{}: {}", p.display(), e)),
                }
            }
        }
        Ok(())
    }

    pub fn mesh_for_entity_or_mesh(&self, name: &str) -> Option<&GfxMeshDef> {
        if let Some(entity) = self.entities.get(name) {
            return self.meshes.get(&entity.pdxmesh);
        }
        self.meshes.get(name)
    }

    pub fn first_mesh_for_names<'a>(
        &'a self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Option<&'a GfxMeshDef> {
        names
            .into_iter()
            .find_map(|name| self.mesh_for_entity_or_mesh(name))
    }

    fn absorb_block(&mut self, block: &Block) -> usize {
        let mut added = 0;
        for entry in &block.entries {
            let key = entry.key.to_ascii_lowercase();
            match (key.as_str(), &entry.value) {
                ("pdxmesh", Value::Block(b)) => {
                    if let Some(mesh) = parse_pdxmesh(b) {
                        self.meshes.insert(mesh.name.clone(), mesh);
                        added += 1;
                    }
                    added += self.absorb_block(b);
                }
                ("entity", Value::Block(b)) => {
                    if let Some(entity) = parse_entity(b) {
                        self.entities.insert(entity.name.clone(), entity);
                        added += 1;
                    }
                    added += self.absorb_block(b);
                }
                (_, Value::Block(b)) => {
                    added += self.absorb_block(b);
                }
                _ => {}
            }
        }
        added
    }
}

fn parse_pdxmesh(b: &Block) -> Option<GfxMeshDef> {
    let name = block_string_ci(b, "name")?.to_string();
    let file = normalize_path(block_string_ci(b, "file").unwrap_or(""));
    let scale = block_float_ci(b, "scale").unwrap_or(1.0) as f32;
    let mut meshsettings = Vec::new();
    for entry in &b.entries {
        if !entry.key.eq_ignore_ascii_case("meshsettings") {
            continue;
        }
        if let Value::Block(ms) = &entry.value {
            meshsettings.push(parse_meshsettings(ms));
        }
    }
    Some(GfxMeshDef {
        name,
        file,
        scale,
        meshsettings,
    })
}

fn parse_entity(b: &Block) -> Option<GfxEntityDef> {
    let name = block_string_ci(b, "name")?.to_string();
    let pdxmesh = block_string_ci(b, "pdxmesh")?.to_string();
    let scale = block_float_ci(b, "scale").unwrap_or(1.0) as f32;
    Some(GfxEntityDef {
        name,
        pdxmesh,
        scale,
    })
}

fn parse_posteffect_values(b: &Block) -> Option<PostEffectValues> {
    Some(PostEffectValues {
        name: block_string_ci(b, "name")?.to_string(),
        inherit: block_string_ci(b, "inherit").map(ToOwned::to_owned),
        lut: block_string_ci(b, "lut").map(normalize_path),
        tonemap_middlegrey: block_float_ci(b, "tonemap_middlegrey").map(|v| v as f32),
        bloom_width: block_float_ci(b, "BLOOM_WIDTH").map(|v| v as f32),
        bloom_scale: block_float_ci(b, "BLOOM_SCALE").map(|v| v as f32),
        bright_threshold: block_float_ci(b, "BRIGHT_THRESHOLD").map(|v| v as f32),
        hdr_min_adjustment: block_float_ci(b, "hdr_min_adjustment").map(|v| v as f32),
        hdr_max_adjustment: block_float_ci(b, "hdr_max_adjustment").map(|v| v as f32),
    })
}

fn parse_posteffect_height_volume(b: &Block) -> Option<PostEffectHeightVolume> {
    Some(PostEffectHeightVolume {
        name: block_string_ci(b, "name")?.to_string(),
        values_day: block_string_ci(b, "posteffect_values_day").map(ToOwned::to_owned),
        values_night: block_string_ci(b, "posteffect_values_night").map(ToOwned::to_owned),
        height: block_float_ci(b, "height").unwrap_or(0.0) as f32,
        fade_distance: block_float_ci(b, "fade_distance").unwrap_or(0.0) as f32,
    })
}

fn parse_posteffect_volume(b: &Block) -> Option<PostEffectVolume> {
    Some(PostEffectVolume {
        name: block_string_ci(b, "name")?.to_string(),
        values_day: block_string_ci(b, "posteffect_values_day").map(ToOwned::to_owned),
        values_night: block_string_ci(b, "posteffect_values_night").map(ToOwned::to_owned),
        values_day_winter: block_string_ci(b, "posteffect_values_day_winter")
            .map(ToOwned::to_owned),
        values_night_winter: block_string_ci(b, "posteffect_values_night_winter")
            .map(ToOwned::to_owned),
        position: block_vec3_ci(b, "position").unwrap_or([0.0; 3]),
        size: block_vec3_ci(b, "size").unwrap_or([0.0; 3]),
        fade_distance: block_float_ci(b, "fade_distance").unwrap_or(0.0) as f32,
    })
}

fn parse_meshsettings(b: &Block) -> GfxMeshSettings {
    GfxMeshSettings {
        name: block_string_ci(b, "name").map(ToOwned::to_owned),
        index: block_int_ci(b, "index").map(|v| v.max(0) as u32),
        shader: block_string_ci(b, "shader").map(ToOwned::to_owned),
        texture_diffuse: block_string_ci(b, "texture_diffuse").map(normalize_path),
        texture_normal: block_string_ci(b, "texture_normal").map(normalize_path),
        texture_specular: block_string_ci(b, "texture_specular").map(normalize_path),
    }
}

fn block_vec3_ci(b: &Block, key: &str) -> Option<[f32; 3]> {
    b.entries.iter().find_map(|entry| {
        if !entry.key.eq_ignore_ascii_case(key) {
            return None;
        }
        match &entry.value {
            Value::Block(block) => {
                let mut out = [0.0f32; 3];
                for (idx, value) in block.values.iter().take(3).enumerate() {
                    out[idx] = match value {
                        Value::Float(v) => *v as f32,
                        Value::Integer(v) => *v as f32,
                        _ => return None,
                    };
                }
                (block.values.len() >= 3).then_some(out)
            }
            _ => None,
        }
    })
}

fn block_string_ci<'a>(b: &'a Block, key: &str) -> Option<&'a str> {
    b.entries.iter().find_map(|entry| {
        if !entry.key.eq_ignore_ascii_case(key) {
            return None;
        }
        match &entry.value {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    })
}

fn block_float_ci(b: &Block, key: &str) -> Option<f64> {
    b.entries.iter().find_map(|entry| {
        if !entry.key.eq_ignore_ascii_case(key) {
            return None;
        }
        match &entry.value {
            Value::Float(v) => Some(*v),
            Value::Integer(v) => Some(*v as f64),
            _ => None,
        }
    })
}

fn block_int_ci(b: &Block, key: &str) -> Option<i64> {
    b.entries.iter().find_map(|entry| {
        if !entry.key.eq_ignore_ascii_case(key) {
            return None;
        }
        match &entry.value {
            Value::Integer(v) => Some(*v),
            _ => None,
        }
    })
}

fn normalize_path(path: impl AsRef<str>) -> String {
    path.as_ref().replace('\\', "/")
}

#[allow(dead_code)]
fn rel_join(dir_relative: &Path, file_name: impl Into<PathBuf>) -> PathBuf {
    dir_relative.join(file_name.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pdxmesh_meshsettings() {
        let block = parse(
            r#"
            objectTypes = {
                pdxmesh = {
                    name = "building_anti_air_building"
                    file = "gfx\models\buildings\88aagun.mesh"
                    scale = 0.18
                    meshsettings = {
                        name = "gunShape"
                        index = 0
                        texture_diffuse = "88_aagun_d.dds"
                        texture_normal = "88_aagun_n.dds"
                        texture_specular = "88_aagun_s.dds"
                        shader = "PdxMeshAdvancedSnow"
                    }
                }
            }
            "#,
        );
        let mut idx = GfxIndex::new();
        let added = idx.absorb_block(&block);
        assert_eq!(added, 1);
        let mesh = idx.meshes.get("building_anti_air_building").unwrap();
        assert_eq!(mesh.file, "gfx/models/buildings/88aagun.mesh");
        assert_eq!(mesh.scale, 0.18);
        assert_eq!(mesh.meshsettings.len(), 1);
        assert_eq!(
            mesh.meshsettings[0].texture_diffuse.as_deref(),
            Some("88_aagun_d.dds")
        );
        assert_eq!(
            mesh.meshsettings[0].shader.as_deref(),
            Some("PdxMeshAdvancedSnow")
        );
    }

    #[test]
    fn resolves_entity_to_mesh() {
        let block = parse(
            r#"
            objectTypes = {
                pdxmesh = {
                    name = "test_mesh"
                    file = "gfx/models/buildings/factory.mesh"
                }
                entity = {
                    name = "test_entity"
                    pdxmesh = "test_mesh"
                }
            }
            "#,
        );
        let mut idx = GfxIndex::new();
        idx.absorb_block(&block);
        let mesh = idx.mesh_for_entity_or_mesh("test_entity").unwrap();
        assert_eq!(mesh.file, "gfx/models/buildings/factory.mesh");
        assert!(idx.mesh_for_entity_or_mesh("test_mesh").is_some());
    }

    #[test]
    fn parses_posteffect_values_and_volumes() {
        let block = parse(
            r#"
            posteffect_values = {
                name = default
                inherit = standard
                lut = "gfx/world/colorcorrection_standard.tga"
                BLOOM_WIDTH = 1.5
                tonemap_middlegrey = 0.55
            }
            posteffect_volumes = {
                posteffect_height_volume = {
                    name = "Base"
                    posteffect_values_day = default
                    posteffect_values_night = default_night
                    height = 0
                    fade_distance = 0
                }
                posteffect_volume = {
                    name = BlueWater
                    posteffect_values_day = blue_water
                    posteffect_values_night = blue_water_night
                    position = { 100.0 200.0 300.0 }
                    size = { 10.0 20.0 30.0 }
                    fade_distance = 75.0
                }
            }
            "#,
        );
        let mut idx = PostEffectVolumeIndex::new();
        let added = idx.absorb_block(&block);
        assert_eq!(added, 3);
        assert_eq!(
            idx.default_lut(),
            Some("gfx/world/colorcorrection_standard.tga")
        );
        assert_eq!(
            idx.lut_paths(),
            vec!["gfx/world/colorcorrection_standard.tga"]
        );
        assert_eq!(
            idx.height_volumes[0].values_night.as_deref(),
            Some("default_night")
        );
        assert_eq!(idx.volumes[0].position, [100.0, 200.0, 300.0]);
        assert_eq!(idx.volumes[0].size, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn posteffect_values_resolve_inherited_fields() {
        let block = parse(
            r#"
            posteffect_values = {
                name = standard
                lut = "gfx/world/colorcorrection.tga"
                BLOOM_WIDTH = 1.5
                tonemap_middlegrey = 0.55
            }
            posteffect_values = {
                name = child
                inherit = standard
                lut = "gfx/world/colorcorrection_child.tga"
            }
            posteffect_values = {
                name = inherited_lut
                inherit = standard
                tonemap_middlegrey = 0.25
            }
            "#,
        );
        let mut idx = PostEffectVolumeIndex::new();
        idx.absorb_block(&block);

        let child = idx.effective_values("child").unwrap();
        assert_eq!(
            child.lut.as_deref(),
            Some("gfx/world/colorcorrection_child.tga")
        );
        assert_eq!(child.bloom_width, Some(1.5));
        assert_eq!(child.tonemap_middlegrey, Some(0.55));

        let inherited = idx.effective_values("inherited_lut").unwrap();
        assert_eq!(
            inherited.lut.as_deref(),
            Some("gfx/world/colorcorrection.tga")
        );
        assert_eq!(inherited.tonemap_middlegrey, Some(0.25));
        assert_eq!(
            idx.resolved_lut_for_values("inherited_lut"),
            Some("gfx/world/colorcorrection.tga")
        );
    }
}
