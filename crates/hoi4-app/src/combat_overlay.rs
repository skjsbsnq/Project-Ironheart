use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use crate::{App, GamePhase, WORLD_SCALE};

#[derive(Clone, Debug)]
pub(crate) struct CombatSideSnapshot {
    country: hoi4_state::CountryId,
    tag: String,
    name: String,
    province: u16,
    active_divisions: usize,
    reserve_divisions: usize,
    avg_org_pct: f32,
    avg_strength_pct: f32,
    combat_width: f32,
    soft_attack: f32,
    hard_attack: f32,
    defense: f32,
    breakthrough: f32,
    is_attacking: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct CombatBubbleSnapshot {
    id: u64,
    screen_pos: [f32; 2],
    chance_pct: u8,
    side_a: CombatSideSnapshot,
    side_b: CombatSideSnapshot,
    advantages: Vec<String>,
}

impl App {
    pub(crate) fn collect_combat_bubbles(&self) -> Vec<CombatBubbleSnapshot> {
        if self.view.game_phase != GamePhase::Playing {
            return Vec::new();
        }
        let Some(state) = self.state.as_ref() else {
            return Vec::new();
        };
        let dpi = state.ui_scale_factor();
        let screen_w = state.config.width as f32 / dpi.max(0.0001);
        let screen_h = state.config.height as f32 / dpi.max(0.0001);
        let view_proj = self.camera.view_proj();
        let player = hoi4_state::CountryId(self.view.player_country as u16);
        let mut seen = HashSet::new();
        let mut bubbles = Vec::new();

        for raw_a in self.world.prov_div_index.keys().copied() {
            if raw_a as usize >= self.world.provinces.count {
                continue;
            }
            let ctrl_a = self.world.provinces.controllers[raw_a as usize];
            if ctrl_a.is_none() || !is_land_province_for_bubble(&self.world, raw_a) {
                continue;
            }
            for &raw_b in self.world.map.neighbors(raw_a) {
                if raw_a >= raw_b || raw_b as usize >= self.world.provinces.count {
                    continue;
                }
                let key = (raw_a, raw_b);
                if !seen.insert(key) {
                    continue;
                }
                if !is_land_province_for_bubble(&self.world, raw_b) {
                    continue;
                }
                let ctrl_b = self.world.provinces.controllers[raw_b as usize];
                if ctrl_b.is_none()
                    || ctrl_b == ctrl_a
                    || !self.world.diplomacy.at_war_with(ctrl_a, ctrl_b)
                {
                    continue;
                }

                let a_active = self.active_combat_divisions(raw_a, ctrl_a);
                let b_active = self.active_combat_divisions(raw_b, ctrl_b);
                if a_active.is_empty() || b_active.is_empty() {
                    continue;
                }

                let a_attacks_b = self.province_has_attack_towards(raw_a, raw_b, ctrl_a);
                let b_attacks_a = self.province_has_attack_towards(raw_b, raw_a, ctrl_b);
                let side_a = match self.combat_side_snapshot(raw_a, ctrl_a, a_attacks_b) {
                    Some(side) => side,
                    None => continue,
                };
                let side_b = match self.combat_side_snapshot(raw_b, ctrl_b, b_attacks_a) {
                    Some(side) => side,
                    None => continue,
                };
                let Some(screen_pos) =
                    self.combat_contact_screen_pos(raw_a, raw_b, &view_proj, screen_w, screen_h)
                else {
                    continue;
                };
                if screen_pos[0] < -80.0
                    || screen_pos[0] > screen_w + 80.0
                    || screen_pos[1] < -80.0
                    || screen_pos[1] > screen_h + 80.0
                {
                    continue;
                }

                let power_a = combat_side_power(&side_a);
                let power_b = combat_side_power(&side_b);
                let focus_country = if side_b.country == player {
                    side_b.country
                } else {
                    side_a.country
                };
                let focus_power = if side_b.country == focus_country {
                    power_b
                } else {
                    power_a
                };
                let total_power = (power_a + power_b).max(1.0);
                let chance_pct = ((focus_power / total_power) * 100.0).clamp(1.0, 99.0) as u8;
                let id = stable_combat_bubble_id(raw_a, raw_b, focus_country);
                let advantages = combat_advantages(&side_a, &side_b, focus_country);

                bubbles.push(CombatBubbleSnapshot {
                    id,
                    screen_pos,
                    chance_pct,
                    side_a,
                    side_b,
                    advantages,
                });
            }
        }

        bubbles.sort_by_key(|b| b.id);
        bubbles.truncate(96);
        bubbles
    }

    pub(crate) fn active_combat_divisions(
        &self,
        province: u16,
        owner: hoi4_state::CountryId,
    ) -> Vec<usize> {
        self.world
            .divisions_in_province(hoi4_state::ProvinceId(province))
            .iter()
            .copied()
            .filter(|&di| {
                di < self.world.divisions.count
                    && self.world.divisions.owners[di] == owner
                    && self.world.divisions.in_combat[di]
            })
            .collect()
    }

    pub(crate) fn province_has_attack_towards(
        &self,
        source: u16,
        target: u16,
        owner: hoi4_state::CountryId,
    ) -> bool {
        self.world
            .divisions_in_province(hoi4_state::ProvinceId(source))
            .iter()
            .copied()
            .any(|di| {
                if di >= self.world.divisions.count
                    || self.world.divisions.owners[di] != owner
                    || !self.world.divisions.in_combat[di]
                {
                    return false;
                }
                let Some(dest) = self.world.divisions.destinations[di] else {
                    return false;
                };
                if dest.0 == target {
                    return true;
                }
                self.destination_points_through_target(source, target, dest.0)
            })
    }

    pub(crate) fn destination_points_through_target(
        &self,
        source: u16,
        target: u16,
        dest: u16,
    ) -> bool {
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let centroids = &state.unit_counter_centroids;
        let Some(&(sx, sy)) = centroids.get(source as usize) else {
            return false;
        };
        let Some(&(tx, ty)) = centroids.get(target as usize) else {
            return false;
        };
        let Some(&(dx, dy)) = centroids.get(dest as usize) else {
            return false;
        };
        if (sx + sy == 0.0) || (tx + ty == 0.0) || (dx + dy == 0.0) {
            return false;
        }
        let source_d = (sx - dx).hypot(sy - dy);
        let target_d = (tx - dx).hypot(ty - dy);
        target_d < source_d
    }

    pub(crate) fn combat_side_snapshot(
        &self,
        province: u16,
        owner: hoi4_state::CountryId,
        is_attacking: bool,
    ) -> Option<CombatSideSnapshot> {
        let mut active_divisions = 0usize;
        let mut reserve_divisions = 0usize;
        let mut org_sum = 0.0f32;
        let mut str_sum = 0.0f32;
        let mut combat_width = 0.0f32;
        let mut soft_attack = 0.0f32;
        let mut hard_attack = 0.0f32;
        let mut defense = 0.0f32;
        let mut breakthrough = 0.0f32;

        for &di in self
            .world
            .divisions_in_province(hoi4_state::ProvinceId(province))
        {
            if di >= self.world.divisions.count || self.world.divisions.owners[di] != owner {
                continue;
            }
            if self.world.divisions.strength[di] < 0.05 {
                continue;
            }
            let max_org = self.world.divisions.max_organisation[di].max(1e-6);
            let org_ratio = (self.world.divisions.organisation[di] / max_org).clamp(0.0, 1.0);
            if org_ratio < 0.05 {
                continue;
            }
            let is_active = self.world.divisions.in_combat[di];
            if is_active {
                active_divisions += 1;
            } else {
                reserve_divisions += 1;
            }
            let mut stats = self.division_stats_for_ui(di)?;
            if let Some(modifier) =
                hoi4_logic::military::general::modifier_for_division(&self.world, di)
            {
                stats.soft_attack *= modifier.attack_mult;
                stats.hard_attack *= modifier.attack_mult;
                stats.breakthrough *= modifier.attack_mult;
                stats.defense *= modifier.defense_mult;
            }
            let strength = self.world.divisions.strength[di].clamp(0.0, 1.0);
            let current_factor = strength * org_ratio;
            let display_factor = if is_active {
                current_factor
            } else {
                current_factor * 0.35
            };
            org_sum += org_ratio;
            str_sum += strength;
            combat_width += stats.combat_width;
            soft_attack += stats.soft_attack * display_factor;
            hard_attack += stats.hard_attack * display_factor;
            defense += stats.defense * display_factor;
            breakthrough += stats.breakthrough * display_factor;
        }

        if active_divisions == 0 {
            return None;
        }
        let counted = (active_divisions + reserve_divisions).max(1) as f32;
        let tag = self.world.country_tag(owner).unwrap_or("???").to_owned();
        Some(CombatSideSnapshot {
            country: owner,
            tag,
            name: self.country_display_name(owner),
            province,
            active_divisions,
            reserve_divisions,
            avg_org_pct: (org_sum / counted) * 100.0,
            avg_strength_pct: (str_sum / counted) * 100.0,
            combat_width,
            soft_attack,
            hard_attack,
            defense,
            breakthrough,
            is_attacking,
        })
    }

    pub(crate) fn division_stats_for_ui(
        &self,
        div_idx: usize,
    ) -> Option<hoi4_logic::military::stats::DivisionStats> {
        let owner = self.world.divisions.owners.get(div_idx).copied()?;
        let tag = self.world.country_tag(owner)?;
        let templates = self.world.data.division_templates.get(tag)?;
        let template_idx = *self.world.divisions.template_indices.get(div_idx)? as usize;
        let template = templates.get(template_idx)?;
        Some(hoi4_logic::military::stats::DivisionStats::aggregate(
            template,
            self.world.data.as_ref(),
        ))
    }

    pub(crate) fn combat_contact_screen_pos(
        &self,
        province_a: u16,
        province_b: u16,
        view_proj: &glam::Mat4,
        screen_w: f32,
        screen_h: f32,
    ) -> Option<[f32; 2]> {
        let state = self.state.as_ref()?;
        let (ax, ay) = *state.unit_counter_centroids.get(province_a as usize)?;
        let (bx, by) = *state.unit_counter_centroids.get(province_b as usize)?;
        if ax + ay == 0.0 || bx + by == 0.0 {
            return None;
        }
        let wx = ((ax + bx) * 0.5) * WORLD_SCALE;
        let wz = ((ay + by) * 0.5) * WORLD_SCALE;
        let clip = *view_proj * glam::Vec4::new(wx, 0.42, wz, 1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        Some([
            (ndc_x * 0.5 + 0.5) * screen_w,
            (1.0 - (ndc_y * 0.5 + 0.5)) * screen_h,
        ])
    }
}

pub(crate) fn is_land_province_for_bubble(world: &hoi4_state::World, raw_id: u16) -> bool {
    world
        .map
        .get_province(raw_id)
        .is_some_and(|def| matches!(def.province_type, hoi4_map::ProvinceType::Land))
}

pub(crate) fn stable_combat_bubble_id(a: u16, b: u16, focus: hoi4_state::CountryId) -> u64 {
    let mut h = DefaultHasher::new();
    a.hash(&mut h);
    b.hash(&mut h);
    focus.hash(&mut h);
    h.finish()
}

pub(crate) fn combat_side_power(side: &CombatSideSnapshot) -> f32 {
    let posture = if side.is_attacking {
        side.breakthrough * 0.35
    } else {
        side.defense * 0.35
    };
    (side.soft_attack + side.hard_attack * 0.75 + posture + side.active_divisions as f32 * 2.5)
        .max(0.1)
}

pub(crate) fn combat_advantages(
    side_a: &CombatSideSnapshot,
    side_b: &CombatSideSnapshot,
    focus_country: hoi4_state::CountryId,
) -> Vec<String> {
    let (own, enemy) = if side_b.country == focus_country {
        (side_b, side_a)
    } else {
        (side_a, side_b)
    };
    let mut advantages = Vec::new();
    let attack_ratio = own.soft_attack.max(1.0) / enemy.soft_attack.max(1.0);
    let org_delta = own.avg_org_pct - enemy.avg_org_pct;
    let strength_delta = own.avg_strength_pct - enemy.avg_strength_pct;
    let div_delta = own.active_divisions as i32 - enemy.active_divisions as i32;

    if attack_ratio >= 1.25 {
        advantages.push(format!("?????? +{:.0}%", (attack_ratio - 1.0) * 100.0));
    } else if attack_ratio <= 0.80 {
        advantages.push(format!("?????? {:.0}%", (attack_ratio - 1.0) * 100.0));
    }
    if org_delta.abs() >= 8.0 {
        advantages.push(format!("?????? {:+.0}%", org_delta));
    }
    if strength_delta.abs() >= 8.0 {
        advantages.push(format!("????????{:+.0}%", strength_delta));
    }
    if div_delta != 0 {
        advantages.push(format!("?????? {:+}", div_delta));
    }
    if advantages.is_empty() {
        advantages.push("Strategic advantage".to_owned());
    }
    advantages
}

pub(crate) fn combat_chance_color(chance: u8) -> hoi4_ui::egui::Color32 {
    if chance >= 65 {
        hoi4_ui::egui::Color32::from_rgb(0x44, 0xb8, 0x68)
    } else if chance >= 45 {
        hoi4_ui::egui::Color32::from_rgb(0xe0, 0xb8, 0x4c)
    } else {
        hoi4_ui::egui::Color32::from_rgb(0xc8, 0x4f, 0x46)
    }
}

pub(crate) fn show_combat_bubble_overlay(
    ctx: &hoi4_ui::egui::Context,
    bubbles: &[CombatBubbleSnapshot],
    selected: &mut Option<u64>,
) {
    use hoi4_ui::egui::{self, Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};

    if selected.is_some_and(|id| !bubbles.iter().any(|b| b.id == id)) {
        *selected = None;
    }

    let mut pointer_over_combat_ui = false;
    let mut clicked_combat_ui = false;

    let avail = ctx.available_rect();
    for bubble in bubbles {
        let chance = bubble.chance_pct;
        let color = combat_chance_color(chance);
        let center = egui::pos2(bubble.screen_pos[0], bubble.screen_pos[1]);
        if !avail.contains(center) {
            continue;
        }
        let id = egui::Id::new(("combat_bubble", bubble.id));
        let radius = 14.0;
        let size = Vec2::splat(radius * 2.0);
        let pos = egui::pos2(center.x - radius, center.y - radius);
        egui::Area::new(id)
            .order(egui::Order::Background)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                let painter = ui.painter();
                let selected_here = *selected == Some(bubble.id);
                let body = Color32::from_rgba_premultiplied(232, 208, 152, 235);
                let outline = if selected_here {
                    Color32::from_rgba_premultiplied(40, 30, 18, 255)
                } else {
                    Color32::from_rgba_premultiplied(70, 52, 30, 230)
                };
                painter.circle_filled(
                    rect.center() + egui::vec2(0.0, 1.5),
                    radius,
                    Color32::from_rgba_premultiplied(0, 0, 0, 80),
                );
                painter.circle_filled(rect.center(), radius, body);
                painter.circle_stroke(
                    rect.center(),
                    radius,
                    Stroke::new(if selected_here { 1.8 } else { 1.0 }, outline),
                );
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    chance.to_string(),
                    FontId::proportional(12.5),
                    color,
                );
                pointer_over_combat_ui |= response.hovered();
                if response.clicked() {
                    clicked_combat_ui = true;
                    *selected = if selected_here { None } else { Some(bubble.id) };
                }
                response.on_hover_text(format!(
                    "{} ({}) vs {} ({}) {}%",
                    bubble.side_a.tag,
                    bubble.side_a.active_divisions,
                    bubble.side_b.tag,
                    bubble.side_b.active_divisions,
                    chance
                ));
            });
    }

    let Some(selected_id) = *selected else {
        return;
    };
    let Some(bubble) = bubbles.iter().find(|b| b.id == selected_id) else {
        return;
    };
    let panel_pos = egui::pos2(
        (bubble.screen_pos[0] + 26.0).clamp(8.0, ctx.screen_rect().right() - 360.0),
        (bubble.screen_pos[1] - 28.0).clamp(52.0, ctx.screen_rect().bottom() - 360.0),
    );
    egui::Window::new("??????")
        .id(egui::Id::new(("combat_detail", bubble.id)))
        .order(egui::Order::Background)
        .fixed_pos(panel_pos)
        .default_width(340.0)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{}%", bubble.chance_pct))
                        .strong()
                        .color(combat_chance_color(bubble.chance_pct)),
                );
                ui.label(format!("{} vs {}", bubble.side_a.name, bubble.side_b.name));
            });
            ui.separator();
            combat_side_detail(ui, &bubble.side_a);
            ui.add_space(4.0);
            combat_side_detail(ui, &bubble.side_b);
            ui.separator();
            ui.label(RichText::new("???").strong());
            for item in &bubble.advantages {
                ui.label(item);
            }
        })
        .inspect(|inner| {
            pointer_over_combat_ui |= inner.response.hovered();
        });

    let clicked_elsewhere =
        ctx.input(|i| i.pointer.any_click()) && !pointer_over_combat_ui && !clicked_combat_ui;
    if clicked_elsewhere {
        *selected = None;
    }
}

pub(crate) fn combat_side_detail(ui: &mut hoi4_ui::egui::Ui, side: &CombatSideSnapshot) {
    use hoi4_ui::egui::{Color32, RichText};
    let posture = if side.is_attacking { "???" } else { "???" };
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} {}", side.tag, posture)).strong());
            ui.with_layout(
                hoi4_ui::egui::Layout::right_to_left(hoi4_ui::egui::Align::Center),
                |ui| {
                    ui.label(format!("??? {}", side.province));
                },
            );
        });
        ui.horizontal(|ui| {
            ui.label(format!(
                "??? {} / ??? {}",
                side.active_divisions, side.reserve_divisions
            ));
            ui.label(format!("??? {:.0}", side.combat_width));
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("??? {:.0}%", side.avg_org_pct)).color(
                    if side.avg_org_pct >= 50.0 {
                        Color32::from_rgb(0x73, 0xc5, 0x79)
                    } else {
                        Color32::from_rgb(0xe0, 0x64, 0x5f)
                    },
                ),
            );
            ui.label(format!("??? {:.0}%", side.avg_strength_pct));
        });
        ui.horizontal(|ui| {
            ui.label(format!("??? {:.0}", side.soft_attack));
            ui.label(format!("??? {:.0}", side.hard_attack));
        });
        ui.horizontal(|ui| {
            ui.label(format!("??? {:.0}", side.defense));
            ui.label(format!("??? {:.0}", side.breakthrough));
        });
    });
}
