use std::time::Instant;

use crate::map_perf::estimate_frame_texture_memory_bytes;
use crate::map_renderer::MapFramePlan;
use crate::passes::PostProcessDebugView;
use crate::passes::{self, PostProcessLutSelection, PostProcessMode};
use crate::render_state::RenderState;
use hoi4_render::terrain::{vertex_count_for_lod, ChunkInstance};

pub(crate) struct MapDrawInput<'a> {
    pub(crate) frame_plan: &'a MapFramePlan,
    pub(crate) buckets: &'a [Vec<ChunkInstance>; 3],
    pub(crate) draw_3d_map: bool,
    pub(crate) show_province_names: bool,
    pub(crate) zoom_factor: f32,
    pub(crate) postprocess_debug_view: PostProcessDebugView,
    pub(crate) postprocess_chain_enabled: bool,
    pub(crate) postprocess_lut_selection: PostProcessLutSelection,
    pub(crate) output_view: &'a wgpu::TextureView,
}

pub(crate) struct MapDrawOutput {
    pub(crate) use_full_chain: bool,
    pub(crate) chain_label: &'static str,
}

pub(crate) fn render_map_frame(
    s: &mut RenderState,
    enc: &mut wgpu::CommandEncoder,
    input: MapDrawInput<'_>,
) -> MapDrawOutput {
    let map_draw = &input.frame_plan.draw;
    let world_objects = input.frame_plan.world_objects;
    let counts = [
        input.buckets[0].len() as u32,
        input.buckets[1].len() as u32,
        input.buckets[2].len() as u32,
    ];
    let vert_counts = [
        vertex_count_for_lod(0),
        vertex_count_for_lod(1),
        vertex_count_for_lod(2),
    ];

    if map_draw.shadow_caster {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_encoder_span(enc, "shadow_caster"));
        s.shadow_pass
            .render_caster(enc, &s.instance_buffers, counts, vertex_count_for_lod);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_encoder_span(enc, token);
        }
        s.pass_registry.record_cpu_ms(
            "shadow_caster",
            pass_started.elapsed().as_secs_f32() * 1000.0,
        );
        s.pass_registry.record_draw_calls(
            "shadow_caster",
            counts.iter().filter(|&&count| count > 0).count() as u32,
        );
    }

    if map_draw.projected_fow_shadow {
        let pass_started = Instant::now();
        // Phase E reserves vanilla orders 77-80 as an explicit fallback
        // producer. The target lifecycle is real; tree/projected,
        // terrainunlit/projected, and two-pass blur shaders remain degraded.
        s.pass_registry.record_cpu_ms(
            "projected_fow_shadow",
            pass_started.elapsed().as_secs_f32() * 1000.0,
        );
        s.pass_registry.record_draw_calls("projected_fow_shadow", 0);
    }

    {
        let pass_token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_encoder_span(enc, "3d_world"));
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("3d_to_hdr_pre_river"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &s.hdr_target.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.05,
                        g: 0.07,
                        b: 0.15,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &s.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        if input.draw_3d_map {
            render_3d_pre_river_passes(s, &mut pass, map_draw, &counts, &vert_counts);
        }
        drop(pass);

        if input.draw_3d_map {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("3d_river_to_hdr"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &s.hdr_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            render_river_pass(s, &mut pass, map_draw, &counts, &vert_counts);
            drop(pass);

            render_water_refraction_pass(s, enc, map_draw);

            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("3d_to_hdr_post_river"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &s.hdr_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &s.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            render_3d_post_river_passes(
                s,
                &mut pass,
                map_draw,
                world_objects,
                &counts,
                &vert_counts,
                input.show_province_names,
                input.zoom_factor,
            );
            drop(pass);
        }
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), pass_token) {
            profiler.end_encoder_span(enc, token);
        }
    }

    s.post_process.debug_view = input.postprocess_debug_view;
    let use_full_chain = map_draw.postprocess
        && s.post_process.mode == PostProcessMode::Full
        && input.postprocess_chain_enabled;
    let postprocess_started = Instant::now();
    if use_full_chain {
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_encoder_span(enc, "postprocess"));
        s.post_process
            .prepare(&s.queue, input.postprocess_lut_selection);
        s.post_process.render(enc, input.output_view);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_encoder_span(enc, token);
        }
        s.pass_registry.record_draw_calls("postprocess", 10);
    } else {
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_encoder_span(enc, "postprocess"));
        s.simple_blit.render(enc, input.output_view);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_encoder_span(enc, token);
        }
        s.pass_registry.record_draw_calls("postprocess", 1);
    }
    s.pass_registry.record_cpu_ms(
        "postprocess",
        postprocess_started.elapsed().as_secs_f32() * 1000.0,
    );
    record_map_pass_resources(s, use_full_chain);
    s.shadow_pass.render_debug(enc, input.output_view);

    MapDrawOutput {
        use_full_chain,
        chain_label: if use_full_chain {
            "post_process_full"
        } else {
            "simple_blit"
        },
    }
}

fn render_water_refraction_pass(
    s: &mut RenderState,
    enc: &mut wgpu::CommandEncoder,
    map_draw: &crate::map_renderer::MapPassDrawSet,
) {
    if !map_draw.water_refraction || !map_draw.water || !s.water_pass.any_loaded {
        return;
    }

    let pass_started = Instant::now();
    let token = s
        .gpu_profiler
        .as_mut()
        .and_then(|profiler| profiler.begin_encoder_span(enc, "water_refraction"));
    s.water_refraction_pass
        .render(&s.queue, enc, &s.water_refraction_target.view);
    if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
        profiler.end_encoder_span(enc, token);
    }
    s.pass_registry.record_cpu_ms(
        "water_refraction",
        pass_started.elapsed().as_secs_f32() * 1000.0,
    );
    s.pass_registry.record_draw_calls("water_refraction", 1);
}

fn render_3d_pre_river_passes<'pass>(
    s: &'pass mut RenderState,
    pass: &mut wgpu::RenderPass<'pass>,
    map_draw: &crate::map_renderer::MapPassDrawSet,
    counts: &[u32; 3],
    vert_counts: &[u32; 3],
) {
    if map_draw.sky {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_sky"));
        s.sky_pass.render(pass);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry
            .record_cpu_ms("3d_sky", pass_started.elapsed().as_secs_f32() * 1000.0);
        s.pass_registry.record_draw_calls("3d_sky", 1);
    }

    if map_draw.terrain {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_terrain"));
        s.terrain_pass
            .render(pass, &s.instance_buffers, counts, vert_counts);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry
            .record_cpu_ms("3d_terrain", pass_started.elapsed().as_secs_f32() * 1000.0);
        s.pass_registry.record_draw_calls(
            "3d_terrain",
            counts.iter().filter(|&&count| count > 0).count() as u32,
        );
    }

    if map_draw.border_first {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_border_first"));
        // P1 exposes vanilla order 82 as a distinct submit point. The current
        // strip BorderPass remains a non-vanilla fallback and is drawn at the
        // second border point until P4/P5 split the traced border families.
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry.record_cpu_ms(
            "3d_border_first",
            pass_started.elapsed().as_secs_f32() * 1000.0,
        );
        s.pass_registry.record_draw_calls("3d_border_first", 0);
    }
}

fn render_river_pass<'pass>(
    s: &'pass mut RenderState,
    pass: &mut wgpu::RenderPass<'pass>,
    map_draw: &crate::map_renderer::MapPassDrawSet,
    counts: &[u32; 3],
    vert_counts: &[u32; 3],
) {
    if map_draw.river {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_river"));
        s.river_pass
            .render(pass, &s.instance_buffers, counts, vert_counts);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry
            .record_cpu_ms("3d_river", pass_started.elapsed().as_secs_f32() * 1000.0);
        s.pass_registry.record_draw_calls(
            "3d_river",
            counts.iter().filter(|&&count| count > 0).count() as u32,
        );
    }
}

fn render_3d_post_river_passes<'pass>(
    s: &'pass mut RenderState,
    pass: &mut wgpu::RenderPass<'pass>,
    map_draw: &crate::map_renderer::MapPassDrawSet,
    world_objects: crate::map_renderer::WorldObjectPlan,
    counts: &[u32; 3],
    vert_counts: &[u32; 3],
    show_province_names: bool,
    zoom_factor: f32,
) {
    if map_draw.map_layers {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_map_layers"));
        // Vanilla orders 84-85 are additional terrain/map-layer submissions.
        // P1 keeps the lifecycle slot explicit; P3/P4 will attach traced
        // producers/consumers instead of folding these into terrain fallback.
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry.record_cpu_ms(
            "3d_map_layers",
            pass_started.elapsed().as_secs_f32() * 1000.0,
        );
        s.pass_registry.record_draw_calls("3d_map_layers", 0);
    }

    if map_draw.water {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_water"));
        s.water_pass
            .render(pass, &s.instance_buffers, counts, vert_counts);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry
            .record_cpu_ms("3d_water", pass_started.elapsed().as_secs_f32() * 1000.0);
        s.pass_registry.record_draw_calls(
            "3d_water",
            counts.iter().filter(|&&count| count > 0).count() as u32,
        );
    }

    if map_draw.border_second {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_border_second"));
        s.border_pass.render(pass);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry.record_cpu_ms(
            "3d_border_second",
            pass_started.elapsed().as_secs_f32() * 1000.0,
        );
        s.pass_registry.record_draw_calls("3d_border_second", 6);
    }

    if map_draw.trade_routes {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_traderoute"));
        s.traderoute_pass.render(pass);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry.record_cpu_ms(
            "3d_traderoute",
            pass_started.elapsed().as_secs_f32() * 1000.0,
        );
        s.pass_registry.record_draw_calls("3d_traderoute", 1);
    }
    if map_draw.straits {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_strait"));
        s.strait_pass.render(pass);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry
            .record_cpu_ms("3d_strait", pass_started.elapsed().as_secs_f32() * 1000.0);
        s.pass_registry.record_draw_calls("3d_strait", 1);
    }

    if map_draw.railways && s.railways_vertex_count > 0 {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_railways"));
        pass.set_pipeline(&s.railways_pipeline);
        pass.set_bind_group(0, &s.railways_bind_group, &[]);
        pass.set_vertex_buffer(0, s.railways_buffer.slice(..));
        pass.draw(0..s.railways_vertex_count, 0..1);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry
            .record_cpu_ms("3d_railways", pass_started.elapsed().as_secs_f32() * 1000.0);
        s.pass_registry.record_draw_calls("3d_railways", 1);
    }

    if map_draw.hoi3_counters {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "hoi3_counter_v3"));
        s.hoi3_counter_pass.render(pass);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry.record_cpu_ms(
            "hoi3_counter_v3",
            pass_started.elapsed().as_secs_f32() * 1000.0,
        );
        s.pass_registry.record_draw_calls("hoi3_counter_v3", 1);
    }

    if map_draw.trees {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_trees"));
        let mut draw_calls = 0u32;
        if let Some(tf) = s.tree_full_pass.as_mut() {
            tf.render(pass);
            draw_calls = 1;
        } else {
            let any_mesh_loaded = s.trees_mesh_instance_counts.iter().any(|&c| c > 0);
            let all_mesh_loaded = !s.trees_mesh_instance_counts.is_empty()
                && s.trees_mesh_instance_counts.iter().all(|&c| c > 0);
            if s.trees_count > 0 && !all_mesh_loaded {
                pass.set_pipeline(&s.trees_pipeline);
                pass.set_bind_group(0, &s.trees_bind_group, &[]);
                pass.set_vertex_buffer(0, s.trees_buffer.slice(..));
                pass.draw(0..6, 0..s.trees_count);
                draw_calls = draw_calls.saturating_add(1);
            }
            if any_mesh_loaded {
                pass.set_pipeline(&s.trees_mesh_pipeline);
                for ty in 0..s.trees_mesh_index_counts.len() {
                    let inst_count = s.trees_mesh_instance_counts[ty];
                    let idx_count = s.trees_mesh_index_counts[ty];
                    if inst_count == 0 || idx_count == 0 {
                        continue;
                    }
                    pass.set_bind_group(0, &s.trees_mesh_bind_groups[ty], &[]);
                    pass.set_vertex_buffer(0, s.trees_mesh_vertex_buffers[ty].slice(..));
                    pass.set_vertex_buffer(1, s.trees_mesh_instance_buffers[ty].slice(..));
                    pass.set_index_buffer(
                        s.trees_mesh_index_buffers[ty].slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    pass.draw_indexed(0..idx_count, 0, 0..inst_count);
                    draw_calls = draw_calls.saturating_add(1);
                }
            }
        }
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry
            .record_cpu_ms("3d_trees", pass_started.elapsed().as_secs_f32() * 1000.0);
        s.pass_registry
            .record_draw_calls("3d_trees", draw_calls.max(1));
    }

    if map_draw.buildings && s.pdxmesh_pass.any_loaded {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_buildings"));
        s.pdxmesh_pass.render(pass);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry.record_cpu_ms(
            "3d_buildings",
            pass_started.elapsed().as_secs_f32() * 1000.0,
        );
        s.pass_registry
            .record_draw_calls("3d_buildings", s.pdxmesh_pass.loaded_draw_count().max(1));
    } else if map_draw.buildings && s.buildings_count > 0 {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_buildings"));
        pass.set_pipeline(&s.buildings_pipeline);
        pass.set_bind_group(0, &s.buildings_bind_group, &[]);
        pass.set_vertex_buffer(0, s.buildings_buffer.slice(..));
        pass.draw(0..6, 0..s.buildings_count);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry.record_cpu_ms(
            "3d_buildings",
            pass_started.elapsed().as_secs_f32() * 1000.0,
        );
        s.pass_registry.record_draw_calls("3d_buildings", 1);
    }

    if map_draw.poi_icons {
        if let Some(poi) = s.poi_icon_pass.as_ref() {
            let pass_started = Instant::now();
            let token = s
                .gpu_profiler
                .as_mut()
                .and_then(|profiler| profiler.begin_render_span(pass, "3d_poi_icons"));
            poi.render(pass);
            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                profiler.end_render_span(pass, token);
            }
            s.pass_registry.record_cpu_ms(
                "3d_poi_icons",
                pass_started.elapsed().as_secs_f32() * 1000.0,
            );
            s.pass_registry.record_draw_calls("3d_poi_icons", 1);
        }
    }

    if map_draw.map_names {
        if let Some(mnp) = s.mapname_pass.as_ref() {
            let pass_started = Instant::now();
            let token = s
                .gpu_profiler
                .as_mut()
                .and_then(|profiler| profiler.begin_render_span(pass, "3d_mapname"));
            mnp.render(pass, world_objects.country_names.scale);
            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                profiler.end_render_span(pass, token);
            }
            s.pass_registry
                .record_cpu_ms("3d_mapname", pass_started.elapsed().as_secs_f32() * 1000.0);
            s.pass_registry.record_draw_calls("3d_mapname", 1);
        }
    }

    if map_draw.province_names && show_province_names {
        if let Some(pnp) = s.province_name_pass.as_ref() {
            let pass_started = Instant::now();
            let token = s
                .gpu_profiler
                .as_mut()
                .and_then(|profiler| profiler.begin_render_span(pass, "3d_province_name"));
            pnp.render(pass, zoom_factor);
            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                profiler.end_render_span(pass, token);
            }
            s.pass_registry.record_cpu_ms(
                "3d_province_name",
                pass_started.elapsed().as_secs_f32() * 1000.0,
            );
            s.pass_registry.record_draw_calls("3d_province_name", 1);
        }
    }

    if map_draw.map_arrows {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_maparrow"));
        s.maparrow_pass.render(pass);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry
            .record_cpu_ms("3d_maparrow", pass_started.elapsed().as_secs_f32() * 1000.0);
        s.pass_registry.record_draw_calls("3d_maparrow", 1);
    }
    if map_draw.frontlines && s.frontlines_vertex_count > 0 {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_frontlines"));
        pass.set_pipeline(&s.frontlines_pipeline);
        pass.set_bind_group(0, &s.frontlines_bind_group, &[]);
        pass.set_vertex_buffer(0, s.frontlines_buffer.slice(..));
        pass.draw(0..s.frontlines_vertex_count, 0..1);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry.record_cpu_ms(
            "3d_frontlines",
            pass_started.elapsed().as_secs_f32() * 1000.0,
        );
        s.pass_registry.record_draw_calls("3d_frontlines", 1);
    }

    if map_draw.particles {
        let pass_started = Instant::now();
        let token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_render_span(pass, "3d_particles"));
        s.particle_pass.render(pass);
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
            profiler.end_render_span(pass, token);
        }
        s.pass_registry.record_cpu_ms(
            "3d_particles",
            pass_started.elapsed().as_secs_f32() * 1000.0,
        );
        s.pass_registry.record_draw_calls("3d_particles", 1);
    }
}

fn record_map_pass_resources(s: &mut RenderState, postprocess_full: bool) {
    let runtime_target_bytes = s.vanilla_targets.memory_bytes();
    let postprocess_bytes =
        estimate_frame_texture_memory_bytes(s.config.width, s.config.height, postprocess_full);

    s.pass_registry.record_texture_memory_bytes(
        "shadow_caster",
        passes::SHADOW_MAP_SIZE as u64 * passes::SHADOW_MAP_SIZE as u64 * 4,
    );
    s.pass_registry.record_texture_memory_bytes(
        "projected_fow_shadow",
        s.vanilla_targets.projected_shadow_fow.memory_bytes()
            + s.vanilla_targets
                .projected_shadow_fow_blur_temp
                .memory_bytes(),
    );
    s.pass_registry
        .record_fallback_count("projected_fow_shadow", 1);
    s.pass_registry
        .record_fallback_count("3d_sky", if s.sky_pass.loaded { 0 } else { 1 });
    s.pass_registry.record_texture_memory_bytes(
        "3d_terrain",
        runtime_target_bytes + s.config.width as u64 * s.config.height as u64 * 12,
    );
    s.pass_registry.record_fallback_count(
        "3d_terrain",
        s.terrain_pass.binding_audit.fallback_count() as u32,
    );
    s.pass_registry
        .record_texture_memory_bytes("3d_water", runtime_target_bytes);
    s.pass_registry
        .record_texture_memory_bytes("water_refraction", s.water_refraction_target.memory_bytes());
    s.pass_registry.record_fallback_count(
        "3d_water",
        s.water_pass.binding_audit.fallback_count() as u32,
    );
    s.pass_registry
        .record_texture_memory_bytes("3d_river", runtime_target_bytes / 8);
    s.pass_registry.record_fallback_count(
        "3d_river",
        s.river_pass.binding_audit.fallback_count() as u32,
    );
    s.pass_registry.record_fallback_count("3d_map_layers", 1);
    s.pass_registry.record_fallback_count("3d_border_first", 1);
    s.pass_registry.record_fallback_count(
        "3d_border_second",
        if s.border_pass.any_loaded { 0 } else { 1 },
    );
    if let Some(tree_full) = s.tree_full_pass.as_ref() {
        s.pass_registry
            .record_fallback_count("3d_trees", tree_full.binding_audit.fallback_count() as u32);
    } else {
        s.pass_registry.record_fallback_count("3d_trees", 1);
    }
    s.pass_registry.record_fallback_count(
        "3d_buildings",
        if s.pdxmesh_pass.any_loaded { 0 } else { 1 },
    );
    s.pass_registry
        .record_texture_memory_bytes("postprocess", postprocess_bytes);
    s.pass_registry.record_fallback_count(
        "postprocess",
        if s.post_process.color_cube_fallback {
            1
        } else {
            0
        },
    );
    s.pass_registry.record_fallback_count("ui", 0);
}
