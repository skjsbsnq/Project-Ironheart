use std::fmt::Write as _;
use std::sync::Arc;

use hoi4_assets::{
    dds_upload_plan, DdsImage, MapAssetAudit, MapResRole, MapSetError, PostEffectVolumeIndex,
    TgaImage, VanillaMapSet,
};

use crate::vanilla_targets;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingSourceKind {
    VanillaResource,
    DynamicTarget,
    ProjectFallback,
    Mock,
}

impl BindingSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VanillaResource => "vanilla_resource",
            Self::DynamicTarget => "dynamic_target",
            Self::ProjectFallback => "project_fallback",
            Self::Mock => "mock",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingProvenance {
    VanillaBacked,
    RuntimeGenerated,
    NeutralFallback,
    ProjectGenerated,
    Missing,
}

impl BindingProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VanillaBacked => "vanilla_backed",
            Self::RuntimeGenerated => "runtime_generated",
            Self::NeutralFallback => "neutral_fallback",
            Self::ProjectGenerated => "project_generated",
            Self::Missing => "missing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BindingBlockingLevel {
    None,
    Degraded,
    Critical,
}

impl BindingBlockingLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Degraded => "degraded",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BindingAuditEntry {
    pub pass: &'static str,
    pub binding: &'static str,
    pub role: Option<MapResRole>,
    pub source_kind: BindingSourceKind,
    pub provenance: BindingProvenance,
    pub source_name: String,
    pub source_detail: Option<String>,
    pub resource_format: Option<String>,
    pub resource_dimensions: Option<String>,
    pub source_trace: Option<String>,
    pub parity_status: Option<String>,
    pub producer_status: Option<String>,
    pub asset_loaded: bool,
    pub binding_present: bool,
    pub producer_equivalent: bool,
    pub loaded: bool,
    pub critical: bool,
    pub mock_name: Option<String>,
    pub reason: Option<String>,
    pub visual_impact: &'static str,
    pub blocking_level: BindingBlockingLevel,
}

impl BindingAuditEntry {
    pub fn vanilla(
        pass: &'static str,
        binding: &'static str,
        role: MapResRole,
        loaded: bool,
        critical: bool,
        reason: Option<String>,
        visual_impact: &'static str,
    ) -> Self {
        Self {
            pass,
            binding,
            role: Some(role),
            source_kind: if loaded {
                BindingSourceKind::VanillaResource
            } else {
                BindingSourceKind::ProjectFallback
            },
            provenance: if loaded {
                BindingProvenance::VanillaBacked
            } else {
                BindingProvenance::Missing
            },
            source_name: role.relative_path(),
            source_detail: None,
            resource_format: None,
            resource_dimensions: None,
            source_trace: None,
            parity_status: None,
            producer_status: None,
            asset_loaded: loaded,
            binding_present: true,
            producer_equivalent: loaded,
            loaded,
            critical,
            mock_name: None,
            reason,
            visual_impact,
            blocking_level: if loaded {
                BindingBlockingLevel::None
            } else if critical {
                BindingBlockingLevel::Critical
            } else {
                BindingBlockingLevel::Degraded
            },
        }
    }

    pub fn dynamic_target(
        pass: &'static str,
        binding: &'static str,
        source_name: impl Into<String>,
        visual_impact: &'static str,
    ) -> Self {
        Self {
            pass,
            binding,
            role: None,
            source_kind: BindingSourceKind::DynamicTarget,
            provenance: BindingProvenance::RuntimeGenerated,
            source_name: source_name.into(),
            source_detail: None,
            resource_format: None,
            resource_dimensions: None,
            source_trace: None,
            parity_status: None,
            producer_status: Some("runtime_generated".to_string()),
            asset_loaded: false,
            binding_present: true,
            producer_equivalent: true,
            loaded: true,
            critical: false,
            mock_name: None,
            reason: None,
            visual_impact,
            blocking_level: BindingBlockingLevel::None,
        }
    }

    pub fn plain_resource(
        pass: &'static str,
        binding: &'static str,
        source_name: impl Into<String>,
        loaded: bool,
        critical: bool,
        reason: Option<String>,
        visual_impact: &'static str,
    ) -> Self {
        Self {
            pass,
            binding,
            role: None,
            source_kind: if loaded {
                BindingSourceKind::VanillaResource
            } else {
                BindingSourceKind::ProjectFallback
            },
            provenance: if loaded {
                BindingProvenance::VanillaBacked
            } else {
                BindingProvenance::Missing
            },
            source_name: source_name.into(),
            source_detail: None,
            resource_format: None,
            resource_dimensions: None,
            source_trace: None,
            parity_status: None,
            producer_status: None,
            asset_loaded: loaded,
            binding_present: true,
            producer_equivalent: loaded,
            loaded,
            critical,
            mock_name: None,
            reason,
            visual_impact,
            blocking_level: if loaded {
                BindingBlockingLevel::None
            } else if critical {
                BindingBlockingLevel::Critical
            } else {
                BindingBlockingLevel::Degraded
            },
        }
    }

    pub fn dynamic_target_blocker(
        pass: &'static str,
        binding: &'static str,
        source_name: impl Into<String>,
        reason: impl Into<String>,
        visual_impact: &'static str,
    ) -> Self {
        Self {
            pass,
            binding,
            role: None,
            source_kind: BindingSourceKind::DynamicTarget,
            provenance: BindingProvenance::RuntimeGenerated,
            source_name: source_name.into(),
            source_detail: None,
            resource_format: None,
            resource_dimensions: None,
            source_trace: None,
            parity_status: None,
            producer_status: Some("missing_or_non_equivalent".to_string()),
            asset_loaded: false,
            binding_present: true,
            producer_equivalent: false,
            loaded: true,
            critical: true,
            mock_name: None,
            reason: Some(reason.into()),
            visual_impact,
            blocking_level: BindingBlockingLevel::Critical,
        }
    }

    pub fn is_critical_mock_in_parity(&self) -> bool {
        self.source_kind == BindingSourceKind::Mock
            && matches!(self.blocking_level, BindingBlockingLevel::Critical)
    }

    pub fn with_runtime_target_metadata(
        mut self,
        source_detail: impl Into<String>,
        resource_format: impl Into<String>,
        resource_dimensions: impl Into<String>,
        source_trace: impl Into<String>,
        parity_status: impl Into<String>,
    ) -> Self {
        self.source_detail = Some(source_detail.into());
        self.resource_format = Some(resource_format.into());
        self.resource_dimensions = Some(resource_dimensions.into());
        self.source_trace = Some(source_trace.into());
        self.parity_status = Some(parity_status.into());
        self
    }

    pub fn with_producer_status(
        mut self,
        provenance: BindingProvenance,
        producer_status: impl Into<String>,
        producer_equivalent: bool,
        blocking_level: BindingBlockingLevel,
    ) -> Self {
        self.provenance = provenance;
        self.producer_status = Some(producer_status.into());
        self.producer_equivalent = producer_equivalent;
        self.critical = blocking_level == BindingBlockingLevel::Critical;
        self.blocking_level = blocking_level;
        self
    }

    pub fn mock(
        pass: &'static str,
        binding: &'static str,
        mock_name: impl Into<String>,
        reason: impl Into<String>,
        visual_impact: &'static str,
        blocking_level: BindingBlockingLevel,
    ) -> Self {
        let mock_name = mock_name.into();
        Self {
            pass,
            binding,
            role: None,
            source_kind: BindingSourceKind::Mock,
            provenance: BindingProvenance::ProjectGenerated,
            source_name: mock_name.clone(),
            source_detail: None,
            resource_format: None,
            resource_dimensions: None,
            source_trace: None,
            parity_status: None,
            producer_status: Some("mock".to_string()),
            asset_loaded: false,
            binding_present: true,
            producer_equivalent: false,
            loaded: true,
            critical: blocking_level == BindingBlockingLevel::Critical,
            mock_name: Some(mock_name),
            reason: Some(reason.into()),
            visual_impact,
            blocking_level,
        }
    }

    pub fn is_blocking_in_parity(&self) -> bool {
        matches!(self.blocking_level, BindingBlockingLevel::Critical)
    }
}

#[derive(Debug, Clone, Default)]
pub struct BindingAudit {
    pub entries: Vec<BindingAuditEntry>,
}

impl BindingAudit {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn extend(&mut self, entries: impl IntoIterator<Item = BindingAuditEntry>) {
        self.entries.extend(entries);
    }

    pub fn total(&self) -> usize {
        self.entries.len()
    }

    pub fn mock_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.source_kind == BindingSourceKind::Mock)
            .count()
    }

    pub fn fallback_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.source_kind == BindingSourceKind::ProjectFallback)
            .count()
    }

    pub fn critical_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.is_blocking_in_parity())
            .count()
    }

    pub fn critical_entries(&self) -> impl Iterator<Item = &BindingAuditEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.is_blocking_in_parity())
    }

    pub fn critical_mock_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.is_critical_mock_in_parity())
            .count()
    }

    pub fn producer_non_equivalent_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.binding_present && !entry.producer_equivalent)
            .count()
    }

    pub fn summary_line(&self) -> String {
        format!(
            "[binding-audit] bindings={} fallback={} mock={} critical={} producer_non_equivalent={}",
            self.total(),
            self.fallback_count(),
            self.mock_count(),
            self.critical_count(),
            self.producer_non_equivalent_count()
        )
    }

    pub fn to_text_report(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "{}", self.summary_line());
        for entry in &self.entries {
            let _ = writeln!(
                out,
                "  - {}.{} source={} name={} loaded={} critical={} blocking={} impact={}",
                entry.pass,
                entry.binding,
                entry.source_kind.as_str(),
                entry.source_name,
                entry.loaded,
                entry.critical,
                entry.blocking_level.as_str(),
                entry.visual_impact
            );
            let _ = writeln!(
                out,
                "    provenance={} asset_loaded={} binding_present={} producer_equivalent={}",
                entry.provenance.as_str(),
                entry.asset_loaded,
                entry.binding_present,
                entry.producer_equivalent
            );
            if let Some(producer_status) = &entry.producer_status {
                let _ = writeln!(out, "    producer_status={}", producer_status);
            }
            if let Some(reason) = &entry.reason {
                let _ = writeln!(out, "    reason={}", reason);
            }
            if let Some(format) = &entry.resource_format {
                let _ = writeln!(out, "    format={}", format);
            }
            if let Some(dimensions) = &entry.resource_dimensions {
                let _ = writeln!(out, "    dimensions={}", dimensions);
            }
            if let Some(source_detail) = &entry.source_detail {
                let _ = writeln!(out, "    source_detail={}", source_detail);
            }
            if let Some(source_trace) = &entry.source_trace {
                let _ = writeln!(out, "    source_trace={}", source_trace);
            }
            if let Some(parity_status) = &entry.parity_status {
                let _ = writeln!(out, "    parity_status={}", parity_status);
            }
        }
        out
    }

    pub fn to_json_report(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        let _ = writeln!(out, "  \"total\": {},", self.total());
        let _ = writeln!(out, "  \"fallback\": {},", self.fallback_count());
        let _ = writeln!(out, "  \"mock\": {},", self.mock_count());
        let _ = writeln!(out, "  \"critical\": {},", self.critical_count());
        let _ = writeln!(
            out,
            "  \"producer_non_equivalent\": {},",
            self.producer_non_equivalent_count()
        );
        out.push_str("  \"entries\": [\n");
        for (idx, entry) in self.entries.iter().enumerate() {
            out.push_str("    {\n");
            let _ = writeln!(out, "      \"pass\": \"{}\",", json_escape(entry.pass));
            let _ = writeln!(
                out,
                "      \"binding\": \"{}\",",
                json_escape(entry.binding)
            );
            match entry.role {
                Some(role) => {
                    let _ = writeln!(out, "      \"role\": \"{:?}\",", role);
                    let _ = writeln!(
                        out,
                        "      \"relative_path\": \"{}\",",
                        json_escape(&role.relative_path())
                    );
                }
                None => {
                    out.push_str("      \"role\": null,\n");
                    out.push_str("      \"relative_path\": null,\n");
                }
            }
            let _ = writeln!(
                out,
                "      \"source_kind\": \"{}\",",
                entry.source_kind.as_str()
            );
            let _ = writeln!(
                out,
                "      \"provenance\": \"{}\",",
                entry.provenance.as_str()
            );
            let _ = writeln!(
                out,
                "      \"source_name\": \"{}\",",
                json_escape(&entry.source_name)
            );
            write_json_string_option(&mut out, "source_detail", entry.source_detail.as_deref(), 6);
            write_json_string_option(
                &mut out,
                "resource_format",
                entry.resource_format.as_deref(),
                6,
            );
            write_json_string_option(
                &mut out,
                "resource_dimensions",
                entry.resource_dimensions.as_deref(),
                6,
            );
            write_json_string_option(&mut out, "source_trace", entry.source_trace.as_deref(), 6);
            write_json_string_option(&mut out, "parity_status", entry.parity_status.as_deref(), 6);
            write_json_string_option(
                &mut out,
                "producer_status",
                entry.producer_status.as_deref(),
                6,
            );
            let _ = writeln!(out, "      \"asset_loaded\": {},", entry.asset_loaded);
            let _ = writeln!(out, "      \"binding_present\": {},", entry.binding_present);
            let _ = writeln!(
                out,
                "      \"producer_equivalent\": {},",
                entry.producer_equivalent
            );
            let _ = writeln!(out, "      \"loaded\": {},", entry.loaded);
            let _ = writeln!(out, "      \"critical\": {},", entry.critical);
            match &entry.mock_name {
                Some(mock_name) => {
                    let _ = writeln!(out, "      \"mock_name\": \"{}\",", json_escape(mock_name));
                }
                None => out.push_str("      \"mock_name\": null,\n"),
            }
            match &entry.reason {
                Some(reason) => {
                    let _ = writeln!(out, "      \"reason\": \"{}\",", json_escape(reason));
                }
                None => out.push_str("      \"reason\": null,\n"),
            }
            let _ = writeln!(
                out,
                "      \"visual_impact\": \"{}\",",
                json_escape(entry.visual_impact)
            );
            let _ = writeln!(
                out,
                "      \"blocking_level\": \"{}\"",
                entry.blocking_level.as_str()
            );
            out.push_str("    }");
            if idx + 1 < self.entries.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  ]\n");
        out.push_str("}\n");
        out
    }
}

#[derive(Debug, Clone)]
pub struct VanillaResourceViews {
    pub map_set: Arc<VanillaMapSet>,
    pub posteffect_volumes: Option<Arc<PostEffectVolumeIndex>>,
    pub posteffect_lut_count: usize,
    pub posteffect_lut_loaded: usize,
    pub posteffect_lut_warnings: Vec<String>,
}

impl VanillaResourceViews {
    pub fn load(path_cfg: &hoi4_paths::PathConfig) -> Result<Self, MapSetError> {
        let db = hoi4_assets::FsAssetDb::new(path_cfg.clone());
        let map_set = VanillaMapSet::load(&db)?;
        let (posteffect_volumes, lut_loaded, lut_count, warnings) =
            load_posteffect_volume_audit(&db);
        Ok(Self {
            map_set: Arc::new(map_set),
            posteffect_volumes,
            posteffect_lut_count: lut_count,
            posteffect_lut_loaded: lut_loaded,
            posteffect_lut_warnings: warnings,
        })
    }

    pub fn load_for_audit(path_cfg: &hoi4_paths::PathConfig) -> Self {
        let db = hoi4_assets::FsAssetDb::new(path_cfg.clone());
        let (posteffect_volumes, lut_loaded, lut_count, warnings) =
            load_posteffect_volume_audit(&db);
        Self {
            map_set: Arc::new(VanillaMapSet::load_for_audit(&db)),
            posteffect_volumes,
            posteffect_lut_count: lut_count,
            posteffect_lut_loaded: lut_loaded,
            posteffect_lut_warnings: warnings,
        }
    }

    pub fn bytes(&self, role: MapResRole) -> Option<&[u8]> {
        self.map_set.bytes(role).map(|bytes| bytes.as_ref())
    }

    pub fn terrain_atlas_roles() -> [MapResRole; 3] {
        [
            MapResRole::TerrainAtlas(0),
            MapResRole::TerrainAtlas(1),
            MapResRole::TerrainAtlas(2),
        ]
    }

    pub fn terrain_atlas_normal_roles() -> [MapResRole; 3] {
        [
            MapResRole::TerrainAtlasNormal(0),
            MapResRole::TerrainAtlasNormal(1),
            MapResRole::TerrainAtlasNormal(2),
        ]
    }

    pub fn mud_diffuse_roles() -> [MapResRole; 3] {
        [
            MapResRole::MudDiffuseGloss(0),
            MapResRole::MudDiffuseGloss(1),
            MapResRole::MudDiffuseGloss(2),
        ]
    }

    pub fn mud_normal_roles() -> [MapResRole; 3] {
        [
            MapResRole::MudNormalSpec(0),
            MapResRole::MudNormalSpec(1),
            MapResRole::MudNormalSpec(2),
        ]
    }

    pub fn citylights_roles() -> [MapResRole; 3] {
        [
            MapResRole::CityLights(0),
            MapResRole::CityLights(1),
            MapResRole::CityLights(2),
        ]
    }

    pub fn colormap_roles() -> [MapResRole; 4] {
        [
            MapResRole::ColormapEmissive,
            MapResRole::ColormapWater(0),
            MapResRole::ColormapWater(1),
            MapResRole::ColormapWater(2),
        ]
    }

    pub fn water_resource_roles() -> [MapResRole; 13] {
        [
            MapResRole::Lean1,
            MapResRole::Lean2,
            MapResRole::Reflection,
            MapResRole::FowWaterSpec,
            MapResRole::ColormapWater(0),
            MapResRole::ColormapWater(1),
            MapResRole::ColormapWater(2),
            MapResRole::IceDiffuse,
            MapResRole::IceNoise(0),
            MapResRole::IceNoise(1),
            MapResRole::IceNoise(2),
            MapResRole::ReflectionLandUnit,
            MapResRole::UnderwaterTerrain(0),
        ]
    }

    pub fn river_resource_roles() -> [MapResRole; 7] {
        [
            MapResRole::RiverDiffuse(0),
            MapResRole::RiverDiffuse(1),
            MapResRole::RiverDiffuse(2),
            MapResRole::RiverNormal(0),
            MapResRole::RiverNormal(1),
            MapResRole::RiverNormal(2),
            MapResRole::RiverMasks,
        ]
    }

    pub fn tree_resource_roles() -> [MapResRole; 4] {
        [
            MapResRole::TreesMask,
            MapResRole::TreeSeason,
            MapResRole::TreeTint,
            MapResRole::ColormapEmissive,
        ]
    }

    pub fn border_texture_roles() -> [MapResRole; 18] {
        [
            MapResRole::BorderCountry(0),
            MapResRole::BorderCountry(1),
            MapResRole::BorderCountry(2),
            MapResRole::BorderProvince(0),
            MapResRole::BorderProvince(1),
            MapResRole::BorderProvince(2),
            MapResRole::BorderState(0),
            MapResRole::BorderState(1),
            MapResRole::BorderState(2),
            MapResRole::BorderSea(0),
            MapResRole::BorderSea(1),
            MapResRole::BorderSea(2),
            MapResRole::BorderSeaRegion(0),
            MapResRole::BorderSeaRegion(1),
            MapResRole::BorderSeaRegion(2),
            MapResRole::BorderImpassable(0),
            MapResRole::BorderImpassable(1),
            MapResRole::BorderImpassable(2),
        ]
    }

    pub fn has_vanilla_color_cube(&self) -> bool {
        self.posteffect_volumes
            .as_ref()
            .is_some_and(|index| index.default_lut().is_some())
            && self.posteffect_lut_count > 0
            && self.posteffect_lut_loaded == self.posteffect_lut_count
    }

    pub fn default_color_cube_source(&self) -> String {
        self.posteffect_volumes
            .as_ref()
            .and_then(|index| index.default_lut())
            .unwrap_or("identity_color_cube")
            .to_string()
    }

    pub fn phase1_binding_audit(&self) -> BindingAudit {
        let asset_audit = MapAssetAudit::from_map_set(&self.map_set);
        let mut audit = BindingAudit::new();
        audit.extend([
            binding_from_asset_audit(
                &asset_audit,
                "terrain",
                "terrain_atlas",
                MapResRole::TerrainAtlas(0),
                true,
                "terrain diffuse atlas falls back to a white texture",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "terrain",
                "terrain_atlas_normal",
                MapResRole::TerrainAtlasNormal(0),
                true,
                "terrain lighting and material normals flatten",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "terrain",
                "world_normal",
                MapResRole::WorldNormal,
                true,
                "large-scale terrain lighting normal falls back to flat",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "terrain",
                "colormap",
                MapResRole::ColormapEmissive,
                true,
                "terrain natural color base falls back to neutral gray",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "terrain",
                "citylights",
                MapResRole::CityLights(0),
                true,
                "night city light contribution disappears",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "terrain",
                "snow_normal_diffuse",
                MapResRole::SnowNormalDiffuse,
                true,
                "snow overlay loses vanilla normal/diffuse texture",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "terrain",
                "mud_diffuse_gloss",
                MapResRole::MudDiffuseGloss(0),
                true,
                "mud overlay falls back to a flat brown tint",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "terrain",
                "mud_normal_spec",
                MapResRole::MudNormalSpec(0),
                true,
                "mud overlay normals/specular flatten",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "terrain",
                "rivers_bmp",
                MapResRole::Rivers,
                false,
                "river mask is unavailable",
            ),
        ]);
        audit.extend(vanilla_targets::runtime_target_audit_entries_for_pass(
            "terrain",
        ));
        audit.extend([
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "water_normal_lean1",
                MapResRole::Lean1,
                true,
                "water normal motion and specular detail flatten",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "water_normal_lean2",
                MapResRole::Lean2,
                true,
                "water normal motion and specular detail flatten",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "reflection_tex",
                MapResRole::Reflection,
                false,
                "water reflection uses flat fallback",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "fow_water_spec",
                MapResRole::FowWaterSpec,
                false,
                "water specular/FOW mask is approximated",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "colormap_water",
                MapResRole::ColormapWater(0),
                true,
                "water base color falls back to flat color",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "colormap_water_1",
                MapResRole::ColormapWater(1),
                false,
                "water medium-distance color LOD falls back to colormap_water_0",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "colormap_water_2",
                MapResRole::ColormapWater(2),
                false,
                "water far-distance color LOD falls back to colormap_water_0",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "ice_diffuse",
                MapResRole::IceDiffuse,
                false,
                "ice coverage is approximated",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "ice_noise",
                MapResRole::IceNoise(0),
                false,
                "ice noise is approximated",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "ice_noise_1",
                MapResRole::IceNoise(1),
                false,
                "secondary ice noise is approximated",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "reflection_land_unit",
                MapResRole::ReflectionLandUnit,
                false,
                "land/unit reflection contribution uses a flat fallback",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "water",
                "underwater_terrain",
                MapResRole::UnderwaterTerrain(0),
                false,
                "water refraction falls back to a flat underwater color",
            ),
        ]);
        audit.extend(vanilla_targets::runtime_target_audit_entries_for_pass(
            "water",
        ));
        audit.extend([
            binding_from_asset_audit(
                &asset_audit,
                "tree",
                "tree_mask",
                MapResRole::TreesMask,
                true,
                "distant tree clipping and forest density mask disappear",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "tree",
                "tree_season",
                MapResRole::TreeSeason,
                true,
                "seasonal foliage colors fall back to summer-neutral",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "tree",
                "tree_tint",
                MapResRole::TreeTint,
                true,
                "per-region foliage tint falls back to neutral gray",
            ),
            binding_from_asset_audit(
                &asset_audit,
                "tree",
                "tree_colormap",
                MapResRole::ColormapEmissive,
                true,
                "tree color no longer follows terrain ColorMap/ColorMapSecond",
            ),
        ]);
        audit.extend(vanilla_targets::runtime_target_audit_entries_for_pass(
            "tree",
        ));
        audit.extend(vanilla_targets::runtime_target_audit_entries_for_pass(
            "pdxmesh",
        ));
        for (binding, role, critical, impact) in [
            (
                "river_diffuse_0",
                MapResRole::RiverDiffuse(0),
                true,
                "river diffuse color falls back to flat blue",
            ),
            (
                "river_diffuse_1",
                MapResRole::RiverDiffuse(1),
                false,
                "alternate river diffuse LOD falls back to flat blue",
            ),
            (
                "river_diffuse_2",
                MapResRole::RiverDiffuse(2),
                false,
                "alternate river diffuse LOD falls back to flat blue",
            ),
            (
                "river_normal_0",
                MapResRole::RiverNormal(0),
                true,
                "river normals and highlights flatten",
            ),
            (
                "river_normal_1",
                MapResRole::RiverNormal(1),
                false,
                "alternate river normal LOD flattens",
            ),
            (
                "river_normal_2",
                MapResRole::RiverNormal(2),
                false,
                "alternate river normal LOD flattens",
            ),
            (
                "river_masks",
                MapResRole::RiverMasks,
                true,
                "river width and alpha masks are approximated",
            ),
        ] {
            audit.extend([binding_from_asset_audit(
                &asset_audit,
                "river",
                binding,
                role,
                critical,
                impact,
            )]);
        }
        audit.extend(vanilla_targets::runtime_target_audit_entries_for_pass(
            "river",
        ));
        audit.extend(vanilla_targets::runtime_target_audit_entries_for_pass(
            "projected_fow_shadow",
        ));
        let color_cube_loaded = self.has_vanilla_color_cube();
        let color_cube_source = self.default_color_cube_source();
        let color_cube_reason = if color_cube_loaded {
            None
        } else if self.posteffect_volumes.is_none() {
            Some("missing:gfx/posteffect_volumes.txt".to_string())
        } else if self.posteffect_lut_count == 0 {
            Some("posteffect_volumes_has_no_lut_entries".to_string())
        } else {
            Some(format!(
                "loaded_luts={}/{}",
                self.posteffect_lut_loaded, self.posteffect_lut_count
            ))
        };
        audit.extend([
            BindingAuditEntry::dynamic_target(
                "postprocess",
                "hdr_scene",
                "HdrTarget",
                "RestoreScene samples the full HDR scene before LDR conversion",
            ),
            BindingAuditEntry::dynamic_target(
                "postprocess",
                "bloom",
                "BloomChain",
                "RestoreScene composites the generated vanilla bloom chain",
            ),
            BindingAuditEntry::dynamic_target(
                "postprocess",
                "average_luminance",
                "AverageLuminance",
                "RestoreScene exposure uses generated average log luminance",
            ),
            BindingAuditEntry::plain_resource(
                "postprocess",
                "color_cube",
                color_cube_source,
                color_cube_loaded,
                false,
                color_cube_reason,
                "RestoreScene samples vanilla posteffect ColorCube LUTs",
            ),
        ]);
        audit
    }
}

fn load_posteffect_volume_audit(
    db: &impl hoi4_assets::AssetDb,
) -> (
    Option<Arc<PostEffectVolumeIndex>>,
    usize,
    usize,
    Vec<String>,
) {
    let mut warnings = Vec::new();
    let index = match PostEffectVolumeIndex::load(db) {
        Ok(index) => index,
        Err(err) => {
            warnings.push(err.to_string());
            return (None, 0, 0, warnings);
        }
    };
    let paths = index.lut_paths();
    let mut loaded = 0;
    for path in &paths {
        match hoi4_assets::load_tga(db, path) {
            Ok(image) if is_color_cube_tga(&image) => loaded += 1,
            Ok(image) => warnings.push(format!(
                "{}: unsupported ColorCube TGA dimensions {}x{}; expected 1024x32",
                path, image.width, image.height
            )),
            Err(err) => warnings.push(
                err.with_referrer_path("gfx/posteffect_volumes.txt")
                    .to_string(),
            ),
        }
    }
    (Some(Arc::new(index)), loaded, paths.len(), warnings)
}

fn is_color_cube_tga(image: &TgaImage) -> bool {
    image.width == 1024
        && image.height == 32
        && image.pixels.len() == (image.width * image.height * 4) as usize
}

fn binding_from_asset_audit(
    audit: &MapAssetAudit,
    pass: &'static str,
    binding: &'static str,
    role: MapResRole,
    critical: bool,
    visual_impact: &'static str,
) -> BindingAuditEntry {
    let asset = audit.entries.iter().find(|entry| entry.role == role);
    let loaded = asset.map(|entry| entry.usable).unwrap_or(false);
    let reason = asset
        .and_then(|entry| entry.fallback_reason.clone())
        .or_else(|| (!loaded).then(|| "missing_resource".to_string()));
    BindingAuditEntry::vanilla(pass, binding, role, loaded, critical, reason, visual_impact)
}

#[derive(Debug, Clone, Copy)]
pub struct DdsUploadRequest {
    pub role: MapResRole,
    pub label: &'static str,
    pub fallback_rgba: [u8; 4],
    pub srgb: bool,
    pub critical: bool,
    pub pass: &'static str,
    pub binding: &'static str,
    pub visual_impact: &'static str,
}

#[derive(Debug)]
pub struct UploadedTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub audit: BindingAuditEntry,
}

pub fn upload_dds_or_fallback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &VanillaResourceViews,
    request: DdsUploadRequest,
    warnings: &mut Vec<String>,
) -> UploadedTexture {
    let path = request.role.relative_path();
    let bytes = resources.bytes(request.role);
    let mut fallback_reason = None;
    let parsed = match bytes {
        Some(bytes) if !bytes.is_empty() => match DdsImage::parse(bytes) {
            Ok(dds) => Some(dds),
            Err(err) => {
                fallback_reason = Some(format!("dds_parse_failed:{err}"));
                None
            }
        },
        Some(_) => {
            fallback_reason = Some("empty_resource".to_string());
            None
        }
        None => {
            fallback_reason = Some("missing_resource".to_string());
            None
        }
    };

    if let Some(dds) = parsed {
        let format = match (dds.format, request.srgb) {
            (hoi4_assets::DdsFormat::Bc1, true) => Some(wgpu::TextureFormat::Bc1RgbaUnormSrgb),
            (hoi4_assets::DdsFormat::Bc1, false) => Some(wgpu::TextureFormat::Bc1RgbaUnorm),
            (hoi4_assets::DdsFormat::Bc3, true) => Some(wgpu::TextureFormat::Bc3RgbaUnormSrgb),
            (hoi4_assets::DdsFormat::Bc3, false) => Some(wgpu::TextureFormat::Bc3RgbaUnorm),
            (hoi4_assets::DdsFormat::Bc5, _) => Some(wgpu::TextureFormat::Bc5RgUnorm),
            (hoi4_assets::DdsFormat::Bgra8, true) => Some(wgpu::TextureFormat::Bgra8UnormSrgb),
            (hoi4_assets::DdsFormat::Bgra8, false) => Some(wgpu::TextureFormat::Bgra8Unorm),
            _ => None,
        };
        if let Some(format) = format {
            if let Some(upload_plan) =
                dds_upload_plan(&dds).filter(|plan| plan.upload_mip_count > 0)
            {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(request.label),
                    size: wgpu::Extent3d {
                        width: dds.width,
                        height: dds.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: upload_plan.upload_mip_count,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                for mip in &upload_plan.mips {
                    let data = &dds.data[mip.offset..mip.offset + mip.size];
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: mip.level,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        data,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(mip.bytes_per_row),
                            rows_per_image: None,
                        },
                        wgpu::Extent3d {
                            width: mip.copy_width,
                            height: mip.copy_height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                let view = texture.create_view(&Default::default());
                return UploadedTexture {
                    texture,
                    view,
                    audit: BindingAuditEntry::vanilla(
                        request.pass,
                        request.binding,
                        request.role,
                        true,
                        request.critical,
                        None,
                        request.visual_impact,
                    ),
                };
            }
            fallback_reason = Some("dds_no_uploadable_mips".to_string());
        } else {
            fallback_reason = Some(format!("unsupported_dds_format:{:?}", dds.format));
        }
    }

    let reason = fallback_reason.unwrap_or_else(|| "unknown_fallback".to_string());
    warnings.push(format!(
        "[{}] {} ({}) using 1x1 fallback: {}",
        request.pass, request.binding, path, reason
    ));
    let (texture, view) = create_1x1_rgba(
        device,
        queue,
        request.label,
        request.fallback_rgba,
        request.srgb,
    );
    UploadedTexture {
        texture,
        view,
        audit: BindingAuditEntry::vanilla(
            request.pass,
            request.binding,
            request.role,
            false,
            request.critical,
            Some(reason),
            request.visual_impact,
        ),
    }
}

pub fn create_1x1_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    rgba: [u8; 4],
    srgb: bool,
) -> (wgpu::Texture, wgpu::TextureView) {
    let format = if srgb {
        wgpu::TextureFormat::Rgba8UnormSrgb
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&Default::default());
    (texture, view)
}

pub fn create_dynamic_target_1x1(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    rgba: [u8; 4],
) -> (wgpu::Texture, wgpu::TextureView) {
    create_1x1_rgba(device, queue, label, rgba, false)
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
            ch if ch.is_control() => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out
}

fn write_json_string_option(out: &mut String, key: &str, value: Option<&str>, indent: usize) {
    let padding = " ".repeat(indent);
    match value {
        Some(value) => {
            let _ = writeln!(out, "{}\"{}\": \"{}\",", padding, key, json_escape(value));
        }
        None => {
            let _ = writeln!(out, "{}\"{}\": null,", padding, key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_views() -> VanillaResourceViews {
        VanillaResourceViews {
            map_set: Arc::new(VanillaMapSet {
                entries: Vec::new(),
            }),
            posteffect_volumes: None,
            posteffect_lut_count: 0,
            posteffect_lut_loaded: 0,
            posteffect_lut_warnings: Vec::new(),
        }
    }

    #[test]
    fn binding_audit_counts_critical_mocks_and_fallbacks() {
        let mut audit = BindingAudit::new();
        audit.extend([
            BindingAuditEntry::vanilla(
                "terrain",
                "terrain_atlas",
                MapResRole::TerrainAtlas(0),
                true,
                true,
                None,
                "terrain surface",
            ),
            BindingAuditEntry::vanilla(
                "water",
                "lean1",
                MapResRole::Lean1,
                false,
                true,
                Some("missing_resource".to_string()),
                "water normals",
            ),
            BindingAuditEntry::mock(
                "terrain",
                "light_data",
                "light_data_mock",
                "not implemented",
                "night lighting disabled",
                BindingBlockingLevel::Critical,
            ),
        ]);
        assert_eq!(audit.total(), 3);
        assert_eq!(audit.fallback_count(), 1);
        assert_eq!(audit.mock_count(), 1);
        assert_eq!(audit.critical_count(), 2);
        assert!(audit
            .to_json_report()
            .contains("\"blocking_level\": \"critical\""));
    }

    #[test]
    fn terrain_binding_no_critical_mock_in_parity() {
        let audit = empty_views().phase1_binding_audit();
        let critical_mocks = audit
            .entries
            .iter()
            .filter(|entry| entry.pass == "terrain" && entry.is_critical_mock_in_parity())
            .count();
        assert_eq!(critical_mocks, 0);
        assert!(audit.entries.iter().any(|entry| {
            entry.pass == "terrain"
                && entry.binding == "light_data"
                && entry.source_kind == BindingSourceKind::DynamicTarget
                && entry.source_name == "LightDataMap"
                && entry.provenance == BindingProvenance::ProjectGenerated
                && entry.binding_present
                && !entry.asset_loaded
                && !entry.producer_equivalent
                && entry.blocking_level == BindingBlockingLevel::Degraded
        }));
    }

    #[test]
    fn water_binding_no_critical_mock_in_parity() {
        let audit = empty_views().phase1_binding_audit();
        let critical_mocks = audit
            .entries
            .iter()
            .filter(|entry| entry.pass == "water" && entry.is_critical_mock_in_parity())
            .count();
        assert_eq!(critical_mocks, 0);
    }

    #[test]
    fn vanilla_resource_views_declares_phase1_groups() {
        assert_eq!(VanillaResourceViews::terrain_atlas_roles().len(), 3);
        assert_eq!(VanillaResourceViews::terrain_atlas_normal_roles().len(), 3);
        assert_eq!(VanillaResourceViews::mud_diffuse_roles().len(), 3);
        assert_eq!(VanillaResourceViews::mud_normal_roles().len(), 3);
        assert_eq!(VanillaResourceViews::citylights_roles().len(), 3);
        assert!(VanillaResourceViews::colormap_roles().contains(&MapResRole::ColormapEmissive));
        assert!(VanillaResourceViews::water_resource_roles().contains(&MapResRole::Lean1));
        assert!(VanillaResourceViews::river_resource_roles().contains(&MapResRole::RiverMasks));
        assert_eq!(
            VanillaResourceViews::tree_resource_roles(),
            [
                MapResRole::TreesMask,
                MapResRole::TreeSeason,
                MapResRole::TreeTint,
                MapResRole::ColormapEmissive
            ]
        );
        assert_eq!(VanillaResourceViews::border_texture_roles().len(), 18);
    }

    #[test]
    fn postprocess_color_cube_fallback_is_not_mock() {
        let audit = empty_views().phase1_binding_audit();
        let color_cube = audit
            .entries
            .iter()
            .find(|entry| entry.pass == "postprocess" && entry.binding == "color_cube")
            .unwrap();
        assert_eq!(color_cube.source_kind, BindingSourceKind::ProjectFallback);
        assert_eq!(color_cube.source_name, "identity_color_cube");
        assert_eq!(color_cube.blocking_level, BindingBlockingLevel::Degraded);
        assert_eq!(audit.mock_count(), 0);
    }

    #[test]
    fn map_audit_runtime_targets_include_phase4_metadata() {
        let audit = empty_views().phase1_binding_audit();
        for target in [
            "GradientBorderChannel1",
            "GradientBorderChannel2",
            "GradientBorderChannel3",
            "ProvinceSecondaryColorMap",
            "IntelMap",
            "ShadowMap",
            "FOW",
            "MudSnow",
            "LightDataMap",
            "LightIndexMap",
        ] {
            let entry = audit
                .entries
                .iter()
                .find(|entry| entry.source_name == target)
                .unwrap_or_else(|| panic!("missing runtime target audit entry: {target}"));
            assert_eq!(entry.source_kind, BindingSourceKind::DynamicTarget);
            assert_eq!(entry.mock_name, None);
            assert!(entry.binding_present, "{target} binding must be present");
            assert!(
                !entry.asset_loaded,
                "{target} is a runtime target, not a static loaded asset"
            );
            assert!(entry.resource_format.is_some(), "{target} missing format");
            assert!(
                entry.resource_dimensions.is_some(),
                "{target} missing dimensions"
            );
            assert!(
                entry.source_trace.as_deref().is_some_and(|trace| {
                    trace.contains("runtime_targets.json") || trace.contains("reverse_out/")
                }),
                "{target} missing runtime trace provenance"
            );
            assert!(
                entry.parity_status.is_some(),
                "{target} missing parity status"
            );
            assert!(
                entry.producer_status.is_some(),
                "{target} missing producer status"
            );
        }

        for fallback in [
            "GradientBorderChannel1",
            "GradientBorderChannel2",
            "ProvinceSecondaryColorMap",
            "ShadowMap",
            "FOW",
            "MudSnow",
        ] {
            let entry = audit
                .entries
                .iter()
                .find(|entry| entry.source_name == fallback)
                .unwrap();
            assert!(
                entry.parity_status.as_deref().is_some_and(|status| {
                    status.contains("fallback")
                        || status.contains("project-generated")
                        || status.contains("non-parity")
                }),
                "{fallback} must be explicitly marked fallback or non-parity"
            );
            assert!(
                !entry.producer_equivalent,
                "{fallback} must not count as vanilla producer parity"
            );
        }

        let mud_snow = audit
            .entries
            .iter()
            .find(|entry| entry.source_name == "MudSnow")
            .unwrap();
        assert_eq!(mud_snow.provenance, BindingProvenance::NeutralFallback);
        assert_eq!(
            mud_snow.producer_status.as_deref(),
            Some("SnowMudData=missing_game_state_zero_fallback_non_parity")
        );
        assert!(mud_snow
            .resource_dimensions
            .as_deref()
            .is_some_and(|value| value.contains("1408x512")));

        let shadow = audit
            .entries
            .iter()
            .find(|entry| entry.source_name == "ShadowMap")
            .unwrap();
        assert_eq!(shadow.provenance, BindingProvenance::NeutralFallback);
        assert_eq!(
            shadow.producer_status.as_deref(),
            Some("ShadowMap_ProjectFOW=missing/fallback")
        );
    }
}
