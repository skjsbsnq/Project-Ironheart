use glam::Vec2;

use hoi4_render::camera::{Camera, CameraUniform};
use hoi4_render::railways::compute_province_centroids;
use hoi4_state::World;

use crate::render_collect::{self, ProvincePixelBounds, RenderCollectMapSpace};

pub(crate) const LEGACY_WORLD_SCALE: f32 = 0.02;
pub(crate) const LEGACY_HEIGHT_SCALE: f32 = 1.45;
pub(crate) const LEGACY_LAT_CORRECTION: f32 = 0.0;

#[derive(Clone, Debug)]
pub(crate) struct AppMapSpatialData {
    pub(crate) province_centroids_px: Vec<(f32, f32)>,
    pub(crate) province_pixel_bounds: Vec<Option<ProvincePixelBounds>>,
}

impl AppMapSpatialData {
    pub(crate) fn from_world(world: &World) -> Self {
        Self::from_province_map(&world.map.province_map)
    }

    pub(crate) fn from_province_map(province_map: &hoi4_map::ProvinceMap) -> Self {
        Self {
            province_centroids_px: compute_province_centroids(province_map),
            province_pixel_bounds: render_collect::build_province_pixel_bounds(province_map),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct MapSpaceCore {
    province_map_px: [u32; 2],
    heightmap_px: [u32; 2],
    world_scale: f32,
    height_scale: f32,
    lat_correction: f32,
}

impl MapSpaceCore {
    fn from_world(world: &World, world_scale: f32, height_scale: f32, lat_correction: f32) -> Self {
        Self {
            province_map_px: [world.map.province_map.width, world.map.province_map.height],
            heightmap_px: [world.map.heightmap.width, world.map.heightmap.height],
            world_scale,
            height_scale,
            lat_correction,
        }
    }

    fn camera_world_size(self) -> Vec2 {
        Vec2::new(
            self.province_map_px[0] as f32 * self.world_scale,
            self.province_map_px[1] as f32 * self.world_scale,
        )
    }

    fn world_scale(self) -> f32 {
        self.world_scale
    }

    fn height_scale(self) -> f32 {
        self.height_scale
    }

    fn camera_uniform_from(self, camera: &Camera) -> CameraUniform {
        CameraUniform::from_camera(camera, self.height_scale, self.lat_correction)
    }

    fn map_px_to_world_xz(self, map_px: Vec2) -> Vec2 {
        let map_w = (self.province_map_px[0] as f32).max(f32::EPSILON);
        let map_h = (self.province_map_px[1] as f32).max(f32::EPSILON);
        let world_size = self.camera_world_size();
        Vec2::new(
            map_px.x.rem_euclid(map_w) / map_w * world_size.x,
            map_px.y.clamp(0.0, map_h) / map_h * world_size.y,
        )
    }

    fn world_xz_to_map_px(self, world_xz: Vec2) -> Vec2 {
        let world_size = self.camera_world_size();
        let world_w = world_size.x.max(f32::EPSILON);
        let world_d = world_size.y.max(f32::EPSILON);
        Vec2::new(
            world_xz.x.rem_euclid(world_w) / world_w * self.province_map_px[0] as f32,
            world_xz.y.clamp(0.0, world_d) / world_d * self.province_map_px[1] as f32,
        )
    }

    fn height_raw_to_world_y(self, raw: u8) -> f32 {
        raw as f32 / 255.0 * self.height_scale
    }

    fn height_fraction_to_world_y(self, fraction: f32) -> f32 {
        fraction.clamp(0.0, 1.0) * self.height_scale
    }

    fn sea_level_world_y(self) -> f32 {
        self.height_raw_to_world_y(hoi4_map::Heightmap::SEA_LEVEL)
    }

    fn height_raw_at_world_xz(self, world: &World, world_xz: Vec2) -> Option<u8> {
        let world_size = self.camera_world_size();
        if world_size.x <= f32::EPSILON
            || world_size.y <= f32::EPSILON
            || world_xz.y < 0.0
            || world_xz.y > world_size.y
            || self.heightmap_px[0] == 0
            || self.heightmap_px[1] == 0
        {
            return None;
        }
        let u = world_xz.x.rem_euclid(world_size.x) / world_size.x;
        let v = world_xz.y / world_size.y;
        let hx = ((u * self.heightmap_px[0] as f32) as u32).min(self.heightmap_px[0] - 1);
        let hy = ((v * self.heightmap_px[1] as f32) as u32).min(self.heightmap_px[1] - 1);
        Some(world.map.heightmap.pixels[(hy * self.heightmap_px[0] + hx) as usize])
    }

    fn pick_province_at_world_xz(self, world: &World, world_xz: Vec2) -> Option<u32> {
        let pmap = &world.map.province_map;
        if pmap.width == 0 || pmap.height == 0 {
            return None;
        }
        let world_size = self.camera_world_size();
        if world_size.x <= f32::EPSILON
            || world_size.y <= f32::EPSILON
            || world_xz.y < 0.0
            || world_xz.y > world_size.y
        {
            return None;
        }
        let map_px = self.world_xz_to_map_px(world_xz);
        let px = (map_px.x as u32).min(pmap.width - 1);
        let py = (map_px.y as u32).min(pmap.height - 1);
        Some(pmap.pixels[(py * pmap.width + px) as usize] as u32)
    }

    fn render_collect_map_space(self) -> RenderCollectMapSpace {
        RenderCollectMapSpace::new(self.world_scale, self.height_scale)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LegacyMapSpace {
    core: MapSpaceCore,
}

impl LegacyMapSpace {
    pub(crate) fn from_world(world: &World) -> Self {
        Self {
            core: MapSpaceCore::from_world(
                world,
                LEGACY_WORLD_SCALE,
                LEGACY_HEIGHT_SCALE,
                LEGACY_LAT_CORRECTION,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CleanMapSpace {
    core: MapSpaceCore,
}

impl CleanMapSpace {
    pub(crate) fn from_world(world: &World) -> Self {
        Self {
            core: MapSpaceCore::from_world(
                world,
                LEGACY_WORLD_SCALE,
                LEGACY_HEIGHT_SCALE,
                LEGACY_LAT_CORRECTION,
            ),
        }
    }
}

macro_rules! impl_map_space {
    ($ty:ty) => {
        impl $ty {
            pub(crate) fn camera_world_size(&self) -> Vec2 {
                self.core.camera_world_size()
            }

            pub(crate) fn world_scale(&self) -> f32 {
                self.core.world_scale()
            }

            pub(crate) fn height_scale(&self) -> f32 {
                self.core.height_scale()
            }

            pub(crate) fn camera_uniform_from(&self, camera: &Camera) -> CameraUniform {
                self.core.camera_uniform_from(camera)
            }

            pub(crate) fn map_px_to_world_xz(&self, map_px: Vec2) -> Vec2 {
                self.core.map_px_to_world_xz(map_px)
            }

            pub(crate) fn world_xz_to_map_px(&self, world_xz: Vec2) -> Vec2 {
                self.core.world_xz_to_map_px(world_xz)
            }

            pub(crate) fn height_raw_to_world_y(&self, raw: u8) -> f32 {
                self.core.height_raw_to_world_y(raw)
            }

            pub(crate) fn height_fraction_to_world_y(&self, fraction: f32) -> f32 {
                self.core.height_fraction_to_world_y(fraction)
            }

            pub(crate) fn sea_level_world_y(&self) -> f32 {
                self.core.sea_level_world_y()
            }

            pub(crate) fn height_raw_at_world_xz(
                &self,
                world: &World,
                world_xz: Vec2,
            ) -> Option<u8> {
                self.core.height_raw_at_world_xz(world, world_xz)
            }

            pub(crate) fn pick_province_at_world_xz(
                &self,
                world: &World,
                world_xz: Vec2,
            ) -> Option<u32> {
                self.core.pick_province_at_world_xz(world, world_xz)
            }

            pub(crate) fn render_collect_map_space(&self) -> RenderCollectMapSpace {
                self.core.render_collect_map_space()
            }
        }
    };
}

impl_map_space!(LegacyMapSpace);
impl_map_space!(CleanMapSpace);

#[derive(Clone, Copy, Debug)]
pub(crate) enum ActiveMapSpace<'a> {
    Legacy(&'a LegacyMapSpace),
    Clean(&'a CleanMapSpace),
}

impl ActiveMapSpace<'_> {
    pub(crate) fn camera_world_size(self) -> Vec2 {
        match self {
            Self::Legacy(space) => space.camera_world_size(),
            Self::Clean(space) => space.camera_world_size(),
        }
    }

    pub(crate) fn world_scale(self) -> f32 {
        match self {
            Self::Legacy(space) => space.world_scale(),
            Self::Clean(space) => space.world_scale(),
        }
    }

    pub(crate) fn height_scale(self) -> f32 {
        match self {
            Self::Legacy(space) => space.height_scale(),
            Self::Clean(space) => space.height_scale(),
        }
    }

    pub(crate) fn camera_uniform_from(self, camera: &Camera) -> CameraUniform {
        match self {
            Self::Legacy(space) => space.camera_uniform_from(camera),
            Self::Clean(space) => space.camera_uniform_from(camera),
        }
    }

    pub(crate) fn map_px_to_world_xz(self, map_px: Vec2) -> Vec2 {
        match self {
            Self::Legacy(space) => space.map_px_to_world_xz(map_px),
            Self::Clean(space) => space.map_px_to_world_xz(map_px),
        }
    }

    pub(crate) fn world_xz_to_map_px(self, world_xz: Vec2) -> Vec2 {
        match self {
            Self::Legacy(space) => space.world_xz_to_map_px(world_xz),
            Self::Clean(space) => space.world_xz_to_map_px(world_xz),
        }
    }

    pub(crate) fn height_raw_to_world_y(self, raw: u8) -> f32 {
        match self {
            Self::Legacy(space) => space.height_raw_to_world_y(raw),
            Self::Clean(space) => space.height_raw_to_world_y(raw),
        }
    }

    pub(crate) fn height_fraction_to_world_y(self, fraction: f32) -> f32 {
        match self {
            Self::Legacy(space) => space.height_fraction_to_world_y(fraction),
            Self::Clean(space) => space.height_fraction_to_world_y(fraction),
        }
    }

    pub(crate) fn sea_level_world_y(self) -> f32 {
        match self {
            Self::Legacy(space) => space.sea_level_world_y(),
            Self::Clean(space) => space.sea_level_world_y(),
        }
    }

    pub(crate) fn height_raw_at_world_xz(self, world: &World, world_xz: Vec2) -> Option<u8> {
        match self {
            Self::Legacy(space) => space.height_raw_at_world_xz(world, world_xz),
            Self::Clean(space) => space.height_raw_at_world_xz(world, world_xz),
        }
    }

    pub(crate) fn pick_province_at_world_xz(self, world: &World, world_xz: Vec2) -> Option<u32> {
        match self {
            Self::Legacy(space) => space.pick_province_at_world_xz(world, world_xz),
            Self::Clean(space) => space.pick_province_at_world_xz(world, world_xz),
        }
    }

    pub(crate) fn render_collect_map_space(self) -> RenderCollectMapSpace {
        match self {
            Self::Legacy(space) => space.render_collect_map_space(),
            Self::Clean(space) => space.render_collect_map_space(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_app_spatial_data_from_province_map_builds_cpu_data() {
        let province_map = hoi4_map::ProvinceMap {
            width: 2,
            height: 2,
            pixels: vec![1, 1, 2, 2],
        };

        let spatial = AppMapSpatialData::from_province_map(&province_map);

        assert!(spatial.province_centroids_px.len() >= 3);
        assert!(spatial.province_pixel_bounds.len() >= 3);
        assert!(spatial.province_pixel_bounds[1].is_some());
        assert!(spatial.province_pixel_bounds[2].is_some());
    }
}
