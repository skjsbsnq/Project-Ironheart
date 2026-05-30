//! Province right-click context menu: move army / justify wargoal / declare war.
//! Province right-click context menu: move army / justify wargoal / declare war.
use crate::i18n::tr;

/// Command emitted by the province context menu.
#[derive(Debug, Clone, PartialEq)]
pub enum ProvinceMenuCommand {
    MoveArmyHere { province_id: u32 },
    JustifyWargoal { target_tag: String },
    DeclareWar { target_tag: String },
    Close,
}

/// Province context menu state.
pub struct ProvinceMenu {
    pub open: bool,
    pub province_id: u32,
    pub owner_tag: String,
    pub is_own_territory: bool,
    pub has_wargoal: bool,
    pub has_selected_divisions: bool,
}

impl ProvinceMenu {
    pub fn new() -> Self {
        Self {
            open: false,
            province_id: 0,
            owner_tag: String::new(),
            is_own_territory: false,
            has_wargoal: false,
            has_selected_divisions: false,
        }
    }

    fn show_v9(&mut self, ctx: &egui::Context) -> Option<ProvinceMenuCommand> {
        if !self.open {
            return None;
        }
        use crate::v9::{
            frame::{FrameStyle, PanelFrame},
            primitives::{Button, ButtonSize, ButtonVariant},
            tokens::{palette, spacing, Elevation, TextRole},
            AnchorLayout,
        };
        use egui::{Align2, Area, Order, Pos2, Rect, Sense, Vec2};

        let screen = ctx.screen_rect();
        let size = Vec2::new(300.0_f32.min(screen.width() * 0.82), 218.0);
        let placed = AnchorLayout::Center.place(screen, size, Vec2::ZERO);
        let mut cmd = None;
        let mut open = self.open;
        let owner_line = tr("province_owner")
            .replacen("{}", &self.province_id.to_string(), 1)
            .replacen("{}", &tr(&self.owner_tag), 1);

        Area::new(egui::Id::new("province_menu_v9"))
            .order(Order::Foreground)
            .fixed_pos(placed.min)
            .show(ctx, |ui| {
                let (outer, _) = ui.allocate_exact_size(size, Sense::click_and_drag());
                let frame = PanelFrame::new(FrameStyle::Panel, outer)
                    .with_accent(palette::BRASS_BRIGHT)
                    .with_elevation(Elevation::E3);
                frame.draw(ui.painter());
                let inner = frame.inner_rect();

                ui.painter().text(
                    inner.left_top(),
                    Align2::LEFT_TOP,
                    tr("province_actions"),
                    TextRole::Heading.font_id(),
                    palette::BRASS_BRIGHT,
                );
                let close_rect = Rect::from_min_size(
                    Pos2::new(inner.right() - 28.0, inner.top() - 2.0),
                    Vec2::new(28.0, 24.0),
                );
                if Button::new("X")
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .show_at(ui, close_rect)
                    .clicked()
                {
                    open = false;
                    cmd = Some(ProvinceMenuCommand::Close);
                }

                let mut y = inner.top() + spacing::S7;
                ui.painter().text(
                    Pos2::new(inner.left(), y),
                    Align2::LEFT_TOP,
                    owner_line,
                    TextRole::Body.font_id(),
                    palette::PARCHMENT,
                );
                y += 30.0;
                ui.painter().hline(
                    inner.left()..=inner.right(),
                    y - spacing::S2,
                    egui::Stroke::new(1.0, palette::HAIRLINE),
                );

                let button_size = Vec2::new(inner.width(), 30.0);
                if self.has_selected_divisions {
                    let rect = Rect::from_min_size(Pos2::new(inner.left(), y), button_size);
                    if Button::new(tr("move_army"))
                        .size(ButtonSize::Md)
                        .variant(ButtonVariant::Primary)
                        .show_at(ui, rect)
                        .clicked()
                    {
                        cmd = Some(ProvinceMenuCommand::MoveArmyHere {
                            province_id: self.province_id,
                        });
                        open = false;
                    }
                    y += button_size.y + spacing::S4;
                } else {
                    ui.painter().text(
                        Pos2::new(inner.left(), y + 7.0),
                        Align2::LEFT_TOP,
                        "No selected divisions",
                        TextRole::Caption.font_id(),
                        palette::MUTED,
                    );
                    y += button_size.y + spacing::S4;
                }

                if !self.is_own_territory {
                    let label = if self.has_wargoal {
                        format!("{} {}", tr("declare_war"), self.owner_tag)
                    } else {
                        format!("{} {}", tr("justify_wargoal"), self.owner_tag)
                    };
                    let rect = Rect::from_min_size(Pos2::new(inner.left(), y), button_size);
                    let variant = if self.has_wargoal {
                        ButtonVariant::Danger
                    } else {
                        ButtonVariant::Secondary
                    };
                    if Button::new(&label)
                        .size(ButtonSize::Md)
                        .variant(variant)
                        .show_at(ui, rect)
                        .clicked()
                    {
                        cmd = Some(if self.has_wargoal {
                            ProvinceMenuCommand::DeclareWar {
                                target_tag: self.owner_tag.clone(),
                            }
                        } else {
                            ProvinceMenuCommand::JustifyWargoal {
                                target_tag: self.owner_tag.clone(),
                            }
                        });
                        open = false;
                    }
                    y += button_size.y + spacing::S4;
                } else {
                    ui.painter().text(
                        Pos2::new(inner.left(), y + 7.0),
                        Align2::LEFT_TOP,
                        "Own territory",
                        TextRole::Caption.font_id(),
                        palette::MUTED,
                    );
                    y += button_size.y + spacing::S4;
                }

                let rect = Rect::from_min_size(Pos2::new(inner.left(), y), button_size);
                if Button::new(tr("close"))
                    .size(ButtonSize::Md)
                    .variant(ButtonVariant::Ghost)
                    .show_at(ui, rect)
                    .clicked()
                {
                    open = false;
                    cmd = Some(ProvinceMenuCommand::Close);
                }
            });

        self.open = open;
        cmd
    }

    #[allow(unreachable_code)]
    pub fn show(&mut self, ctx: &egui::Context) -> Option<ProvinceMenuCommand> {
        return self.show_v9(ctx);

        if !self.open {
            return None;
        }
        let mut cmd = None;
        let mut open = self.open;

        egui::Window::new(tr("province_actions"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.label(
                    tr("province_owner")
                        .replace("{}", &self.province_id.to_string())
                        .replacen("{}", &tr(&self.owner_tag), 1),
                );
                ui.separator();

                // Move army (always available if divisions selected)
                if self.has_selected_divisions {
                    if ui.button(format!("🚶 {}", tr("move_army"))).clicked() {
                        cmd = Some(ProvinceMenuCommand::MoveArmyHere {
                            province_id: self.province_id,
                        });
                        self.open = false;
                    }
                }

                // War actions (only for foreign provinces)
                if !self.is_own_territory {
                    if self.has_wargoal {
                        if ui
                            .button(format!("⚔ {} {}", tr("declare_war"), self.owner_tag))
                            .clicked()
                        {
                            cmd = Some(ProvinceMenuCommand::DeclareWar {
                                target_tag: self.owner_tag.clone(),
                            });
                            self.open = false;
                        }
                    } else {
                        if ui
                            .button(format!("📋 {} {}", tr("justify_wargoal"), self.owner_tag))
                            .clicked()
                        {
                            cmd = Some(ProvinceMenuCommand::JustifyWargoal {
                                target_tag: self.owner_tag.clone(),
                            });
                            self.open = false;
                        }
                    }
                }
            });
        self.open = open;
        cmd
    }
}
