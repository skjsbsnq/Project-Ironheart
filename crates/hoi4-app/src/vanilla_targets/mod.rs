pub mod fow;
pub mod gradient_border;
pub mod province_secondary;

use hoi4_render::map_mode::MapMode;
use hoi4_state::{CountryId, World};

use crate::vanilla_resource_views::BindingAuditEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTargetChannelSemantic {
    CountryBorderGradient,
    ProvinceBorderGradient,
    StateSeaImpassableGradient,
    ProvinceSecondaryColor,
    FogOfWar,
    MudSnow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTargetSpec {
    pub name: &'static str,
    pub format: wgpu::TextureFormat,
    pub bytes_per_pixel: u32,
    pub semantic: RuntimeTargetChannelSemantic,
}

pub const PHASE4_RUNTIME_TARGET_SPECS: [RuntimeTargetSpec; 6] = [
    RuntimeTargetSpec {
        name: "GradientBorderChannel1",
        format: wgpu::TextureFormat::R8Unorm,
        bytes_per_pixel: 1,
        semantic: RuntimeTargetChannelSemantic::CountryBorderGradient,
    },
    RuntimeTargetSpec {
        name: "GradientBorderChannel2",
        format: wgpu::TextureFormat::R8Unorm,
        bytes_per_pixel: 1,
        semantic: RuntimeTargetChannelSemantic::ProvinceBorderGradient,
    },
    RuntimeTargetSpec {
        name: "GradientBorderChannel3",
        format: wgpu::TextureFormat::R8Unorm,
        bytes_per_pixel: 1,
        semantic: RuntimeTargetChannelSemantic::StateSeaImpassableGradient,
    },
    RuntimeTargetSpec {
        name: "ProvinceSecondaryColorMap",
        format: wgpu::TextureFormat::Rgba8Unorm,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::ProvinceSecondaryColor,
    },
    RuntimeTargetSpec {
        name: "FOW",
        format: wgpu::TextureFormat::Rgba8Unorm,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::FogOfWar,
    },
    RuntimeTargetSpec {
        name: "MudSnow",
        format: wgpu::TextureFormat::Rgba8Unorm,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::MudSnow,
    },
];

pub struct RuntimeTargetTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
}

impl RuntimeTargetTexture {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        bytes_per_pixel: u32,
        data: &[u8],
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let target = Self {
            texture,
            view,
            width,
            height,
            bytes_per_pixel,
        };
        target.write(queue, data);
        target
    }

    pub fn new_r8(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Self {
        Self::new(
            device,
            queue,
            label,
            width,
            height,
            wgpu::TextureFormat::R8Unorm,
            1,
            data,
        )
    }

    pub fn new_rgba8(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Self {
        Self::new(
            device,
            queue,
            label,
            width,
            height,
            wgpu::TextureFormat::Rgba8Unorm,
            4,
            data,
        )
    }

    pub fn write(&self, queue: &wgpu::Queue, data: &[u8]) {
        let expected = self.width as usize * self.height as usize * self.bytes_per_pixel as usize;
        debug_assert_eq!(data.len(), expected);
        if data.len() != expected || self.width == 0 || self.height == 0 {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * self.bytes_per_pixel),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

pub struct GradientBorderRuntimeTargets {
    pub ch1: RuntimeTargetTexture,
    pub ch2: RuntimeTargetTexture,
    pub ch3: RuntimeTargetTexture,
}

pub struct VanillaRuntimeTargets {
    pub gradient_border: GradientBorderRuntimeTargets,
    pub province_secondary_color: RuntimeTargetTexture,
    pub fow: RuntimeTargetTexture,
    pub mud_snow: RuntimeTargetTexture,
    province_secondary_signature: u64,
    mud_snow_signature: u64,
    province_secondary_data: Vec<u8>,
    mud_snow_data: Vec<u8>,
}

pub struct VanillaRuntimeTargetViews<'a> {
    pub gradient_border_ch1: &'a wgpu::TextureView,
    pub gradient_border_ch2: &'a wgpu::TextureView,
    pub gradient_border_ch3: &'a wgpu::TextureView,
    pub province_secondary_color: &'a wgpu::TextureView,
    pub fow: &'a wgpu::TextureView,
    pub mud_snow: &'a wgpu::TextureView,
}

#[derive(Clone, Copy)]
pub struct VanillaRuntimeTargetInputs<'a> {
    pub world: &'a World,
    pub country_sdf: &'a [u8],
    pub province_sdf: &'a [u8],
    pub coast_sdf: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct VanillaRuntimeTargetFrameParams {
    pub selected_province_id: u32,
    pub hovered_province_id: u32,
    pub map_mode: MapMode,
    pub player_country: Option<CountryId>,
    pub battle_plan_opacity: f32,
    pub naval_dominance_opacity: f32,
    pub occupation_opacity: f32,
    pub selected_opacity: f32,
    pub hover_opacity: f32,
    pub map_mode_overlay_opacity: f32,
    pub season_snow_offset: f32,
    pub season_blend: f32,
}

impl Default for VanillaRuntimeTargetFrameParams {
    fn default() -> Self {
        Self {
            selected_province_id: u32::MAX,
            hovered_province_id: u32::MAX,
            map_mode: MapMode::Political,
            player_country: None,
            battle_plan_opacity: 1.0,
            naval_dominance_opacity: 0.0,
            occupation_opacity: 1.0,
            selected_opacity: 1.0,
            hover_opacity: 1.0,
            map_mode_overlay_opacity: 0.0,
            season_snow_offset: 0.0,
            season_blend: 0.0,
        }
    }
}

impl VanillaRuntimeTargets {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        inputs: VanillaRuntimeTargetInputs<'_>,
    ) -> Self {
        let width = inputs.world.map.province_map.width;
        let height = inputs.world.map.province_map.height;
        let gradient_cpu = gradient_border::generate(inputs);
        let frame_params = VanillaRuntimeTargetFrameParams::default();
        let province_secondary_data = province_secondary::generate(inputs.world, &frame_params);
        let province_secondary_signature =
            province_secondary::signature(inputs.world, &frame_params);
        let mud_snow_data = fow::generate_mud_snow(
            &inputs.world.map.heightmap,
            frame_params.season_snow_offset,
            frame_params.season_blend,
        );
        let mud_snow_signature = fow::mud_snow_signature(&frame_params);
        let fow_data = fow::generate_default_fow(width, height);

        Self {
            gradient_border: GradientBorderRuntimeTargets {
                ch1: RuntimeTargetTexture::new_r8(
                    device,
                    queue,
                    "GradientBorderChannel1",
                    width,
                    height,
                    &gradient_cpu.ch1,
                ),
                ch2: RuntimeTargetTexture::new_r8(
                    device,
                    queue,
                    "GradientBorderChannel2",
                    width,
                    height,
                    &gradient_cpu.ch2,
                ),
                ch3: RuntimeTargetTexture::new_r8(
                    device,
                    queue,
                    "GradientBorderChannel3",
                    width,
                    height,
                    &gradient_cpu.ch3,
                ),
            },
            province_secondary_color: RuntimeTargetTexture::new_rgba8(
                device,
                queue,
                "ProvinceSecondaryColorMap",
                width,
                height,
                &province_secondary_data,
            ),
            fow: RuntimeTargetTexture::new_rgba8(device, queue, "FOW", width, height, &fow_data),
            mud_snow: RuntimeTargetTexture::new_rgba8(
                device,
                queue,
                "MudSnow",
                width,
                height,
                &mud_snow_data,
            ),
            province_secondary_signature,
            mud_snow_signature,
            province_secondary_data,
            mud_snow_data,
        }
    }

    pub fn views(&self) -> VanillaRuntimeTargetViews<'_> {
        VanillaRuntimeTargetViews {
            gradient_border_ch1: &self.gradient_border.ch1.view,
            gradient_border_ch2: &self.gradient_border.ch2.view,
            gradient_border_ch3: &self.gradient_border.ch3.view,
            province_secondary_color: &self.province_secondary_color.view,
            fow: &self.fow.view,
            mud_snow: &self.mud_snow.view,
        }
    }

    pub fn update_frame(
        &mut self,
        queue: &wgpu::Queue,
        world: &World,
        params: &VanillaRuntimeTargetFrameParams,
    ) {
        let secondary_signature = province_secondary::signature(world, params);
        if secondary_signature != self.province_secondary_signature {
            self.province_secondary_data = province_secondary::generate(world, params);
            self.province_secondary_color
                .write(queue, &self.province_secondary_data);
            self.province_secondary_signature = secondary_signature;
        }

        let mud_snow_signature = fow::mud_snow_signature(params);
        if mud_snow_signature != self.mud_snow_signature {
            self.mud_snow_data = fow::generate_mud_snow(
                &world.map.heightmap,
                params.season_snow_offset,
                params.season_blend,
            );
            self.mud_snow.write(queue, &self.mud_snow_data);
            self.mud_snow_signature = mud_snow_signature;
        }
    }

    pub fn binding_audit_entries_for_pass(&self, pass: &'static str) -> Vec<BindingAuditEntry> {
        let mut entries = vec![
            BindingAuditEntry::dynamic_target(
                pass,
                "gradient_border_ch1",
                "GradientBorderChannel1",
                "country border gradient is generated from runtime ownership state",
            ),
            BindingAuditEntry::dynamic_target(
                pass,
                "gradient_border_ch2",
                "GradientBorderChannel2",
                "province border gradient is generated from runtime province topology",
            ),
            BindingAuditEntry::dynamic_target(
                pass,
                "gradient_border_ch3",
                "GradientBorderChannel3",
                "state, coast, and impassable border gradient is generated at runtime",
            ),
            BindingAuditEntry::dynamic_target(
                pass,
                "province_secondary_color",
                "ProvinceSecondaryColorMap",
                "occupation, battle plans, selection, hover, map-mode tint, and naval dominance use a runtime map target",
            ),
            BindingAuditEntry::dynamic_target(
                pass,
                "fow",
                "FOW",
                "fog-of-war visibility is supplied as a runtime map target",
            ),
        ];
        if pass == "terrain" {
            entries.push(BindingAuditEntry::dynamic_target(
                pass,
                "mud_snow",
                "MudSnow",
                "mud and snow masks are supplied as a runtime map target",
            ));
        } else if pass == "tree" {
            entries.push(BindingAuditEntry::dynamic_target(
                pass,
                "mud_snow",
                "MudSnow",
                "tree material receives the shared snow mask runtime target",
            ));
        }
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn phase4_runtime_target_specs_cover_required_targets() {
        let names: HashSet<&'static str> = PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .map(|spec| spec.name)
            .collect();
        for required in [
            "GradientBorderChannel1",
            "GradientBorderChannel2",
            "GradientBorderChannel3",
            "ProvinceSecondaryColorMap",
            "FOW",
            "MudSnow",
        ] {
            assert!(
                names.contains(required),
                "missing runtime target spec: {required}"
            );
        }
    }

    #[test]
    fn phase4_runtime_target_formats_match_shader_bindings() {
        let r8 = PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .filter(|spec| spec.format == wgpu::TextureFormat::R8Unorm)
            .count();
        let rgba8 = PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .filter(|spec| spec.format == wgpu::TextureFormat::Rgba8Unorm)
            .count();
        assert_eq!(r8, 3);
        assert_eq!(rgba8, 3);
        assert!(PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .all(|spec| spec.bytes_per_pixel == 1 || spec.bytes_per_pixel == 4));
    }
}
