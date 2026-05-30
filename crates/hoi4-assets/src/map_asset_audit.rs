use std::collections::BTreeMap;

use crate::{dds_upload_plan, DdsImage, MapResEntry, MapResRole, VanillaMapSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MapAssetQuality {
    Valid,
    Degraded,
    FallbackOnly,
    InvalidForVisualReview,
}

impl MapAssetQuality {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "Valid",
            Self::Degraded => "Degraded",
            Self::FallbackOnly => "FallbackOnly",
            Self::InvalidForVisualReview => "InvalidForVisualReview",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MapAssetRequirement {
    StartupRequired,
    BaseMapRequired,
    HighQuality,
    Optional,
}

impl MapAssetRequirement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StartupRequired => "StartupRequired",
            Self::BaseMapRequired => "BaseMapRequired",
            Self::HighQuality => "HighQuality",
            Self::Optional => "Optional",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MapAssetFallbackPolicy {
    StartupBlocked,
    VisualReviewBlocked,
    DegradedAllowed,
    OptionalAllowed,
}

impl MapAssetFallbackPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StartupBlocked => "StartupBlocked",
            Self::VisualReviewBlocked => "VisualReviewBlocked",
            Self::DegradedAllowed => "DegradedAllowed",
            Self::OptionalAllowed => "OptionalAllowed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapAssetFileKind {
    Bmp,
    Dds,
}

impl MapAssetFileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bmp => "Bmp",
            Self::Dds => "Dds",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapAssetMipStatus {
    NotDds,
    Missing,
    Complete,
    Empty,
    ParseFailed,
    UnsupportedFormat,
}

impl MapAssetMipStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotDds => "NotDds",
            Self::Missing => "Missing",
            Self::Complete => "Complete",
            Self::Empty => "Empty",
            Self::ParseFailed => "ParseFailed",
            Self::UnsupportedFormat => "UnsupportedFormat",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MapAssetDdsInfo {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub source_mips: u32,
    pub upload_mips: u32,
    pub block_width: u32,
    pub block_height: u32,
    pub bytes_per_block: u32,
}

#[derive(Debug, Clone)]
pub struct MapAssetAuditEntry {
    pub role: MapResRole,
    pub category: &'static str,
    pub relative_path: String,
    pub requirement: MapAssetRequirement,
    pub fallback_policy: MapAssetFallbackPolicy,
    pub file_kind: MapAssetFileKind,
    pub loaded: bool,
    pub usable: bool,
    pub is_srgb: bool,
    pub mip_status: MapAssetMipStatus,
    pub dds: Option<MapAssetDdsInfo>,
    pub fallback_reason: Option<String>,
    pub fallback_makes_visual_review_invalid: bool,
}

impl MapAssetAuditEntry {
    pub fn needs_fallback(&self) -> bool {
        self.fallback_reason.is_some()
    }
}

#[derive(Debug, Clone, Default)]
pub struct MapAssetCategoryStats {
    pub total: usize,
    pub loaded: usize,
    pub missing: usize,
    pub fallback: usize,
}

#[derive(Debug, Clone)]
pub struct MapAssetAudit {
    pub total: usize,
    pub loaded: usize,
    pub missing: usize,
    pub fallback: usize,
    pub quality: MapAssetQuality,
    pub entries: Vec<MapAssetAuditEntry>,
    pub category_stats: BTreeMap<&'static str, MapAssetCategoryStats>,
    pub fallback_paths: Vec<String>,
    pub invalid_visual_review_paths: Vec<String>,
}

impl MapAssetAudit {
    pub fn from_map_set(map_set: &VanillaMapSet) -> Self {
        let mut entries = Vec::with_capacity(map_set.entries.len());
        let mut category_stats: BTreeMap<&'static str, MapAssetCategoryStats> = BTreeMap::new();
        let mut fallback_paths = Vec::new();
        let mut invalid_visual_review_paths = Vec::new();

        for entry in &map_set.entries {
            let audit_entry = build_audit_entry(entry);
            let stats = category_stats.entry(audit_entry.category).or_default();
            stats.total += 1;
            if audit_entry.loaded {
                stats.loaded += 1;
            } else {
                stats.missing += 1;
            }
            if audit_entry.needs_fallback() {
                stats.fallback += 1;
                fallback_paths.push(audit_entry.relative_path.clone());
                if audit_entry.fallback_makes_visual_review_invalid {
                    invalid_visual_review_paths.push(audit_entry.relative_path.clone());
                }
            }
            entries.push(audit_entry);
        }

        let total = entries.len();
        let loaded = entries.iter().filter(|entry| entry.loaded).count();
        let missing = total.saturating_sub(loaded);
        let fallback = entries
            .iter()
            .filter(|entry| entry.needs_fallback())
            .count();
        let quality = quality_for(&entries, total, loaded, fallback);

        Self {
            total,
            loaded,
            missing,
            fallback,
            quality,
            entries,
            category_stats,
            fallback_paths,
            invalid_visual_review_paths,
        }
    }

    pub fn can_use_for_visual_review(&self) -> bool {
        matches!(
            self.quality,
            MapAssetQuality::Valid | MapAssetQuality::Degraded
        )
    }

    pub fn summary_line(&self) -> String {
        format!(
            "[map-asset-audit] {}/{} loaded, {} missing, {} fallback, quality={}",
            self.loaded,
            self.total,
            self.missing,
            self.fallback,
            self.quality.as_str()
        )
    }

    pub fn to_text_report(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.summary_line());
        out.push('\n');
        out.push_str(&format!(
            "visual_review_usable={}\n",
            self.can_use_for_visual_review()
        ));
        out.push_str("\ncategory_stats:\n");
        for (category, stats) in &self.category_stats {
            out.push_str(&format!(
                "  {}: {}/{} loaded, {} missing, {} fallback\n",
                category, stats.loaded, stats.total, stats.missing, stats.fallback
            ));
        }

        out.push_str("\nmip_status:\n");
        for entry in self
            .entries
            .iter()
            .filter(|entry| entry.file_kind == MapAssetFileKind::Dds)
        {
            match &entry.dds {
                Some(dds) => out.push_str(&format!(
                    "  - {}: {} {}x{} {} source_mips={} upload_mips={}\n",
                    entry.relative_path,
                    entry.mip_status.as_str(),
                    dds.width,
                    dds.height,
                    dds.format,
                    dds.source_mips,
                    dds.upload_mips
                )),
                None => out.push_str(&format!(
                    "  - {}: {}\n",
                    entry.relative_path,
                    entry.mip_status.as_str()
                )),
            }
        }

        if !self.invalid_visual_review_paths.is_empty() {
            out.push_str("\ninvalid_for_visual_review:\n");
            for path in &self.invalid_visual_review_paths {
                out.push_str("  - ");
                out.push_str(path);
                out.push('\n');
            }
        }

        if !self.fallback_paths.is_empty() {
            out.push_str("\nfallback_paths:\n");
            for path in &self.fallback_paths {
                out.push_str("  - ");
                out.push_str(path);
                out.push('\n');
            }
        }

        out
    }

    pub fn to_json_report(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str(&format!("  \"total\": {},\n", self.total));
        out.push_str(&format!("  \"loaded\": {},\n", self.loaded));
        out.push_str(&format!("  \"missing\": {},\n", self.missing));
        out.push_str(&format!("  \"fallback\": {},\n", self.fallback));
        out.push_str(&format!("  \"quality\": \"{}\",\n", self.quality.as_str()));
        out.push_str(&format!(
            "  \"visual_review_usable\": {},\n",
            self.can_use_for_visual_review()
        ));
        out.push_str("  \"category_stats\": {\n");
        for (idx, (category, stats)) in self.category_stats.iter().enumerate() {
            out.push_str(&format!(
                "    \"{}\": {{ \"total\": {}, \"loaded\": {}, \"missing\": {}, \"fallback\": {} }}{}\n",
                json_escape(category),
                stats.total,
                stats.loaded,
                stats.missing,
                stats.fallback,
                comma(idx + 1, self.category_stats.len())
            ));
        }
        out.push_str("  },\n");
        out.push_str("  \"fallback_paths\": ");
        write_json_string_array(&mut out, &self.fallback_paths, 2);
        out.push_str(",\n");
        out.push_str("  \"invalid_visual_review_paths\": ");
        write_json_string_array(&mut out, &self.invalid_visual_review_paths, 2);
        out.push_str(",\n");
        out.push_str("  \"entries\": [\n");
        for (idx, entry) in self.entries.iter().enumerate() {
            out.push_str("    {\n");
            out.push_str(&format!("      \"role\": \"{:?}\",\n", entry.role));
            out.push_str(&format!(
                "      \"category\": \"{}\",\n",
                json_escape(entry.category)
            ));
            out.push_str(&format!(
                "      \"relative_path\": \"{}\",\n",
                json_escape(&entry.relative_path)
            ));
            out.push_str(&format!(
                "      \"requirement\": \"{}\",\n",
                entry.requirement.as_str()
            ));
            out.push_str(&format!(
                "      \"fallback_policy\": \"{}\",\n",
                entry.fallback_policy.as_str()
            ));
            out.push_str(&format!(
                "      \"file_kind\": \"{}\",\n",
                entry.file_kind.as_str()
            ));
            out.push_str(&format!("      \"loaded\": {},\n", entry.loaded));
            out.push_str(&format!("      \"usable\": {},\n", entry.usable));
            out.push_str(&format!("      \"is_srgb\": {},\n", entry.is_srgb));
            out.push_str(&format!(
                "      \"mip_status\": \"{}\",\n",
                entry.mip_status.as_str()
            ));
            out.push_str("      \"dds\": ");
            write_dds_json(&mut out, entry.dds.as_ref(), 6);
            out.push_str(",\n");
            out.push_str("      \"fallback_reason\": ");
            write_json_string_option(&mut out, entry.fallback_reason.as_deref());
            out.push_str(",\n");
            out.push_str(&format!(
                "      \"fallback_makes_visual_review_invalid\": {}\n",
                entry.fallback_makes_visual_review_invalid
            ));
            out.push_str("    }");
            out.push_str(comma(idx + 1, self.entries.len()));
            out.push('\n');
        }
        out.push_str("  ]\n");
        out.push_str("}\n");
        out
    }
}

fn build_audit_entry(entry: &MapResEntry) -> MapAssetAuditEntry {
    let role = entry.role;
    let loaded = entry.bytes.is_some();
    let requirement = requirement_for(role);
    let fallback_policy = fallback_policy_for(role);
    let category = audit_category_name(role);
    let file_kind = file_kind_for(&entry.relative_path);

    let mut usable = loaded;
    let mut mip_status = match file_kind {
        MapAssetFileKind::Bmp => MapAssetMipStatus::NotDds,
        MapAssetFileKind::Dds => {
            if loaded {
                MapAssetMipStatus::Complete
            } else {
                MapAssetMipStatus::Missing
            }
        }
    };
    let mut dds = None;
    let mut fallback_reason = if loaded {
        None
    } else {
        Some("missing".to_string())
    };

    if loaded && file_kind == MapAssetFileKind::Dds {
        let bytes = entry.bytes.as_ref().expect("loaded entry has bytes");
        match DdsImage::parse(bytes) {
            Ok(image) => match dds_upload_plan(&image) {
                Some(plan) if plan.upload_mip_count > 0 => {
                    mip_status = MapAssetMipStatus::Complete;
                    dds = Some(MapAssetDdsInfo {
                        width: image.width,
                        height: image.height,
                        format: format!("{:?}", image.format),
                        source_mips: plan.source_mip_count,
                        upload_mips: plan.upload_mip_count,
                        block_width: plan.block_width,
                        block_height: plan.block_height,
                        bytes_per_block: plan.bytes_per_block,
                    });
                }
                Some(plan) => {
                    usable = false;
                    mip_status = MapAssetMipStatus::Empty;
                    fallback_reason = Some("dds_empty_mip_chain".to_string());
                    dds = Some(MapAssetDdsInfo {
                        width: image.width,
                        height: image.height,
                        format: format!("{:?}", image.format),
                        source_mips: plan.source_mip_count,
                        upload_mips: plan.upload_mip_count,
                        block_width: plan.block_width,
                        block_height: plan.block_height,
                        bytes_per_block: plan.bytes_per_block,
                    });
                }
                None => {
                    usable = false;
                    mip_status = MapAssetMipStatus::UnsupportedFormat;
                    fallback_reason = Some(format!("unsupported_dds_format:{:?}", image.format));
                    dds = Some(MapAssetDdsInfo {
                        width: image.width,
                        height: image.height,
                        format: format!("{:?}", image.format),
                        source_mips: image.mip_count(),
                        upload_mips: 0,
                        block_width: 0,
                        block_height: 0,
                        bytes_per_block: 0,
                    });
                }
            },
            Err(err) => {
                usable = false;
                mip_status = MapAssetMipStatus::ParseFailed;
                fallback_reason = Some(format!("dds_parse_failed:{err}"));
            }
        }
    }

    if fallback_reason.is_some() {
        usable = false;
    }
    let fallback_makes_visual_review_invalid =
        fallback_reason.is_some() && fallback_invalidates_visual_review(role);

    MapAssetAuditEntry {
        role,
        category,
        relative_path: entry.relative_path.clone(),
        requirement,
        fallback_policy,
        file_kind,
        loaded,
        usable,
        is_srgb: role.is_srgb(),
        mip_status,
        dds,
        fallback_reason,
        fallback_makes_visual_review_invalid,
    }
}

pub fn requirement_for(role: MapResRole) -> MapAssetRequirement {
    use MapAssetRequirement::*;
    use MapResRole::*;
    match role {
        Provinces | Heightmap | TerrainIndex => StartupRequired,
        TerrainAtlas(_) | ColormapEmissive | ColormapWater(_) => BaseMapRequired,
        TerrainAtlasNormal(_)
        | WorldNormal
        | Lean1
        | Lean2
        | BorderCountry(_)
        | BorderProvince(_)
        | BorderState(_)
        | BorderSea(_)
        | BorderSeaRegion(_)
        | BorderImpassable(_)
        | RiverDiffuse(_)
        | RiverNormal(_)
        | RiverMasks
        | CityLights(_) => HighQuality,
        Rivers | TreesMask | Cities | MudDiffuseGloss(_) | MudNormalSpec(_) | SnowNormalDiffuse
        | IceDiffuse | IceNoise(_) | Reflection | ReflectionLandUnit | FowWaterSpec
        | FowNoise(_) | TreeSeason | TreeTint | Strait | NavalDominance | UnderwaterTerrain(_) => {
            Optional
        }
    }
}

pub fn fallback_policy_for(role: MapResRole) -> MapAssetFallbackPolicy {
    use MapAssetFallbackPolicy::*;
    match requirement_for(role) {
        MapAssetRequirement::StartupRequired if !role.allow_missing() => StartupBlocked,
        MapAssetRequirement::StartupRequired
        | MapAssetRequirement::BaseMapRequired
        | MapAssetRequirement::HighQuality
            if fallback_invalidates_visual_review(role) =>
        {
            VisualReviewBlocked
        }
        MapAssetRequirement::HighQuality => DegradedAllowed,
        MapAssetRequirement::Optional => OptionalAllowed,
        MapAssetRequirement::StartupRequired => StartupBlocked,
        MapAssetRequirement::BaseMapRequired => VisualReviewBlocked,
    }
}

pub fn fallback_invalidates_visual_review(role: MapResRole) -> bool {
    matches!(
        requirement_for(role),
        MapAssetRequirement::StartupRequired
            | MapAssetRequirement::BaseMapRequired
            | MapAssetRequirement::HighQuality
    )
}

fn quality_for(
    entries: &[MapAssetAuditEntry],
    total: usize,
    loaded: usize,
    fallback: usize,
) -> MapAssetQuality {
    if total > 0 && fallback == total {
        return MapAssetQuality::FallbackOnly;
    }
    if entries
        .iter()
        .any(|entry| entry.fallback_makes_visual_review_invalid)
    {
        return MapAssetQuality::InvalidForVisualReview;
    }
    if loaded < total || fallback > 0 {
        return MapAssetQuality::Degraded;
    }
    MapAssetQuality::Valid
}

pub fn audit_category_name(role: MapResRole) -> &'static str {
    use MapResRole::*;
    match role {
        Provinces | Heightmap | TerrainIndex | Rivers | TreesMask | Cities | WorldNormal => "bmp",
        TerrainAtlas(_) => "terrain_atlas",
        TerrainAtlasNormal(_) => "terrain_atlas_normal",
        MudDiffuseGloss(_) | MudNormalSpec(_) => "mud",
        SnowNormalDiffuse => "snow",
        IceDiffuse | IceNoise(_) => "ice",
        ColormapEmissive => "colormap",
        CityLights(_) => "citylights",
        ColormapWater(_) | Lean1 | Lean2 | Reflection | ReflectionLandUnit | FowWaterSpec
        | FowNoise(_) | UnderwaterTerrain(_) => "water",
        RiverDiffuse(_) | RiverNormal(_) | RiverMasks => "rivers",
        BorderCountry(_) | BorderProvince(_) | BorderState(_) | BorderSea(_)
        | BorderSeaRegion(_) | BorderImpassable(_) => "borders",
        TreeSeason | TreeTint => "trees",
        Strait | NavalDominance => "misc",
    }
}

fn file_kind_for(path: &str) -> MapAssetFileKind {
    if path.to_ascii_lowercase().ends_with(".dds") {
        MapAssetFileKind::Dds
    } else {
        MapAssetFileKind::Bmp
    }
}

fn comma(done: usize, total: usize) -> &'static str {
    if done < total {
        ","
    } else {
        ""
    }
}

fn write_json_string_array(out: &mut String, values: &[String], indent: usize) {
    if values.is_empty() {
        out.push_str("[]");
        return;
    }
    let pad = " ".repeat(indent);
    out.push_str("[\n");
    for (idx, value) in values.iter().enumerate() {
        out.push_str(&pad);
        out.push_str("  \"");
        out.push_str(&json_escape(value));
        out.push('"');
        out.push_str(comma(idx + 1, values.len()));
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

fn write_json_string_option(out: &mut String, value: Option<&str>) {
    match value {
        Some(value) => {
            out.push('"');
            out.push_str(&json_escape(value));
            out.push('"');
        }
        None => out.push_str("null"),
    }
}

fn write_dds_json(out: &mut String, dds: Option<&MapAssetDdsInfo>, indent: usize) {
    let Some(dds) = dds else {
        out.push_str("null");
        return;
    };
    let pad = " ".repeat(indent);
    out.push_str("{\n");
    out.push_str(&format!("{pad}  \"width\": {},\n", dds.width));
    out.push_str(&format!("{pad}  \"height\": {},\n", dds.height));
    out.push_str(&format!(
        "{pad}  \"format\": \"{}\",\n",
        json_escape(&dds.format)
    ));
    out.push_str(&format!("{pad}  \"source_mips\": {},\n", dds.source_mips));
    out.push_str(&format!("{pad}  \"upload_mips\": {},\n", dds.upload_mips));
    out.push_str(&format!("{pad}  \"block_width\": {},\n", dds.block_width));
    out.push_str(&format!("{pad}  \"block_height\": {},\n", dds.block_height));
    out.push_str(&format!(
        "{pad}  \"bytes_per_block\": {}\n",
        dds.bytes_per_block
    ));
    out.push_str(&pad);
    out.push('}');
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mip_size, AssetBytes, DdsFormat};
    use std::sync::Arc;

    fn loaded(role: MapResRole) -> MapResEntry {
        MapResEntry {
            role,
            relative_path: role.relative_path(),
            bytes: Some(Arc::from(synth_dds(4, 4, b"DXT5", 1)) as AssetBytes),
        }
    }

    fn loaded_bmp(role: MapResRole) -> MapResEntry {
        MapResEntry {
            role,
            relative_path: role.relative_path(),
            bytes: Some(Arc::from([1u8]) as AssetBytes),
        }
    }

    fn loaded_invalid_dds(role: MapResRole) -> MapResEntry {
        MapResEntry {
            role,
            relative_path: role.relative_path(),
            bytes: Some(Arc::from([1u8, 2, 3, 4]) as AssetBytes),
        }
    }

    fn missing(role: MapResRole) -> MapResEntry {
        MapResEntry {
            role,
            relative_path: role.relative_path(),
            bytes: None,
        }
    }

    fn synth_dds(width: u32, height: u32, fourcc_str: &[u8; 4], mip_count: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x20534444u32.to_le_bytes());
        let mut header = vec![0u8; 124];
        header[0..4].copy_from_slice(&124u32.to_le_bytes());
        header[4..8].copy_from_slice(&0x1007u32.to_le_bytes());
        header[8..12].copy_from_slice(&height.to_le_bytes());
        header[12..16].copy_from_slice(&width.to_le_bytes());
        header[24..28].copy_from_slice(&mip_count.to_le_bytes());
        let pf_offset = 72;
        header[pf_offset..pf_offset + 4].copy_from_slice(&32u32.to_le_bytes());
        header[pf_offset + 4..pf_offset + 8].copy_from_slice(&0x4u32.to_le_bytes());
        header[pf_offset + 8..pf_offset + 12].copy_from_slice(fourcc_str);
        buf.extend_from_slice(&header);

        let format = match fourcc_str {
            b"DXT1" => DdsFormat::Bc1,
            b"DXT5" => DdsFormat::Bc3,
            b"ATI2" => DdsFormat::Bc5,
            _ => DdsFormat::Bc3,
        };
        let mut total = 0usize;
        let mut w = width;
        let mut h = height;
        for _ in 0..mip_count {
            total += mip_size(w, h, format);
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        buf.resize(128 + total, 0xAB);
        buf
    }

    #[test]
    fn requirement_tiers_match_phase2_contract() {
        assert_eq!(
            requirement_for(MapResRole::Provinces),
            MapAssetRequirement::StartupRequired
        );
        assert_eq!(
            requirement_for(MapResRole::TerrainAtlas(0)),
            MapAssetRequirement::BaseMapRequired
        );
        assert_eq!(
            requirement_for(MapResRole::Lean1),
            MapAssetRequirement::HighQuality
        );
        assert_eq!(
            requirement_for(MapResRole::TreeSeason),
            MapAssetRequirement::Optional
        );
    }

    #[test]
    fn critical_base_fallback_invalidates_review() {
        let set = VanillaMapSet {
            entries: vec![
                loaded_bmp(MapResRole::Provinces),
                loaded_bmp(MapResRole::Heightmap),
                loaded_bmp(MapResRole::TerrainIndex),
                missing(MapResRole::TerrainAtlas(0)),
            ],
        };
        let audit = MapAssetAudit::from_map_set(&set);
        assert_eq!(audit.quality, MapAssetQuality::InvalidForVisualReview);
        assert!(!audit.can_use_for_visual_review());
        assert_eq!(
            audit.invalid_visual_review_paths,
            vec!["map/terrain/atlas0.dds".to_string()]
        );
    }

    #[test]
    fn high_quality_fallback_invalidates_review() {
        let set = VanillaMapSet {
            entries: vec![
                loaded_bmp(MapResRole::Provinces),
                loaded_bmp(MapResRole::Heightmap),
                loaded_bmp(MapResRole::TerrainIndex),
                missing(MapResRole::Lean1),
            ],
        };
        let audit = MapAssetAudit::from_map_set(&set);
        assert_eq!(audit.quality, MapAssetQuality::InvalidForVisualReview);
        assert!(!audit.can_use_for_visual_review());
        assert_eq!(
            audit.entries.last().unwrap().fallback_policy,
            MapAssetFallbackPolicy::VisualReviewBlocked
        );
    }

    #[test]
    fn optional_fallback_degrades_but_allows_review() {
        let set = VanillaMapSet {
            entries: vec![
                loaded_bmp(MapResRole::Provinces),
                loaded_bmp(MapResRole::Heightmap),
                loaded_bmp(MapResRole::TerrainIndex),
                loaded(MapResRole::TerrainAtlas(0)),
                missing(MapResRole::TreeSeason),
            ],
        };
        let audit = MapAssetAudit::from_map_set(&set);
        assert_eq!(audit.quality, MapAssetQuality::Degraded);
        assert!(audit.can_use_for_visual_review());
        assert!(audit.invalid_visual_review_paths.is_empty());
    }

    #[test]
    fn all_fallbacks_are_fallback_only_and_not_usable() {
        let set = VanillaMapSet {
            entries: vec![
                missing(MapResRole::Provinces),
                missing(MapResRole::Heightmap),
                missing(MapResRole::TerrainIndex),
            ],
        };
        let audit = MapAssetAudit::from_map_set(&set);
        assert_eq!(audit.quality, MapAssetQuality::FallbackOnly);
        assert!(!audit.can_use_for_visual_review());
    }

    #[test]
    fn dds_parse_failure_is_reported_as_fallback() {
        let set = VanillaMapSet {
            entries: vec![
                loaded_bmp(MapResRole::Provinces),
                loaded_bmp(MapResRole::Heightmap),
                loaded_bmp(MapResRole::TerrainIndex),
                loaded_invalid_dds(MapResRole::ColormapWater(0)),
            ],
        };
        let audit = MapAssetAudit::from_map_set(&set);
        let entry = audit
            .entries
            .iter()
            .find(|entry| entry.role == MapResRole::ColormapWater(0))
            .unwrap();
        assert_eq!(audit.quality, MapAssetQuality::InvalidForVisualReview);
        assert!(entry.loaded);
        assert!(!entry.usable);
        assert_eq!(entry.mip_status, MapAssetMipStatus::ParseFailed);
        assert!(entry
            .fallback_reason
            .as_deref()
            .unwrap()
            .starts_with("dds_parse_failed:"));
    }

    #[test]
    fn json_report_contains_phase2_gate_fields() {
        let set = VanillaMapSet {
            entries: vec![
                loaded_bmp(MapResRole::Provinces),
                loaded_bmp(MapResRole::Heightmap),
                loaded_bmp(MapResRole::TerrainIndex),
                loaded(MapResRole::ColormapWater(0)),
            ],
        };
        let json = MapAssetAudit::from_map_set(&set).to_json_report();
        assert!(json.contains("\"quality\": \"Valid\""));
        assert!(json.contains("\"visual_review_usable\": true"));
        assert!(json.contains("\"fallback_policy\""));
        assert!(json.contains("\"mip_status\": \"Complete\""));
        assert!(json.contains("\"upload_mips\": 1"));
    }
}
