//! `hoi4-assets` — V5 收口后保留的资产管线骨架。
//!
//! V5（2026-05-18）放弃 vanilla GUI 全兼容路线后，本 crate 只保留三类基础设施：
//!
//! 1. **AssetDb 抽象**（[`AssetDb`] / [`FsAssetDb`]） — 按相对路径取一个 vanilla
//!    资产文件，缓存字节与解析结果。两层 LRU：原始字节 + 类型化结果。
//! 2. **二进制资产解析器** — DDS（地图 / 国旗 / sprite 纹理）、TGA（主菜单背景与
//!    旧 vanilla sprite 兼容路径）、PdxMesh（3D 模型）、`.asset` metadata、
//!    `vanilla_map_set`（地图必备文件白名单）。
//! 3. **`TextureBank`** — 显式路径 → wgpu 纹理缓存（V5 已去掉与 `.gfx`
//!    `SpriteDef` 联动，`get_or_load_path` 直接走 [`AssetDb::open`]）。
//!
//! ## V5 移除清单（不再属于本 crate 的职责）
//!
//! - `gui.rs` / `gui_rt_removed.rs` / `gfx.rs` / `fnt.rs` 全部删除
//! - 与 `.gui` AST / `.gfx` SpriteDef / `.fnt` BM-font 相关的所有 trait 与导出
//! - vanilla GUI 测试（`tests/vanilla_gui.rs` 等）

pub mod asset_meta;
mod cache;
mod db;
pub mod dds;
mod error;
pub mod generated_flags;
pub mod map_asset_audit;
pub mod pdx_mesh;
pub mod texture_bank;
pub mod tga;
pub mod vanilla_map_set;

pub use asset_meta::{AssetMetaIndex, ModelAsset, MusicAsset};
pub use cache::Cache;
pub use db::{AssetDb, FsAssetDb};
pub use dds::{
    compute_frame_uvs, dds_upload_layout, dds_upload_plan, mip_size, DdsFormat, DdsImage,
    DdsUploadMip, DdsUploadPlan, FrameUv,
};
pub use error::AssetError;
pub use generated_flags::generated_historical_flag;
pub use map_asset_audit::{
    audit_category_name, fallback_invalidates_visual_review, fallback_policy_for, requirement_for,
    MapAssetAudit, MapAssetAuditEntry, MapAssetCategoryStats, MapAssetDdsInfo,
    MapAssetFallbackPolicy, MapAssetFileKind, MapAssetMipStatus, MapAssetQuality,
    MapAssetRequirement,
};
pub use pdx_mesh::{MeshMaterial, PdxMesh, SubMesh};
pub use texture_bank::{dds_to_wgpu_format, GpuTexture, TextureBank, TextureEntry};
pub use tga::{load_tga, TgaImage};
pub use vanilla_map_set::{all_vanilla_roles, MapResEntry, MapResRole, MapSetError, VanillaMapSet};

/// 资产字节的零拷贝句柄。`Arc<[u8]>` 的语义别名 — 缓存命中时 `clone` 是 O(1) 引用计数。
pub type AssetBytes = std::sync::Arc<[u8]>;

/// 默认 LRU 容量（项数）。两个独立缓存各一份。
pub const DEFAULT_RAW_CACHE_CAP: usize = 256;
pub const DEFAULT_PARSED_CACHE_CAP: usize = 1024;
