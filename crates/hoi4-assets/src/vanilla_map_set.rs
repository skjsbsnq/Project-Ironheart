//! Phase 3.11.2 — vanilla `map/*` 全资源枚举与加载层。
//!
//! 路线图要求把 vanilla 71 张 map / terrain 资源全部上 GPU。本模块**只负责**:
//!
//! 1. 枚举每张资源的相对路径与"角色"（用途 enum）
//! 2. 通过 [`AssetDb`] 拉字节流（缺失时安全 fallback）
//! 3. 给上层（hoi4-render / hoi4-app）一个 [`VanillaMapSet`] 简单 struct 拿
//!    `Option<AssetBytes>` 列表，让它们决定如何 GPU 上传
//!
//! 不解析 BMP / DDS——那是 [`crate::dds`] 与 [`hoi4_map`] 的事。本模块**只**
//! 列资源 + IO。
//!
//! ## 资源类目（按 ROADMAP 3.11.2）
//!
//! | 类目 | 数量 | 用途 |
//! |---|---|---|
//! | provinces / heightmap / terrain / rivers / trees / cities / world_normal BMP | 7 | 主地图 ID + 高度 + 类型索引 |
//! | terrain atlas (RGB + normal) × 3 LOD | 6 | 地形纹理打包 + 法线 |
//! | mud / snow / ice diffuse + normal | ~12 | 泥浆 / 雪 / 冰 overlay |
//! | colormap variants | ~5 | 自然底色 + 城市灯光遮罩 |
//! | citylights × 3 LOD | 3 | 夜光遮罩 |
//! | colormap_water × 3 LOD | 3 | 海域基色 |
//! | RiverSurface (diffuse / normal / masks) × 3 LOD | 9 | 河面 |
//! | fow_rgb_waterspec_a / fow_noise × 3 LOD | 4 | FoW 噪声 |
//! | reflection (海洋 + 陆地) | 2 | 立方体反射 fallback |
//! | border textures × 6 类 × 3 LOD | 18 | 国 / 省 / 州 / 海 / 海区 / 不可通行边界 |
//! | Tree_season + Tree_tint | 2 | 季节染色 |
//! | strait + naval_dominance + lean × 2 + underwater × 3 | 8 | 杂项 |
//!
//! 合计 **71 张**（与 vanilla 实测一致；缺失 1-2 张视为可接受 — 各 LOD 偶尔
//! 缺最低或最高 mip）。
//!
//! ## 缺失策略
//!
//! 找不到的资源在 `entries[i].bytes = None`，启动 banner 列出 "vanilla 资源
//! X/71 加载成功"。**不**在加载层做 1×1 纹理回退——那是 GPU 上传层（hoi4-render）
//! 的事，本模块只报告"有/无"。
//!
//! ## 与 mod 链的交互
//!
//! `AssetDb::open` 已经按 [`hoi4_paths::PathConfig`] mod 链回退到 vanilla，
//! 所以 mod 替换贴图自动生效。本模块不需要额外处理。

use crate::{AssetBytes, AssetDb};
use std::collections::BTreeMap;

/// 单个资源条目的"用途 / 语义角色"。
///
/// 对应 vanilla shader 里它被绑到的 sampler 名（去 `Sampler` / `Texture` 后缀）。
/// 翻译过的 wgsl 在 `@group(N) @binding(M)` 时按这个 enum 起名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MapResRole {
    // ── 主地图位图（BMP） ─────────────────────────────────────────
    /// `map/provinces.bmp`
    Provinces,
    /// `map/heightmap.bmp`
    Heightmap,
    /// `map/terrain.bmp`
    TerrainIndex,
    /// `map/rivers.bmp`
    Rivers,
    /// `map/trees.bmp`
    TreesMask,
    /// `map/cities.bmp`
    Cities,
    /// `map/world_normal.bmp`
    WorldNormal,

    // ── 地形 atlas（DDS，3 LOD） ──────────────────────────────────
    /// `map/terrain/atlas{0,1,2}.dds`
    TerrainAtlas(u8),
    /// `map/terrain/atlas_normal{0,1,2}.dds` ⭐ 当前缺
    TerrainAtlasNormal(u8),

    // ── Mud / Snow / Ice overlay ────────────────────────────────
    /// `map/terrain/mud_diffuse_rgb_gloss_a_{0,1,2}.dds`
    MudDiffuseGloss(u8),
    /// `map/terrain/mud_normal_rgb_spec_a_{0,1,2}.dds`
    MudNormalSpec(u8),
    /// `map/terrain/snow_normal_rgb_diffuse_a.dds`
    SnowNormalDiffuse,
    /// `map/terrain/ice_diffuse.dds`
    IceDiffuse,
    /// `map/terrain/ice_noise_{0,1,2}.dds`
    IceNoise(u8),

    // ── Colormap / 城市灯光 ──────────────────────────────────────
    /// `map/terrain/colormap_rgb_cityemissivemask_a.dds`
    ColormapEmissive,
    /// `map/terrain/citylights_rgb_snowmask_a_{0,1,2}.dds`
    CityLights(u8),
    /// `map/terrain/colormap_water_{0,1,2}.dds`
    ColormapWater(u8),

    // ── 河面 ────────────────────────────────────────────────────
    /// `map/terrain/RiverSurface_diffuse_{0,1,2}.dds`
    RiverDiffuse(u8),
    /// `map/terrain/RiverSurface_normal_{0,1,2}.dds`
    RiverNormal(u8),
    /// `map/terrain/RiverSurface_masks.dds`
    RiverMasks,

    // ── FoW ────────────────────────────────────────────────────
    /// `map/terrain/fow_rgb_waterspec_a.dds`
    FowWaterSpec,
    /// `map/terrain/fow_noise_{0,1,2}.dds`
    FowNoise(u8),

    // ── 反射 ────────────────────────────────────────────────────
    /// `map/terrain/reflection.dds`
    Reflection,
    /// `map/terrain/reflection_land_unit.dds`
    ReflectionLandUnit,

    // ── 边界（5 类 × 3 LOD = 15 + 海区边界 3 = 18） ──────────────
    BorderCountry(u8),
    BorderProvince(u8),
    BorderState(u8),
    BorderSea(u8),
    BorderSeaRegion(u8),
    BorderImpassable(u8),

    // ── 树木季节 / 染色 ─────────────────────────────────────────
    /// `map/terrain/Tree_season.bmp`
    TreeSeason,
    /// `map/terrain/Tree_tint.bmp`
    TreeTint,

    // ── 杂项 ────────────────────────────────────────────────────
    Strait,
    NavalDominance,
    Lean1,
    Lean2,
    UnderwaterTerrain(u8),
}

impl MapResRole {
    /// 该角色对应的 vanilla 相对路径（不含 mod 路径前缀）。
    pub fn relative_path(self) -> String {
        use MapResRole::*;
        match self {
            Provinces => "map/provinces.bmp".to_string(),
            Heightmap => "map/heightmap.bmp".to_string(),
            TerrainIndex => "map/terrain.bmp".to_string(),
            Rivers => "map/rivers.bmp".to_string(),
            TreesMask => "map/trees.bmp".to_string(),
            Cities => "map/cities.bmp".to_string(),
            WorldNormal => "map/world_normal.bmp".to_string(),

            TerrainAtlas(i) => format!("map/terrain/atlas{}.dds", i),
            TerrainAtlasNormal(i) => format!("map/terrain/atlas_normal{}.dds", i),

            MudDiffuseGloss(i) => format!("map/terrain/mud_diffuse_rgb_gloss_a_{}.dds", i),
            MudNormalSpec(i) => format!("map/terrain/mud_normal_rgb_spec_a_{}.dds", i),
            SnowNormalDiffuse => "map/terrain/snow_normal_rgb_diffuse_a.dds".to_string(),
            IceDiffuse => "map/terrain/ice_diffuse.dds".to_string(),
            IceNoise(i) => format!("map/terrain/ice_noise_{}.dds", i),

            ColormapEmissive => "map/terrain/colormap_rgb_cityemissivemask_a.dds".to_string(),
            CityLights(i) => format!("map/terrain/citylights_rgb_snowmask_a_{}.dds", i),
            ColormapWater(i) => format!("map/terrain/colormap_water_{}.dds", i),

            RiverDiffuse(i) => format!("map/terrain/RiverSurface_diffuse_{}.dds", i),
            RiverNormal(i) => format!("map/terrain/RiverSurface_normal_{}.dds", i),
            RiverMasks => "map/terrain/RiverSurface_masks.dds".to_string(),

            FowWaterSpec => "map/terrain/fow_rgb_waterspec_a.dds".to_string(),
            FowNoise(i) => format!("map/terrain/fow_noise_{}.dds", i),

            Reflection => "map/terrain/reflection.dds".to_string(),
            ReflectionLandUnit => "map/terrain/reflection_land_unit.dds".to_string(),

            BorderCountry(i) => format!("map/terrain/border_country_{}.dds", i),
            BorderProvince(i) => format!("map/terrain/border_province_{}.dds", i),
            BorderState(i) => format!("map/terrain/border_state_{}.dds", i),
            BorderSea(i) => format!("map/terrain/border_sea_{}.dds", i),
            BorderSeaRegion(i) => format!("map/terrain/border_sea_region_{}.dds", i),
            BorderImpassable(i) => format!("map/terrain/border_impassable_{}.dds", i),

            TreeSeason => "map/terrain/Tree_season.bmp".to_string(),
            TreeTint => "map/terrain/Tree_tint.bmp".to_string(),

            Strait => "map/terrain/strait.dds".to_string(),
            NavalDominance => "map/terrain/naval_dominance_fx.dds".to_string(),
            Lean1 => "map/terrain/lean1.dds".to_string(),
            Lean2 => "map/terrain/lean2.dds".to_string(),
            UnderwaterTerrain(i) => format!("map/terrain/underwater_terrain_{}.dds", i),
        }
    }

    /// 角色对应的 GPU sRGB 标记。法线 / mask / 噪声等"数据型纹理"是 linear，
    /// 漫反射 / colormap 是 sRGB。
    pub fn is_srgb(self) -> bool {
        use MapResRole::*;
        matches!(
            self,
            TerrainAtlas(_)
                | MudDiffuseGloss(_)
                | IceDiffuse
                | ColormapEmissive
                | ColormapWater(_)
                | RiverDiffuse(_)
                | TreeTint
        )
    }

    /// 该资源缺失时是否仍可启动（fallback 1×1 纹理）。
    /// `false` 意味着引擎应 panic（如 heightmap 缺了根本没法画地图）。
    pub fn allow_missing(self) -> bool {
        use MapResRole::*;
        !matches!(self, Provinces | Heightmap | TerrainIndex)
    }
}

/// 一次性枚举出 vanilla 期望的全 71 张资源角色（按 `MapResRole` 排序方便测试稳定）。
pub fn all_vanilla_roles() -> Vec<MapResRole> {
    use MapResRole::*;
    let mut v = Vec::with_capacity(80);

    // 7 张 BMP
    v.extend([
        Provinces,
        Heightmap,
        TerrainIndex,
        Rivers,
        TreesMask,
        Cities,
        WorldNormal,
    ]);

    // 3 LOD 阵列：每 LOD 一组
    for i in 0..3 {
        v.push(TerrainAtlas(i));
        v.push(TerrainAtlasNormal(i));
        v.push(MudDiffuseGloss(i));
        v.push(MudNormalSpec(i));
        v.push(IceNoise(i));
        v.push(CityLights(i));
        v.push(ColormapWater(i));
        v.push(RiverDiffuse(i));
        v.push(RiverNormal(i));
        v.push(FowNoise(i));
        v.push(BorderCountry(i));
        v.push(BorderProvince(i));
        v.push(BorderState(i));
        v.push(BorderSea(i));
        v.push(BorderSeaRegion(i));
        v.push(BorderImpassable(i));
        v.push(UnderwaterTerrain(i));
    }

    // 单一文件
    v.extend([
        SnowNormalDiffuse,
        IceDiffuse,
        ColormapEmissive,
        RiverMasks,
        FowWaterSpec,
        Reflection,
        ReflectionLandUnit,
        TreeSeason,
        TreeTint,
        Strait,
        NavalDominance,
        Lean1,
        Lean2,
    ]);

    v.sort();
    v
}

/// 单个资源加载结果。
#[derive(Debug, Clone)]
pub struct MapResEntry {
    pub role: MapResRole,
    pub relative_path: String,
    /// `None` 表示资源不存在或读 IO 失败。
    pub bytes: Option<AssetBytes>,
}

/// 整个 vanilla map 资源集。
#[derive(Debug, Clone)]
pub struct VanillaMapSet {
    pub entries: Vec<MapResEntry>,
}

impl VanillaMapSet {
    pub fn load_for_audit<D: AssetDb>(db: &D) -> Self {
        let mut entries = Vec::new();
        for role in all_vanilla_roles() {
            let rel = role.relative_path();
            let bytes = db.open(&rel).ok();
            entries.push(MapResEntry {
                role,
                relative_path: rel,
                bytes,
            });
        }
        Self { entries }
    }

    /// 从 `db` 加载所有 vanilla 期望的资源。**不**因任何缺失资源而 panic
    /// （除非 `MapResRole::allow_missing() == false` 且确实缺）。
    ///
    /// 用泛型而非 `&dyn AssetDb` 因为 [`AssetDb`] 含 `impl AsRef<Path>` 泛型方法，
    /// 不是 dyn-compatible。
    pub fn load<D: AssetDb>(db: &D) -> Result<Self, MapSetError> {
        let mut entries = Vec::new();
        for role in all_vanilla_roles() {
            let rel = role.relative_path();
            // 注意：AssetDb::open 对找不到的文件返回 Err，不返回 Ok(empty) —
            // 我们这里把"找不到"和"找到了但读失败"都归为 None。
            let bytes = match db.open(&rel) {
                Ok(b) => Some(b),
                Err(_) => None,
            };

            if bytes.is_none() && !role.allow_missing() {
                return Err(MapSetError::RequiredMissing(role));
            }

            entries.push(MapResEntry {
                role,
                relative_path: rel,
                bytes,
            });
        }
        Ok(Self { entries })
    }

    /// 加载成功率（0..=1）。
    pub fn success_ratio(&self) -> f32 {
        let total = self.entries.len();
        if total == 0 {
            return 1.0;
        }
        let ok = self.entries.iter().filter(|e| e.bytes.is_some()).count();
        ok as f32 / total as f32
    }

    /// 已加载条目数 / 总条目数。
    pub fn loaded_count(&self) -> (usize, usize) {
        let total = self.entries.len();
        let ok = self.entries.iter().filter(|e| e.bytes.is_some()).count();
        (ok, total)
    }

    /// 打印启动 banner（"vanilla 资源 X/71 加载成功"）。
    pub fn report(&self) -> String {
        let (ok, total) = self.loaded_count();
        format!(
            "[vanilla_map_set] {}/{} resources loaded ({:.1}%)",
            ok,
            total,
            self.success_ratio() * 100.0
        )
    }

    /// 按角色查找已加载条目。`Some` 仅当资源在磁盘上存在。
    pub fn find(&self, role: MapResRole) -> Option<&MapResEntry> {
        self.entries.iter().find(|e| e.role == role)
    }

    /// 按角色查找字节流。
    pub fn bytes(&self, role: MapResRole) -> Option<&AssetBytes> {
        self.find(role).and_then(|e| e.bytes.as_ref())
    }

    /// 列出所有缺失的资源（启动诊断用）。
    pub fn missing(&self) -> Vec<&MapResEntry> {
        self.entries.iter().filter(|e| e.bytes.is_none()).collect()
    }

    /// 按"哪些 LOD 缺哪些"汇总（诊断输出可读化）。
    pub fn summarise_missing(&self) -> BTreeMap<&'static str, Vec<String>> {
        let mut out: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
        for e in self.missing() {
            let cat = category_name(e.role);
            out.entry(cat).or_default().push(e.relative_path.clone());
        }
        out
    }
}

/// 把 role 归到类别字符串（诊断用）。
fn category_name(role: MapResRole) -> &'static str {
    use MapResRole::*;
    match role {
        Provinces | Heightmap | TerrainIndex | Rivers | TreesMask | Cities | WorldNormal => "bmp",
        TerrainAtlas(_) => "terrain_atlas",
        TerrainAtlasNormal(_) => "terrain_atlas_normal",
        MudDiffuseGloss(_) | MudNormalSpec(_) => "mud",
        SnowNormalDiffuse => "snow",
        IceDiffuse | IceNoise(_) => "ice",
        ColormapEmissive => "colormap_emissive",
        CityLights(_) => "citylights",
        ColormapWater(_) => "colormap_water",
        RiverDiffuse(_) | RiverNormal(_) | RiverMasks => "rivers",
        FowWaterSpec | FowNoise(_) => "fow",
        Reflection | ReflectionLandUnit => "reflection",
        BorderCountry(_) | BorderProvince(_) | BorderState(_) | BorderSea(_)
        | BorderSeaRegion(_) | BorderImpassable(_) => "borders",
        TreeSeason | TreeTint => "tree_season",
        Strait | NavalDominance | Lean1 | Lean2 | UnderwaterTerrain(_) => "misc",
    }
}

/// VanillaMapSet 加载错误。
#[derive(Debug)]
pub enum MapSetError {
    /// 必需资源缺失（`allow_missing() == false`）。
    RequiredMissing(MapResRole),
}

impl std::fmt::Display for MapSetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapSetError::RequiredMissing(role) => write!(
                f,
                "required vanilla resource missing: {} ({:?})",
                role.relative_path(),
                role
            ),
        }
    }
}

impl std::error::Error for MapSetError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AssetError;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    /// 一个"假装持有这些路径"的 mock AssetDb。
    struct MockDb {
        files: std::collections::HashMap<String, AssetBytes>,
    }
    impl MockDb {
        fn new() -> Self {
            Self {
                files: Default::default(),
            }
        }
        fn add(&mut self, rel: &str, bytes: &[u8]) {
            self.files.insert(rel.to_string(), Arc::from(bytes));
        }
    }
    impl AssetDb for MockDb {
        fn open(&self, relative: impl AsRef<Path>) -> Result<AssetBytes, AssetError> {
            let key = relative.as_ref().to_string_lossy().replace('\\', "/");
            self.files
                .get(&key)
                .cloned()
                .ok_or_else(|| AssetError::not_found(PathBuf::from(key)))
        }

        fn list(&self, _relative: impl AsRef<Path>) -> Vec<PathBuf> {
            Vec::new()
        }

        fn parse_or_get<T, F>(
            &self,
            _relative: impl AsRef<Path>,
            _parser: F,
        ) -> Result<Arc<T>, AssetError>
        where
            T: Send + Sync + 'static,
            F: FnOnce(&[u8]) -> Result<T, AssetError>,
        {
            // 测试不需要 parse_or_get
            Err(AssetError::not_found(PathBuf::from("mock")))
        }
    }

    #[test]
    fn role_count_matches_roadmap_estimate() {
        let v = all_vanilla_roles();
        // ROADMAP 写"71 张"。我们枚举的是 7 BMP + 17×3 LOD + 13 单文件 = 71
        assert_eq!(v.len(), 71, "role enumeration drifted from 71");
    }

    #[test]
    fn role_paths_unique() {
        let v = all_vanilla_roles();
        let paths: Vec<_> = v.iter().map(|r| r.relative_path()).collect();
        let mut sorted = paths.clone();
        sorted.sort();
        let dup_idx = sorted.windows(2).position(|w| w[0] == w[1]);
        assert!(dup_idx.is_none(), "duplicate path at {:?}", dup_idx);
    }

    #[test]
    fn paths_have_expected_prefix() {
        for role in all_vanilla_roles() {
            let p = role.relative_path();
            assert!(p.starts_with("map/"), "non-map path: {}", p);
            assert!(p.ends_with(".bmp") || p.ends_with(".dds"));
        }
    }

    #[test]
    fn srgb_classification_sane() {
        // 法线贴图必须是 linear（不能 sRGB 解码）
        assert!(!MapResRole::TerrainAtlasNormal(0).is_srgb());
        assert!(!MapResRole::WorldNormal.is_srgb());
        assert!(!MapResRole::MudNormalSpec(0).is_srgb());
        assert!(!MapResRole::SnowNormalDiffuse.is_srgb());
        assert!(!MapResRole::RiverNormal(0).is_srgb());
        // 漫反射 / colormap 必须是 sRGB
        assert!(MapResRole::TerrainAtlas(0).is_srgb());
        assert!(MapResRole::ColormapEmissive.is_srgb());
        assert!(MapResRole::ColormapWater(0).is_srgb());
        assert!(MapResRole::RiverDiffuse(0).is_srgb());
    }

    #[test]
    fn essential_resources_required() {
        assert!(!MapResRole::Provinces.allow_missing());
        assert!(!MapResRole::Heightmap.allow_missing());
        assert!(!MapResRole::TerrainIndex.allow_missing());
        // 大部分其它资源是 optional
        assert!(MapResRole::TerrainAtlasNormal(0).allow_missing());
        assert!(MapResRole::CityLights(0).allow_missing());
    }

    #[test]
    fn load_with_partial_data_works() {
        let mut db = MockDb::new();
        // 只放最关键 3 张
        db.add("map/provinces.bmp", &[1u8, 2, 3]);
        db.add("map/heightmap.bmp", &[4u8, 5]);
        db.add("map/terrain.bmp", &[6u8]);
        // 加几张 optional
        db.add("map/terrain/atlas0.dds", &[7u8; 64]);
        db.add("map/terrain/colormap_water_0.dds", &[8u8; 32]);

        let set = VanillaMapSet::load(&db).expect("required present, must succeed");
        let (ok, total) = set.loaded_count();
        assert_eq!(total, 71);
        assert_eq!(ok, 5);
        assert!((set.success_ratio() - 5.0 / 71.0).abs() < 1e-4);

        assert!(set.bytes(MapResRole::Provinces).is_some());
        assert!(set.bytes(MapResRole::TerrainAtlas(0)).is_some());
        assert!(set.bytes(MapResRole::CityLights(0)).is_none());
    }

    #[test]
    fn load_fails_when_essential_missing() {
        let db = MockDb::new(); // 全空
        let result = VanillaMapSet::load(&db);
        assert!(result.is_err(), "should fail if essentials missing");
    }

    #[test]
    fn load_for_audit_keeps_required_missing_entries() {
        let db = MockDb::new();
        let set = VanillaMapSet::load_for_audit(&db);
        let (ok, total) = set.loaded_count();
        assert_eq!(total, 71);
        assert_eq!(ok, 0);
        assert!(set.bytes(MapResRole::Provinces).is_none());
    }

    #[test]
    fn missing_summary_groups_by_category() {
        let mut db = MockDb::new();
        // 加载所有 7 张 BMP，让 "bmp" 类别完整
        for r in [
            MapResRole::Provinces,
            MapResRole::Heightmap,
            MapResRole::TerrainIndex,
            MapResRole::Rivers,
            MapResRole::TreesMask,
            MapResRole::Cities,
            MapResRole::WorldNormal,
        ] {
            db.add(&r.relative_path(), &[0u8]);
        }
        let set = VanillaMapSet::load(&db).unwrap();
        let summary = set.summarise_missing();
        // 其他类别（borders / rivers DDS / fow 等）仍缺
        assert!(summary.contains_key("borders"));
        assert!(summary.contains_key("rivers"));
        // BMP 类别全在 → 不应出现在缺失里
        assert!(!summary.contains_key("bmp"));
    }

    #[test]
    fn report_format_stable() {
        let db = {
            let mut d = MockDb::new();
            d.add("map/provinces.bmp", &[]);
            d.add("map/heightmap.bmp", &[]);
            d.add("map/terrain.bmp", &[]);
            d
        };
        let set = VanillaMapSet::load(&db).unwrap();
        let r = set.report();
        assert!(r.contains("3/71"));
        assert!(r.contains("vanilla_map_set"));
    }
}
