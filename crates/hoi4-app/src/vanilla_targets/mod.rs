pub mod fow;
pub mod gradient_border;
pub mod point_lights;
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
    PointLightData,
    PointLightIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTargetSpec {
    pub name: &'static str,
    pub format: wgpu::TextureFormat,
    pub bytes_per_pixel: u32,
    pub semantic: RuntimeTargetChannelSemantic,
    pub dimensions: &'static str,
    pub source_detail: &'static str,
    pub source_trace: &'static str,
    pub parity_status: &'static str,
}

pub const PHASE4_RUNTIME_TARGET_SPECS: [RuntimeTargetSpec; 6] = [
    RuntimeTargetSpec {
        name: "GradientBorderChannel1",
        format: wgpu::TextureFormat::R8Unorm,
        bytes_per_pixel: 1,
        semantic: RuntimeTargetChannelSemantic::CountryBorderGradient,
        dimensions: "province-map pixels: world.map.province_map.width x height",
        source_detail: "CPU generated from the province controller SDF at map load, then regenerated when province control changes; shared by terrain, water, tree, and object passes",
        source_trace: "tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json and render_passes.json resource name GradientBorderChannel1",
        parity_status: "trace-backed binding and project-generated pixels; not copied from vanilla runtime memory",
    },
    RuntimeTargetSpec {
        name: "GradientBorderChannel2",
        format: wgpu::TextureFormat::R8Unorm,
        bytes_per_pixel: 1,
        semantic: RuntimeTargetChannelSemantic::ProvinceBorderGradient,
        dimensions: "province-map pixels: world.map.province_map.width x height",
        source_detail: "CPU generated from the static province topology SDF at map load; shared by terrain, water, tree, and object passes",
        source_trace: "tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json and render_passes.json resource name GradientBorderChannel2",
        parity_status: "trace-backed binding and project-generated pixels; not copied from vanilla runtime memory",
    },
    RuntimeTargetSpec {
        name: "GradientBorderChannel3",
        format: wgpu::TextureFormat::R8Unorm,
        bytes_per_pixel: 1,
        semantic: RuntimeTargetChannelSemantic::StateSeaImpassableGradient,
        dimensions: "province-map pixels: world.map.province_map.width x height",
        source_detail: "CPU generated from static state, coast, and impassable boundaries at map load; shared by terrain, water, tree, and object passes",
        source_trace: "tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json and render_passes.json resource name GradientBorderChannel3 in water",
        parity_status: "trace-backed water binding and project-generated pixels; terrain use is project extension",
    },
    RuntimeTargetSpec {
        name: "ProvinceSecondaryColorMap",
        format: wgpu::TextureFormat::Rgba8Unorm,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::ProvinceSecondaryColor,
        dimensions: "province-map pixels: world.map.province_map.width x height",
        source_detail: "CPU generated from occupation, battle-plan, naval dominance, selection, hover, and map-mode overlay state; regenerated when those frame inputs change",
        source_trace: "tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json and render_passes.json resource name ProvinceSecondaryColorMap",
        parity_status: "trace-backed binding and project-generated pixels; not copied from vanilla runtime memory",
    },
    RuntimeTargetSpec {
        name: "FOW",
        format: wgpu::TextureFormat::Rgba8Unorm,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::FogOfWar,
        dimensions: "province-map pixels: world.map.province_map.width x height",
        source_detail: "CPU generated default visibility map with all provinces explored and visible",
        source_trace: "tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json FOW/FOWNoise/FOWHeight family and project FOW binding",
        parity_status: "fallback: visible-all placeholder, not vanilla fog-of-war equivalent",
    },
    RuntimeTargetSpec {
        name: "MudSnow",
        format: wgpu::TextureFormat::Rgba8Unorm,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::MudSnow,
        dimensions: "heightmap/province-map pixels: world.map.heightmap.width x height; vanilla map dimensions match province map",
        source_detail: "CPU generated procedural mud/snow channels from heightmap elevation and season frame parameters",
        source_trace: "tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json and render_passes.json SnowMudData/SnowMudTexture resource names",
        parity_status: "fallback: procedural seasonal mask, not vanilla mud/snow runtime equivalent",
    },
];

pub const PHASE5_RUNTIME_TARGET_SPECS: [RuntimeTargetSpec; 2] = [
    RuntimeTargetSpec {
        name: "LightDataMap",
        format: wgpu::TextureFormat::Rgba32Float,
        bytes_per_pixel: 16,
        semantic: RuntimeTargetChannelSemantic::PointLightData,
        dimensions: "384x1 texels: MAX_POINT_LIGHTS(192) * 2 RGBA32F texels per light",
        source_detail: "CPU generated point-light records from victory points, ports, airbases, buildings, and combat state; regenerated when building/combat light sources change",
        source_trace: "tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json and render_passes.json resource name LightDataMap",
        parity_status: "trace-backed binding and project-generated light data; not copied from vanilla runtime memory",
    },
    RuntimeTargetSpec {
        name: "LightIndexMap",
        format: wgpu::TextureFormat::Rgba8Unorm,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::PointLightIndex,
        dimensions: "tile grid: ceil(province_map / 16px), width aligned to 64 texels; RGBA packs up to 4 light ids with 255 sentinel",
        source_detail: "CPU generated point-light tile index sharing LightDataMap light ids; regenerated with LightDataMap when light sources change",
        source_trace: "tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json and render_passes.json resource name LightIndexMap",
        parity_status: "trace-backed binding and project-derived 16px tile rule; not copied from vanilla runtime memory",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTargetPassBinding {
    pub binding: &'static str,
    pub target_name: &'static str,
    pub visual_impact: &'static str,
}

const TERRAIN_RUNTIME_TARGET_BINDINGS: [RuntimeTargetPassBinding; 8] = [
    RuntimeTargetPassBinding {
        binding: "light_data",
        target_name: "LightDataMap",
        visual_impact: "night lighting and local highlights use generated point light data",
    },
    RuntimeTargetPassBinding {
        binding: "light_index",
        target_name: "LightIndexMap",
        visual_impact: "point light lookup uses a generated tile index",
    },
    RuntimeTargetPassBinding {
        binding: "province_secondary_color",
        target_name: "ProvinceSecondaryColorMap",
        visual_impact: "occupation, selection, hover, and map-mode secondary tint use a runtime map target",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch1",
        target_name: "GradientBorderChannel1",
        visual_impact: "country border gradient is generated from runtime ownership state",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch2",
        target_name: "GradientBorderChannel2",
        visual_impact: "province border gradient is generated from runtime province topology",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch3",
        target_name: "GradientBorderChannel3",
        visual_impact: "state, coast, and impassable border gradient is generated at runtime",
    },
    RuntimeTargetPassBinding {
        binding: "fow",
        target_name: "FOW",
        visual_impact: "fog-of-war visibility is supplied as a runtime map target",
    },
    RuntimeTargetPassBinding {
        binding: "mud_snow",
        target_name: "MudSnow",
        visual_impact: "mud and snow masks are supplied as a runtime map target",
    },
];

const WATER_RUNTIME_TARGET_BINDINGS: [RuntimeTargetPassBinding; 8] = [
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch1",
        target_name: "GradientBorderChannel1",
        visual_impact: "country border gradient is shared with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch2",
        target_name: "GradientBorderChannel2",
        visual_impact: "province border gradient is shared with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch3",
        target_name: "GradientBorderChannel3",
        visual_impact: "semantic border gradient is shared with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "province_secondary_color",
        target_name: "ProvinceSecondaryColorMap",
        visual_impact: "water material receives selection and map-mode secondary tint",
    },
    RuntimeTargetPassBinding {
        binding: "fow",
        target_name: "FOW",
        visual_impact: "water material receives runtime fog-of-war visibility",
    },
    RuntimeTargetPassBinding {
        binding: "mud_snow",
        target_name: "MudSnow",
        visual_impact: "water has the shared snow/mud target available for SnowMudTexture parity work",
    },
    RuntimeTargetPassBinding {
        binding: "light_data",
        target_name: "LightDataMap",
        visual_impact: "water receives generated local night highlights",
    },
    RuntimeTargetPassBinding {
        binding: "light_index",
        target_name: "LightIndexMap",
        visual_impact: "water point light lookup uses the shared tile index",
    },
];

const TREE_RUNTIME_TARGET_BINDINGS: [RuntimeTargetPassBinding; 8] = [
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch1",
        target_name: "GradientBorderChannel1",
        visual_impact: "tree material shares country border gradient with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch2",
        target_name: "GradientBorderChannel2",
        visual_impact: "tree material shares province border gradient with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch3",
        target_name: "GradientBorderChannel3",
        visual_impact: "tree material shares semantic border gradient with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "province_secondary_color",
        target_name: "ProvinceSecondaryColorMap",
        visual_impact: "tree material receives selection and map-mode secondary tint",
    },
    RuntimeTargetPassBinding {
        binding: "fow",
        target_name: "FOW",
        visual_impact: "tree material receives runtime fog-of-war visibility",
    },
    RuntimeTargetPassBinding {
        binding: "mud_snow",
        target_name: "MudSnow",
        visual_impact: "tree material receives runtime snow masks",
    },
    RuntimeTargetPassBinding {
        binding: "light_data",
        target_name: "LightDataMap",
        visual_impact: "tree material receives generated local night highlights",
    },
    RuntimeTargetPassBinding {
        binding: "light_index",
        target_name: "LightIndexMap",
        visual_impact: "tree point light lookup uses the shared tile index",
    },
];

const PDXMESH_RUNTIME_TARGET_BINDINGS: [RuntimeTargetPassBinding; 8] = [
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch1",
        target_name: "GradientBorderChannel1",
        visual_impact: "mesh material shares country border gradient with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch2",
        target_name: "GradientBorderChannel2",
        visual_impact: "mesh material shares province border gradient with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch3",
        target_name: "GradientBorderChannel3",
        visual_impact: "mesh material shares semantic border gradient with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "province_secondary_color",
        target_name: "ProvinceSecondaryColorMap",
        visual_impact: "mesh material receives battle-plan, selection, and map-mode secondary tint",
    },
    RuntimeTargetPassBinding {
        binding: "fow",
        target_name: "FOW",
        visual_impact: "mesh material receives runtime fog-of-war visibility",
    },
    RuntimeTargetPassBinding {
        binding: "mud_snow",
        target_name: "MudSnow",
        visual_impact: "mesh material receives shared snow masks",
    },
    RuntimeTargetPassBinding {
        binding: "light_data",
        target_name: "LightDataMap",
        visual_impact: "mesh material receives generated local night highlights",
    },
    RuntimeTargetPassBinding {
        binding: "light_index",
        target_name: "LightIndexMap",
        visual_impact: "mesh point light lookup uses the shared tile index",
    },
];

pub fn runtime_target_spec(name: &str) -> Option<&'static RuntimeTargetSpec> {
    PHASE4_RUNTIME_TARGET_SPECS
        .iter()
        .chain(PHASE5_RUNTIME_TARGET_SPECS.iter())
        .find(|spec| spec.name == name)
}

pub fn runtime_target_bindings_for_pass(
    pass: &'static str,
) -> &'static [RuntimeTargetPassBinding] {
    match pass {
        "terrain" => &TERRAIN_RUNTIME_TARGET_BINDINGS,
        "water" => &WATER_RUNTIME_TARGET_BINDINGS,
        "tree" => &TREE_RUNTIME_TARGET_BINDINGS,
        "pdxmesh" | "object" => &PDXMESH_RUNTIME_TARGET_BINDINGS,
        _ => &[],
    }
}

pub fn runtime_target_audit_entries_for_pass(pass: &'static str) -> Vec<BindingAuditEntry> {
    runtime_target_bindings_for_pass(pass)
        .iter()
        .filter_map(|binding| runtime_target_audit_entry(pass, binding))
        .collect()
}

fn runtime_target_audit_entry(
    pass: &'static str,
    binding: &RuntimeTargetPassBinding,
) -> Option<BindingAuditEntry> {
    let spec = runtime_target_spec(binding.target_name)?;
    Some(
        BindingAuditEntry::dynamic_target(
            pass,
            binding.binding,
            spec.name,
            binding.visual_impact,
        )
        .with_runtime_target_metadata(
            spec.source_detail,
            format!("{:?}; {} bytes/pixel", spec.format, spec.bytes_per_pixel),
            spec.dimensions,
            spec.source_trace,
            spec.parity_status,
        ),
    )
}

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

    pub fn new_rgba32_float(
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
            wgpu::TextureFormat::Rgba32Float,
            16,
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

    pub fn memory_bytes(&self) -> u64 {
        self.width as u64 * self.height as u64 * self.bytes_per_pixel as u64
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
    pub light_data: RuntimeTargetTexture,
    pub light_index: RuntimeTargetTexture,
    country_border_signature: u64,
    province_secondary_signature: u64,
    mud_snow_signature: u64,
    point_light_signature: u64,
    province_secondary_data: Vec<u8>,
    mud_snow_data: Vec<u8>,
    point_light_data: point_lights::PointLightTargetData,
    world_scale: f32,
    height_scale: f32,
}

pub struct VanillaRuntimeTargetViews<'a> {
    pub gradient_border_ch1: &'a wgpu::TextureView,
    pub gradient_border_ch2: &'a wgpu::TextureView,
    pub gradient_border_ch3: &'a wgpu::TextureView,
    pub province_secondary_color: &'a wgpu::TextureView,
    pub fow: &'a wgpu::TextureView,
    pub mud_snow: &'a wgpu::TextureView,
    pub light_data: &'a wgpu::TextureView,
    pub light_index: &'a wgpu::TextureView,
}

#[derive(Clone, Copy)]
pub struct VanillaRuntimeTargetInputs<'a> {
    pub world: &'a World,
    pub country_sdf: &'a [u8],
    pub province_sdf: &'a [u8],
    pub coast_sdf: &'a [u8],
    pub world_scale: f32,
    pub height_scale: f32,
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
        let country_border_signature = gradient_border::country_signature(inputs.world);
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
        let point_light_data = point_lights::generate(inputs);
        let point_light_signature = point_lights::signature(inputs.world);

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
            light_data: RuntimeTargetTexture::new_rgba32_float(
                device,
                queue,
                "LightDataMap",
                point_light_data.light_data_width,
                1,
                &point_light_data.light_data,
            ),
            light_index: RuntimeTargetTexture::new_rgba8(
                device,
                queue,
                "LightIndexMap",
                point_light_data.light_index_width,
                point_light_data.light_index_height,
                &point_light_data.light_index,
            ),
            country_border_signature,
            province_secondary_signature,
            mud_snow_signature,
            point_light_signature,
            province_secondary_data,
            mud_snow_data,
            point_light_data,
            world_scale: inputs.world_scale,
            height_scale: inputs.height_scale,
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
            light_data: &self.light_data.view,
            light_index: &self.light_index.view,
        }
    }

    pub fn memory_bytes(&self) -> u64 {
        self.gradient_border.ch1.memory_bytes()
            + self.gradient_border.ch2.memory_bytes()
            + self.gradient_border.ch3.memory_bytes()
            + self.province_secondary_color.memory_bytes()
            + self.fow.memory_bytes()
            + self.mud_snow.memory_bytes()
            + self.light_data.memory_bytes()
            + self.light_index.memory_bytes()
    }

    pub fn cache_entry_count(&self) -> usize {
        // Three gradient targets plus secondary, FOW, mud/snow, light data,
        // and light index. The CPU-side generated buffers are retained and
        // rewritten only when their signatures change.
        8
    }

    pub fn update_frame(
        &mut self,
        queue: &wgpu::Queue,
        world: &World,
        params: &VanillaRuntimeTargetFrameParams,
    ) {
        let country_border_signature = gradient_border::country_signature(world);
        if country_border_signature != self.country_border_signature {
            let ch1 = gradient_border::generate_country_channel(world);
            self.gradient_border.ch1.write(queue, &ch1);
            self.country_border_signature = country_border_signature;
        }

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

        let point_light_signature = point_lights::signature(world);
        if point_light_signature != self.point_light_signature {
            self.point_light_data = point_lights::generate(VanillaRuntimeTargetInputs {
                world,
                country_sdf: &[],
                province_sdf: &[],
                coast_sdf: &[],
                world_scale: self.world_scale,
                height_scale: self.height_scale,
            });
            self.light_data
                .write(queue, &self.point_light_data.light_data);
            self.light_index
                .write(queue, &self.point_light_data.light_index);
            self.point_light_signature = point_light_signature;
        }
    }

    pub fn binding_audit_entries_for_pass(&self, pass: &'static str) -> Vec<BindingAuditEntry> {
        runtime_target_audit_entries_for_pass(pass)
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

    #[test]
    fn phase5_runtime_target_specs_cover_point_lights() {
        let names: HashSet<&'static str> = PHASE5_RUNTIME_TARGET_SPECS
            .iter()
            .map(|spec| spec.name)
            .collect();
        assert!(names.contains("LightDataMap"));
        assert!(names.contains("LightIndexMap"));
        assert!(PHASE5_RUNTIME_TARGET_SPECS.iter().any(|spec| {
            spec.semantic == RuntimeTargetChannelSemantic::PointLightData
                && spec.format == wgpu::TextureFormat::Rgba32Float
                && spec.bytes_per_pixel == 16
        }));
        assert!(PHASE5_RUNTIME_TARGET_SPECS.iter().any(|spec| {
            spec.semantic == RuntimeTargetChannelSemantic::PointLightIndex
                && spec.format == wgpu::TextureFormat::Rgba8Unorm
                && spec.bytes_per_pixel == 4
        }));
    }

    #[test]
    fn runtime_target_specs_have_stable_cache_shape() {
        assert_eq!(
            PHASE4_RUNTIME_TARGET_SPECS.len() + PHASE5_RUNTIME_TARGET_SPECS.len(),
            8
        );
        let bytes_per_pixel: u32 = PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .chain(PHASE5_RUNTIME_TARGET_SPECS.iter())
            .map(|spec| spec.bytes_per_pixel)
            .sum();
        assert_eq!(bytes_per_pixel, 35);
    }

    #[test]
    fn runtime_target_specs_are_traceable_and_explicit_about_parity() {
        for spec in PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .chain(PHASE5_RUNTIME_TARGET_SPECS.iter())
        {
            assert!(
                spec.source_trace.contains("tools/vanilla_trace/runtime_targets.json"),
                "{} missing runtime_targets.json provenance",
                spec.name
            );
            assert!(
                !spec.source_detail.is_empty()
                    && !spec.dimensions.is_empty()
                    && !spec.parity_status.is_empty(),
                "{} missing audit metadata",
                spec.name
            );
        }

        for fallback in ["FOW", "MudSnow"] {
            let spec = runtime_target_spec(fallback).unwrap();
            assert!(
                spec.parity_status.contains("fallback"),
                "{fallback} must explicitly declare fallback status"
            );
        }
    }

    #[test]
    fn shared_pass_bindings_cover_terrain_water_tree_and_objects() {
        for pass in ["terrain", "water", "tree", "pdxmesh"] {
            let entries = runtime_target_audit_entries_for_pass(pass);
            assert_eq!(entries.len(), 8, "{pass} runtime target coverage changed");
            for entry in entries {
                assert_eq!(entry.source_name, runtime_target_spec(&entry.source_name).unwrap().name);
                assert!(entry.resource_format.is_some(), "{pass}.{} missing format", entry.binding);
                assert!(
                    entry.resource_dimensions.is_some(),
                    "{pass}.{} missing dimensions",
                    entry.binding
                );
                assert!(
                    entry
                        .source_trace
                        .as_deref()
                        .is_some_and(|trace| trace.contains("runtime_targets.json")),
                    "{pass}.{} missing trace provenance",
                    entry.binding
                );
            }
        }
    }
}
