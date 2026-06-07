//! Event modal bridge.
//!
//! The scheduler and command contract stay in this legacy module so callers do
//! not need to change. The primary shell is rendered through the vanilla GUI
//! runtime, with the V9 event composite kept as a fallback when vanilla files
//! or sprite loading are unavailable.

use std::sync::OnceLock;

use egui::{Color32, Order, Sense};
use hoi4_content::{Event as ContentEvent, EventScheduler};

use crate::i18n::tr;
use crate::vanilla_gui::{
    bind_profile_tree, bind_profile_tree_with_path, compute_layout_tree_with_path, grid_slots,
    GfxIndex, GuiAction, GuiActionKind, GuiBinding, GuiDocument, GuiNodePath, GuiRect,
    LayoutOptions, VanillaGuiRenderer, VanillaPanelProfile, VanillaTemplateInstance,
};

#[derive(Debug, Clone, PartialEq)]
pub enum EventCommand {
    PickOption { event_id: String, option_idx: usize },
}

const EVENT_WINDOW_GUI_FILE: &str = "interface/eventwindow.gui";
const EVENT_WINDOW_PROFILE_ID: &str = "event_window";
const EVENT_WINDOW_ROOT: &str = "EventWindow";
const EVENT_OPTION_TEMPLATE: &str = "event_option_entry";

#[derive(Debug)]
struct VanillaEventGuiContext {
    document: GuiDocument,
    gfx_index: GfxIndex,
}

fn event_vanilla_gui_context() -> Option<&'static VanillaEventGuiContext> {
    static CACHE: OnceLock<Option<VanillaEventGuiContext>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let path_cfg = hoi4_paths::PathConfig::resolve(Default::default()).ok()?;
            let gui_path = path_cfg.find(EVENT_WINDOW_GUI_FILE)?;
            let document = crate::vanilla_gui::parse_gui_file(gui_path).ok()?;
            let gfx_index = GfxIndex::from_path_config(&path_cfg);
            Some(VanillaEventGuiContext {
                document,
                gfx_index,
            })
        })
        .as_ref()
}

struct EventWindowProfile;

impl VanillaPanelProfile for EventWindowProfile {
    type Data = EventWindowData;
    type Command = EventCommand;

    fn profile_id(&self) -> &'static str {
        EVENT_WINDOW_PROFILE_ID
    }

    fn root_template(&self) -> &'static str {
        EVENT_WINDOW_ROOT
    }

    fn required_gui_files(&self) -> &'static [&'static str] {
        &[EVENT_WINDOW_GUI_FILE]
    }

    fn template_instances(&self) -> &'static [VanillaTemplateInstance] {
        &[VanillaTemplateInstance {
            template_name: EVENT_OPTION_TEMPLATE,
            count: 1,
        }]
    }

    fn bind_node(&self, node_path: &GuiNodePath, data: &Self::Data) -> GuiBinding {
        let path = node_path.to_string();
        let name = node_path
            .0
            .last()
            .map(|label| event_path_label_name(label))
            .unwrap_or_default();

        if let Some(option_idx) = event_option_index_from_path(&path) {
            let Some(option) = data.options.get(option_idx) else {
                return GuiBinding::default().visible(false);
            };
            let mut binding = match name {
                "Name" => GuiBinding::default().text(&option.name),
                EVENT_OPTION_TEMPLATE | "event_option_background" => GuiBinding::default(),
                _ => GuiBinding::default(),
            };
            if !option.tooltip.is_empty() {
                binding = binding.tooltip(&option.tooltip);
            }
            if option.enabled
                && matches!(
                    name,
                    EVENT_OPTION_TEMPLATE | "event_option_background" | "Name"
                )
            {
                binding = binding.click(event_option_command(option_idx));
            }
            return binding;
        }

        match name {
            "Title" => {
                let mut binding = GuiBinding::default().text(&data.title);
                if data.queue_extra > 0 {
                    binding =
                        binding.tooltip(format!("+{} {}", data.queue_extra, tr("more_events")));
                }
                binding
            }
            "Description" => GuiBinding::default().text(&data.description),
            "event_picture" => GuiBinding::default().sprite(&data.picture),
            "options_grid" => GuiBinding::default().instances(data.options.len()),
            "btn_minimize" => GuiBinding::default().tooltip(tr("MINIMIZE_EVENT")),
            _ => GuiBinding::default(),
        }
    }

    fn handle_action(&self, action: GuiAction, data: &Self::Data) -> Option<Self::Command> {
        if action.kind != GuiActionKind::Click {
            return None;
        }
        let option_idx = event_option_index_from_path(&action.node_path.to_string())?;
        data.options
            .get(option_idx)
            .filter(|option| option.enabled)
            .map(|_| EventCommand::PickOption {
                event_id: data.event_id.clone(),
                option_idx,
            })
    }

    fn uses_slide_animation(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
struct EventWindowData {
    event_id: String,
    title: String,
    description: String,
    picture: String,
    queue_extra: usize,
    options: Vec<EventOptionBinding>,
}

impl EventWindowData {
    fn from_event(event: &ContentEvent, queue_extra: usize, option_enabled: &[bool]) -> Self {
        Self {
            event_id: event.id.clone(),
            title: localized_event_title(event),
            description: localized_event_description(event),
            picture: if event.picture.is_empty() {
                "GFX_report_event_001".to_owned()
            } else {
                event.picture.clone()
            },
            queue_extra,
            options: event
                .options
                .iter()
                .enumerate()
                .map(|(idx, option)| {
                    let name = tr(&option.name).to_owned();
                    let effect_preview = option
                        .effects
                        .iter()
                        .filter_map(|effect| effect.effect_summary())
                        .collect::<Vec<_>>()
                        .join(" | ");
                    EventOptionBinding {
                        name,
                        tooltip: effect_preview,
                        enabled: option_enabled.get(idx).copied().unwrap_or(false),
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct EventOptionBinding {
    name: String,
    tooltip: String,
    enabled: bool,
}

pub fn show_event_modal(
    ctx: &egui::Context,
    scheduler: &EventScheduler,
    mut option_trigger_satisfied: impl FnMut(&str, usize) -> bool,
) -> Option<EventCommand> {
    show_event_modal_inner(ctx, scheduler, None, |id, idx| {
        option_trigger_satisfied(id, idx)
    })
}

pub fn show_event_modal_with_icons(
    ctx: &egui::Context,
    scheduler: &EventScheduler,
    icon_bank: &mut crate::icons::IconBank,
    mut option_trigger_satisfied: impl FnMut(&str, usize) -> bool,
) -> Option<EventCommand> {
    show_event_modal_inner(ctx, scheduler, Some(icon_bank), |id, idx| {
        option_trigger_satisfied(id, idx)
    })
}

fn show_event_modal_inner(
    ctx: &egui::Context,
    scheduler: &EventScheduler,
    icon_bank: Option<&mut crate::icons::IconBank>,
    mut option_trigger_satisfied: impl FnMut(&str, usize) -> bool,
) -> Option<EventCommand> {
    let pending = scheduler.front()?;
    let event = scheduler.db.find(&pending.event_id)?;
    let queue_extra = scheduler.pending_len().saturating_sub(1);
    let option_enabled = (0..event.options.len())
        .map(|idx| option_trigger_satisfied(&event.id, idx))
        .collect::<Vec<_>>();

    match icon_bank {
        Some(icon_bank) => {
            if let Some(context) = event_vanilla_gui_context() {
                return show_vanilla_event_window(
                    ctx,
                    event,
                    queue_extra,
                    icon_bank,
                    &option_enabled,
                    context,
                );
            }
            crate::v9::composites::modal_event::show_event_modal(
                ctx,
                event,
                queue_extra,
                Some(icon_bank),
                |idx| option_enabled.get(idx).copied().unwrap_or(false),
            )
            .map(|option_idx| EventCommand::PickOption {
                event_id: event.id.clone(),
                option_idx,
            })
        }
        None => crate::v9::composites::modal_event::show_event_modal(
            ctx,
            event,
            queue_extra,
            None,
            |idx| option_enabled.get(idx).copied().unwrap_or(false),
        )
        .map(|option_idx| EventCommand::PickOption {
            event_id: event.id.clone(),
            option_idx,
        }),
    }
}

fn show_vanilla_event_window(
    ctx: &egui::Context,
    event: &ContentEvent,
    queue_extra: usize,
    icon_bank: &mut crate::icons::IconBank,
    option_enabled: &[bool],
    context: &VanillaEventGuiContext,
) -> Option<EventCommand> {
    let profile = EventWindowProfile;
    let data = EventWindowData::from_event(event, queue_extra, option_enabled);
    let root = context
        .document
        .template_index()
        .get(profile.root_template())?;
    let screen = ctx.screen_rect();
    let viewport = GuiRect::new(screen.left(), screen.top(), screen.width(), screen.height());
    let layout = compute_layout_tree_with_path(
        root,
        &LayoutOptions::new(viewport),
        GuiNodePath::root(EVENT_WINDOW_ROOT),
    );
    let bindings = bind_profile_tree(&profile, root, &data);

    icon_bank.add_search_dir("gfx/interface");
    icon_bank.add_search_dir("gfx/interface/events");

    let mut picked = None;
    egui::Area::new(egui::Id::new(("event_window_vanilla", &event.id)))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (backdrop, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(
                backdrop,
                egui::epaint::CornerRadius::ZERO,
                Color32::from_black_alpha(86),
            );
            let renderer = VanillaGuiRenderer::new(&context.gfx_index);
            let stats = renderer.paint_tree(ui, root, &layout, &bindings, icon_bank);
            for command in stats.clicked_commands {
                if let Some(command) = event_command_from_render_command(&profile, &data, &command)
                {
                    picked = Some(command);
                }
            }
            paint_event_option_instances(
                ui,
                &renderer,
                context,
                &profile,
                &data,
                root,
                &layout,
                icon_bank,
                &mut picked,
            );
        });

    picked
}

#[allow(clippy::too_many_arguments)]
fn paint_event_option_instances(
    ui: &mut egui::Ui,
    renderer: &VanillaGuiRenderer<'_>,
    context: &VanillaEventGuiContext,
    profile: &EventWindowProfile,
    data: &EventWindowData,
    root: &crate::vanilla_gui::GuiNode,
    root_layout: &crate::vanilla_gui::LayoutNode,
    icon_bank: &mut crate::icons::IconBank,
    picked: &mut Option<EventCommand>,
) {
    let Some(grid_node) = root.find_node_by_name("options_grid") else {
        return;
    };
    let Some(grid_layout) = root_layout.find_by_name("options_grid") else {
        return;
    };
    let Some(template) = context.document.template_index().get(EVENT_OPTION_TEMPLATE) else {
        return;
    };

    for (option_idx, slot) in grid_slots(grid_node, grid_layout.rect, data.options.len())
        .into_iter()
        .enumerate()
    {
        let path = event_option_instance_path(option_idx);
        let option_layout =
            compute_layout_tree_with_path(template, &LayoutOptions::new(slot), path.clone());
        let option_bindings = bind_profile_tree_with_path(profile, template, data, path);
        let stats = renderer.paint_tree(ui, template, &option_layout, &option_bindings, icon_bank);
        for command in stats.clicked_commands {
            if let Some(command) = event_command_from_render_command(profile, data, &command) {
                *picked = Some(command);
            }
        }
    }
}

fn event_command_from_render_command(
    profile: &EventWindowProfile,
    data: &EventWindowData,
    command: &str,
) -> Option<EventCommand> {
    let option_idx = command.strip_prefix("event_option:")?.parse().ok()?;
    profile.handle_action(
        GuiAction {
            node_path: event_option_instance_path(option_idx),
            kind: GuiActionKind::Click,
        },
        data,
    )
}

fn event_option_instance_path(option_idx: usize) -> GuiNodePath {
    GuiNodePath::root(EVENT_WINDOW_ROOT)
        .child("options_grid")
        .child(format!("{EVENT_OPTION_TEMPLATE}[{option_idx}]"))
}

fn event_option_command(option_idx: usize) -> String {
    format!("event_option:{option_idx}")
}

fn event_option_index_from_path(path: &str) -> Option<usize> {
    let marker = format!("{EVENT_OPTION_TEMPLATE}[");
    let start = path.find(&marker)? + marker.len();
    let rest = &path[start..];
    let end = rest.find(']')?;
    rest[..end].parse().ok()
}

fn event_path_label_name(label: &str) -> &str {
    label.split('[').next().unwrap_or(label)
}

fn localized_event_title(event: &ContentEvent) -> String {
    let translated = tr(&event.id);
    if translated == event.id {
        event.title.clone()
    } else {
        translated.to_owned()
    }
}

fn localized_event_description(event: &ContentEvent) -> String {
    let desc_key = format!("desc.{}", event.id);
    let translated = tr(&desc_key);
    if translated == desc_key {
        event.description.clone()
    } else {
        translated.to_owned()
    }
}

#[doc(hidden)]
pub fn _option_count(event: &ContentEvent) -> usize {
    event.options.len()
}

#[cfg(test)]
mod tests {
    use crate::vanilla_gui::VanillaPanelProfile;
    use hoi4_content::{Effect, Event, EventDb, EventOption, EventScheduler, Trigger};

    fn sample_db() -> EventDb {
        EventDb {
            events: vec![Event {
                id: "test.modal".into(),
                title: "Test".into(),
                description: "desc".into(),
                picture: String::new(),
                scope: hoi4_content::EventScope::Country,
                is_triggered_only: true,
                fire_only_once: true,
                hidden: false,
                trigger: Trigger::AlwaysTrue,
                mean_time_to_happen_days: 1,
                immediate: vec![],
                options: vec![
                    EventOption {
                        name: "A".into(),
                        trigger: Trigger::AlwaysTrue,
                        effects: vec![Effect::AddPoliticalPower(5.0)],
                        ai_chance: 1.0,
                    },
                    EventOption {
                        name: "B".into(),
                        trigger: Trigger::AlwaysFalse,
                        effects: vec![],
                        ai_chance: 0.5,
                    },
                ],
            }],
        }
    }

    #[test]
    fn front_when_pending_empty() {
        let s = EventScheduler::new(sample_db(), 1);
        assert!(s.front().is_none());
    }

    #[test]
    fn option_count_helper() {
        let db = sample_db();
        assert_eq!(super::_option_count(&db.events[0]), 2);
    }

    #[test]
    fn gate12_event_profile_declares_window_and_binds_dynamic_fields() {
        let db = sample_db();
        let event = &db.events[0];
        let profile = super::EventWindowProfile;
        let data = super::EventWindowData::from_event(event, 2, &[true, false]);

        assert_eq!(profile.profile_id(), super::EVENT_WINDOW_PROFILE_ID);
        assert_eq!(profile.root_template(), super::EVENT_WINDOW_ROOT);
        assert_eq!(
            profile.required_gui_files(),
            &[super::EVENT_WINDOW_GUI_FILE]
        );
        assert_eq!(
            profile.template_instances()[0].template_name,
            super::EVENT_OPTION_TEMPLATE
        );
        assert!(!profile.uses_slide_animation());

        let title = profile.bind_node(
            &super::GuiNodePath::root(super::EVENT_WINDOW_ROOT)
                .child("top_Window")
                .child("Title"),
            &data,
        );
        assert_eq!(title.text.as_deref(), Some("Test"));
        assert!(title.tooltip.as_deref().unwrap_or_default().contains("+2"));

        let description = profile.bind_node(
            &super::GuiNodePath::root(super::EVENT_WINDOW_ROOT)
                .child("midsection")
                .child("Description"),
            &data,
        );
        assert_eq!(description.text.as_deref(), Some("desc"));

        let grid = profile.bind_node(
            &super::GuiNodePath::root(super::EVENT_WINDOW_ROOT)
                .child("bottom_Window")
                .child("options_grid"),
            &data,
        );
        assert_eq!(grid.instance_count, Some(2));

        let option0_name =
            profile.bind_node(&super::event_option_instance_path(0).child("Name"), &data);
        assert_eq!(option0_name.text.as_deref(), Some("A"));
        assert_eq!(
            option0_name
                .click
                .as_ref()
                .map(|click| click.command.as_str()),
            Some("event_option:0")
        );

        let option1_name =
            profile.bind_node(&super::event_option_instance_path(1).child("Name"), &data);
        assert_eq!(option1_name.text.as_deref(), Some("B"));
        assert!(option1_name.click.is_none());

        let command = profile.handle_action(
            super::GuiAction {
                node_path: super::event_option_instance_path(0),
                kind: super::GuiActionKind::Click,
            },
            &data,
        );
        assert_eq!(
            command,
            Some(super::EventCommand::PickOption {
                event_id: "test.modal".to_owned(),
                option_idx: 0
            })
        );

        let disabled = profile.handle_action(
            super::GuiAction {
                node_path: super::event_option_instance_path(1),
                kind: super::GuiActionKind::Click,
            },
            &data,
        );
        assert!(disabled.is_none());
    }

    #[test]
    fn gate12_real_eventwindow_gui_reuses_runtime_when_vanilla_available() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        let Some(gui_path) = path_cfg.find(super::EVENT_WINDOW_GUI_FILE) else {
            return;
        };
        let doc = crate::vanilla_gui::parse_gui_file(gui_path).unwrap();
        let index = doc.template_index();

        for name in [
            super::EVENT_WINDOW_ROOT,
            "EventWindow_News",
            super::EVENT_OPTION_TEMPLATE,
        ] {
            assert!(index.get(name).is_some(), "missing template {name}");
        }

        let root = index.get(super::EVENT_WINDOW_ROOT).unwrap();
        assert_eq!(
            root.find_node_by_name("Title").map(|node| &node.kind),
            Some(&crate::vanilla_gui::GuiNodeKind::InstantTextbox)
        );
        assert_eq!(
            root.find_node_by_name("Description").map(|node| &node.kind),
            Some(&crate::vanilla_gui::GuiNodeKind::InstantTextbox)
        );
        assert_eq!(
            index
                .get(super::EVENT_OPTION_TEMPLATE)
                .and_then(|node| node.find_node_by_name("Name"))
                .map(|node| &node.kind),
            Some(&crate::vanilla_gui::GuiNodeKind::InstantTextbox)
        );

        let layout = crate::vanilla_gui::compute_layout_tree(
            root,
            &crate::vanilla_gui::LayoutOptions::new(crate::vanilla_gui::GuiRect::new(
                0.0, 0.0, 1920.0, 1080.0,
            )),
        );
        assert_eq!(layout.rect.x, 678.0);
        assert_eq!(layout.rect.y, 230.0);

        let refs = crate::vanilla_gui::collect_gfx_references(&doc);
        let gfx = crate::vanilla_gui::GfxIndex::from_path_config(&path_cfg);
        let report = gfx.hit_report(refs.iter().map(String::as_str));
        assert!(
            report.all_hit(),
            "missing event window resources: {:?}",
            report.missing
        );
        assert_eq!(
            gfx.get("GFX_event_report_tileable_midsection")
                .map(|resource| &resource.kind),
            Some(&crate::vanilla_gui::GfxResourceKind::CorneredTile)
        );
        assert_eq!(
            gfx.get("GFX_event_operative_background")
                .map(|resource| &resource.kind),
            Some(&crate::vanilla_gui::GfxResourceKind::FrameAnimated)
        );
    }
}
