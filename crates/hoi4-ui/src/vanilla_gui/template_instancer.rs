use super::ast::{GuiNode, GuiNodePath};
use super::binding::GuiBindingMap;
use super::layout::{
    compute_layout_tree_with_path, grid_slots, GuiPoint, GuiRect, LayoutNode, LayoutOptions,
};
use super::profile::{bind_profile_tree_with_path, VanillaPanelProfile};
use super::renderer::{RenderStats, VanillaGuiRenderer};
use crate::icons::IconBank;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemplateInstanceOptions {
    pub use_template_size: bool,
    pub zoom: f32,
    pub scroll_offset: GuiPoint,
    pub transform_origin: GuiPoint,
}

impl Default for TemplateInstanceOptions {
    fn default() -> Self {
        Self {
            use_template_size: false,
            zoom: 1.0,
            scroll_offset: GuiPoint::default(),
            transform_origin: GuiPoint::default(),
        }
    }
}

impl TemplateInstanceOptions {
    pub fn template_size(mut self, value: bool) -> Self {
        self.use_template_size = value;
        self
    }

    pub fn zoom(mut self, value: f32) -> Self {
        self.zoom = value.max(0.01);
        self
    }

    pub fn scroll_offset(mut self, value: GuiPoint) -> Self {
        self.scroll_offset = value;
        self
    }

    pub fn transform_origin(mut self, value: GuiPoint) -> Self {
        self.transform_origin = value;
        self
    }
}

#[derive(Debug, Clone)]
pub struct TemplateInstanceLayout {
    pub index: usize,
    pub path: GuiNodePath,
    pub rect: GuiRect,
    pub layout: LayoutNode,
}

pub struct TemplateInstancer<'a> {
    template_name: &'a str,
    template: &'a GuiNode,
}

impl<'a> TemplateInstancer<'a> {
    pub fn new(template_name: &'a str, template: &'a GuiNode) -> Self {
        Self {
            template_name,
            template,
        }
    }

    pub fn template_name(&self) -> &'a str {
        self.template_name
    }

    pub fn grid_instances(
        &self,
        parent_grid_node: &GuiNode,
        parent_grid_layout: &LayoutNode,
        count: usize,
    ) -> Vec<TemplateInstanceLayout> {
        self.grid_instances_with_options(
            parent_grid_node,
            parent_grid_layout,
            count,
            TemplateInstanceOptions::default(),
        )
    }

    pub fn grid_instances_with_options(
        &self,
        parent_grid_node: &GuiNode,
        parent_grid_layout: &LayoutNode,
        count: usize,
        options: TemplateInstanceOptions,
    ) -> Vec<TemplateInstanceLayout> {
        let slots = grid_slots(parent_grid_node, parent_grid_layout.rect, count);
        self.rect_instances_with_options(&parent_grid_layout.path, slots, options)
    }

    pub fn rect_instances(
        &self,
        parent_path: &GuiNodePath,
        rects: impl IntoIterator<Item = GuiRect>,
    ) -> Vec<TemplateInstanceLayout> {
        self.rect_instances_with_options(parent_path, rects, TemplateInstanceOptions::default())
    }

    pub fn rect_instances_with_options(
        &self,
        parent_path: &GuiNodePath,
        rects: impl IntoIterator<Item = GuiRect>,
        options: TemplateInstanceOptions,
    ) -> Vec<TemplateInstanceLayout> {
        rects
            .into_iter()
            .enumerate()
            .map(|(index, rect)| self.instance_layout(parent_path, index, rect, options))
            .collect()
    }

    pub fn bind_profile_instance_tree<P: VanillaPanelProfile>(
        &self,
        profile: &P,
        data: &P::Data,
        instance_path: GuiNodePath,
    ) -> GuiBindingMap {
        bind_profile_tree_with_path(profile, self.template, data, instance_path)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn paint_profile_grid_instances<P: VanillaPanelProfile>(
        &self,
        ui: &mut egui::Ui,
        renderer: &VanillaGuiRenderer<'_>,
        profile: &P,
        data: &P::Data,
        parent_grid_node: &GuiNode,
        parent_grid_layout: &LayoutNode,
        count: usize,
        icon_bank: &mut IconBank,
    ) -> RenderStats {
        self.paint_profile_grid_instances_with_options(
            ui,
            renderer,
            profile,
            data,
            parent_grid_node,
            parent_grid_layout,
            count,
            TemplateInstanceOptions::default(),
            icon_bank,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn paint_profile_grid_instances_with_options<P: VanillaPanelProfile>(
        &self,
        ui: &mut egui::Ui,
        renderer: &VanillaGuiRenderer<'_>,
        profile: &P,
        data: &P::Data,
        parent_grid_node: &GuiNode,
        parent_grid_layout: &LayoutNode,
        count: usize,
        options: TemplateInstanceOptions,
        icon_bank: &mut IconBank,
    ) -> RenderStats {
        let instances =
            self.grid_instances_with_options(parent_grid_node, parent_grid_layout, count, options);
        self.paint_profile_instances(ui, renderer, profile, data, instances, icon_bank)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn paint_profile_rect_instances<P: VanillaPanelProfile>(
        &self,
        ui: &mut egui::Ui,
        renderer: &VanillaGuiRenderer<'_>,
        profile: &P,
        data: &P::Data,
        parent_path: &GuiNodePath,
        rects: impl IntoIterator<Item = GuiRect>,
        options: TemplateInstanceOptions,
        icon_bank: &mut IconBank,
    ) -> RenderStats {
        let instances = self.rect_instances_with_options(parent_path, rects, options);
        self.paint_profile_instances(ui, renderer, profile, data, instances, icon_bank)
    }

    pub fn paint_instances_with_bindings(
        &self,
        ui: &mut egui::Ui,
        renderer: &VanillaGuiRenderer<'_>,
        instances: impl IntoIterator<Item = TemplateInstanceLayout>,
        mut bind_instance: impl FnMut(&TemplateInstanceLayout) -> GuiBindingMap,
        icon_bank: &mut IconBank,
    ) -> RenderStats {
        let mut total = RenderStats::default();
        for instance in instances {
            let bindings = bind_instance(&instance);
            total.merge(renderer.paint_tree(
                ui,
                self.template,
                &instance.layout,
                &bindings,
                icon_bank,
            ));
        }
        total
    }

    fn paint_profile_instances<P: VanillaPanelProfile>(
        &self,
        ui: &mut egui::Ui,
        renderer: &VanillaGuiRenderer<'_>,
        profile: &P,
        data: &P::Data,
        instances: impl IntoIterator<Item = TemplateInstanceLayout>,
        icon_bank: &mut IconBank,
    ) -> RenderStats {
        self.paint_instances_with_bindings(
            ui,
            renderer,
            instances,
            |instance| self.bind_profile_instance_tree(profile, data, instance.path.clone()),
            icon_bank,
        )
    }

    fn instance_layout(
        &self,
        parent_path: &GuiNodePath,
        index: usize,
        rect: GuiRect,
        options: TemplateInstanceOptions,
    ) -> TemplateInstanceLayout {
        let path = template_instance_path(parent_path, self.template_name, index);
        let layout_options = LayoutOptions::new(rect);
        let mut layout =
            compute_layout_tree_with_path(self.template, &layout_options, path.clone());
        if options.use_template_size
            && (layout.rect.width != rect.width || layout.rect.height != rect.height)
        {
            let template_rect = GuiRect::new(rect.x, rect.y, layout.rect.width, layout.rect.height);
            let layout_options = LayoutOptions::new(template_rect);
            layout = compute_layout_tree_with_path(self.template, &layout_options, path.clone());
        }
        if !options.use_template_size {
            layout.rect.width = rect.width;
            layout.rect.height = rect.height;
        }
        if needs_instance_transform(options) {
            transform_layout_tree(&mut layout, options);
        }
        let rect = layout.rect;
        TemplateInstanceLayout {
            index,
            path,
            rect,
            layout,
        }
    }
}

pub fn template_instance_path(
    parent_path: &GuiNodePath,
    template_name: &str,
    index: usize,
) -> GuiNodePath {
    parent_path.child(format!("{template_name}[{index}]"))
}

fn needs_instance_transform(options: TemplateInstanceOptions) -> bool {
    (options.zoom - 1.0).abs() > f32::EPSILON
        || options.scroll_offset.x.abs() > f32::EPSILON
        || options.scroll_offset.y.abs() > f32::EPSILON
}

fn transform_layout_tree(layout: &mut LayoutNode, options: TemplateInstanceOptions) {
    layout.rect = transform_rect(layout.rect, options);
    layout.clip_rect = transform_rect(layout.clip_rect, options);
    layout.scale *= options.zoom;
    for child in &mut layout.children {
        transform_layout_tree(child, options);
    }
}

fn transform_rect(rect: GuiRect, options: TemplateInstanceOptions) -> GuiRect {
    let origin = options.transform_origin;
    GuiRect::new(
        origin.x + (rect.x - origin.x) * options.zoom - options.scroll_offset.x,
        origin.y + (rect.y - origin.y) * options.zoom - options.scroll_offset.y,
        rect.width * options.zoom,
        rect.height * options.zoom,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vanilla_gui::{
        parse_gui_str, GuiAction, GuiActionKind, GuiBinding, GuiNodePath, GuiRect,
    };
    use egui::{Pos2, Sense, Vec2};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct RowProfile;

    impl VanillaPanelProfile for RowProfile {
        type Data = Vec<String>;
        type Command = String;

        fn root_template(&self) -> &'static str {
            "root"
        }

        fn required_gui_files(&self) -> &'static [&'static str] {
            &[]
        }

        fn bind_node(&self, node_path: &GuiNodePath, data: &Self::Data) -> GuiBinding {
            let path = node_path.to_string();
            if path.ends_with(".name") {
                let idx = instance_index_from_path(&path, "row").unwrap_or(0);
                return GuiBinding::default().text(data.get(idx).cloned().unwrap_or_default());
            }
            GuiBinding::default()
        }

        fn handle_action(&self, action: GuiAction, _: &Self::Data) -> Option<Self::Command> {
            (action.kind == GuiActionKind::Click).then(|| action.node_path.to_string())
        }
    }

    #[test]
    fn gate14_grid_instancer_generates_stable_paths_and_slot_rects() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width = 300 height = 120 }
        gridboxtype = {
            name = "rows"
            position = { x = 10 y = 20 }
            size = { width = 200 height = 90 }
            slotsize = { width = 80 height = 30 }
            max_slots = { x = 1 y = 3 }
            add_horizontal = no
        }
    }
    containerWindowType = {
        name = "row"
        size = { width = 80 height = 30 }
        instantTextboxType = { name = "name" text = "" }
    }
}
"#,
        );
        let root = doc.template_index().get("root").unwrap();
        let grid = root.find_node_by_name("rows").unwrap();
        let root_layout = crate::vanilla_gui::compute_layout_tree(
            root,
            &LayoutOptions::new(GuiRect::new(0.0, 0.0, 300.0, 120.0)),
        );
        let grid_layout = root_layout.find_by_name("rows").unwrap();
        let template = doc.template_index().get("row").unwrap();
        let instancer = TemplateInstancer::new("row", template);

        let instances = instancer.grid_instances(grid, grid_layout, 3);

        assert_eq!(instances.len(), 3);
        assert_eq!(instances[0].path.to_string(), "root.rows.row[0]");
        assert_eq!(instances[1].path.to_string(), "root.rows.row[1]");
        assert_eq!(instances[2].rect, GuiRect::new(10.0, 80.0, 80.0, 30.0));
        assert_eq!(
            instancer
                .bind_profile_instance_tree(
                    &RowProfile,
                    &vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
                    instances[1].path.clone(),
                )
                .for_node(&instances[1].path.child("name"), Some("name"))
                .text
                .as_deref(),
            Some("B")
        );
    }

    #[test]
    fn gate15_absolute_instances_support_template_size_zoom_and_scroll() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "national_focus_item"
        size = { width = 165 height = 128 }
        iconType = {
            name = "bg"
            position = { x = 5 y = 40 }
            size = { width = 90 height = 60 }
        }
        buttonType = {
            name = "symbol"
            position = { x = 5 y = -44 }
            size = { width = 70 height = 70 }
            Orientation = center
            centerposition = yes
        }
        instantTextboxType = {
            name = "name"
            position = { x = 15 y = 58 }
            maxWidth = 147
            maxHeight = 20
        }
    }
}
"#,
        );
        let template = doc.template_index().get("national_focus_item").unwrap();
        let parent_path = GuiNodePath::root("nationalfocusview")
            .child("tree")
            .child("grid");
        let instancer = TemplateInstancer::new("national_focus_item", template);

        let unscaled = instancer.rect_instances_with_options(
            &parent_path,
            [GuiRect::new(300.0, 200.0, 1.0, 1.0)],
            TemplateInstanceOptions::default().template_size(true),
        );
        assert_eq!(
            unscaled[0].path.to_string(),
            "nationalfocusview.tree.grid.national_focus_item[0]"
        );
        assert_eq!(
            unscaled[0].layout.rect,
            GuiRect::new(300.0, 200.0, 165.0, 128.0)
        );
        assert_eq!(
            unscaled[0].layout.clip_rect,
            GuiRect::new(300.0, 200.0, 165.0, 128.0)
        );
        assert_eq!(
            unscaled[0].layout.find_by_name("bg").unwrap().rect,
            GuiRect::new(305.0, 240.0, 90.0, 60.0)
        );
        assert_eq!(
            unscaled[0].layout.find_by_name("bg").unwrap().clip_rect,
            GuiRect::new(300.0, 200.0, 165.0, 128.0)
        );

        let scaled = instancer.rect_instances_with_options(
            &parent_path,
            [GuiRect::new(300.0, 200.0, 1.0, 1.0)],
            TemplateInstanceOptions::default()
                .template_size(true)
                .zoom(0.5)
                .scroll_offset(GuiPoint { x: 100.0, y: 40.0 }),
        );

        assert_eq!(scaled[0].layout.rect, GuiRect::new(50.0, 60.0, 82.5, 64.0));
        assert_eq!(
            scaled[0].layout.find_by_name("bg").unwrap().rect,
            GuiRect::new(52.5, 80.0, 45.0, 30.0)
        );

        let anchored = instancer.rect_instances_with_options(
            &parent_path,
            [GuiRect::new(300.0, 200.0, 1.0, 1.0)],
            TemplateInstanceOptions::default()
                .template_size(true)
                .zoom(0.5)
                .transform_origin(GuiPoint { x: 200.0, y: 100.0 })
                .scroll_offset(GuiPoint { x: 10.0, y: 20.0 }),
        );

        assert_eq!(
            anchored[0].layout.rect,
            GuiRect::new(240.0, 130.0, 82.5, 64.0)
        );
        assert_eq!(
            anchored[0].layout.find_by_name("bg").unwrap().rect,
            GuiRect::new(242.5, 150.0, 45.0, 30.0)
        );
    }

    #[test]
    fn gate17_decision_instance_click_routes_the_matching_dynamic_id() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width = 260 height = 80 }
        gridboxtype = {
            name = "decision_grid"
            size = { width = 240 height = 40 }
            slotsize = { width = 80 height = 40 }
            max_slots = { x = 3 y = 1 }
        }
    }
    containerWindowType = {
        name = "decision_item"
        size = { width = 80 height = 40 }
        buttonType = { name = "btn_select" size = { width = 80 height = 40 } }
    }
}
"#,
        );
        let root = doc.template_index().get("root").unwrap().clone();
        let grid = root.find_node_by_name("decision_grid").unwrap().clone();
        let template = doc.template_index().get("decision_item").unwrap().clone();
        let root_layout = crate::vanilla_gui::compute_layout_tree(
            &root,
            &LayoutOptions::new(GuiRect::new(0.0, 0.0, 260.0, 80.0)),
        );
        let grid_layout = root_layout.find_by_name("decision_grid").unwrap().clone();
        let commands = click_instance_buttons(
            template,
            "decision_item",
            grid,
            grid_layout,
            vec![
                "decision:activate:d1".to_owned(),
                "decision:activate:d2".to_owned(),
                "decision:activate:d3".to_owned(),
            ],
            Pos2::new(95.0, 20.0),
        );

        assert!(
            commands
                .iter()
                .any(|command| command == "decision:activate:d2"),
            "{commands:?}"
        );
        assert!(
            !commands
                .iter()
                .any(|command| command == "decision:activate:d1"),
            "{commands:?}"
        );
    }

    #[test]
    fn gate17_focus_rect_instance_click_routes_the_matching_dynamic_id() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "national_focus_item"
        size = { width = 80 height = 40 }
        buttonType = { name = "symbol" size = { width = 80 height = 40 } }
    }
    gridboxtype = {
        name = "focus_grid"
        size = { width = 260 height = 80 }
        slotsize = { width = 80 height = 40 }
        max_slots = { x = 3 y = 1 }
    }
}
"#,
        );
        let template = doc
            .template_index()
            .get("national_focus_item")
            .unwrap()
            .clone();
        let grid = doc.find_node_by_name("focus_grid").unwrap().clone();
        let grid_layout = LayoutNode {
            path: GuiNodePath::root("nationalfocusview")
                .child("tree")
                .child("grid"),
            name: Some("focus_grid".to_owned()),
            kind: grid.kind.clone(),
            rect: GuiRect::new(0.0, 0.0, 260.0, 80.0),
            clip_rect: GuiRect::new(0.0, 0.0, 260.0, 80.0),
            rects: crate::vanilla_gui::GuiControlRects::from_layout_rect(GuiRect::new(
                0.0, 0.0, 260.0, 80.0,
            )),
            intrinsic_size: None,
            visible: true,
            scale: 1.0,
            scroll: None,
            scroll_offset: GuiPoint::default(),
            children: Vec::new(),
            issues: Vec::new(),
        };
        let commands = click_instance_buttons(
            template,
            "national_focus_item",
            grid,
            grid_layout,
            vec![
                "focus:select:f1".to_owned(),
                "focus:select:f2".to_owned(),
                "focus:select:f3".to_owned(),
            ],
            Pos2::new(95.0, 20.0),
        );

        assert!(
            commands.iter().any(|command| command == "focus:select:f2"),
            "{commands:?}"
        );
        assert!(
            !commands.iter().any(|command| command == "focus:select:f1"),
            "{commands:?}"
        );
    }

    fn click_instance_buttons(
        template: GuiNode,
        template_name: &'static str,
        grid: GuiNode,
        grid_layout: LayoutNode,
        commands_by_idx: Vec<String>,
        click_pos: Pos2,
    ) -> Vec<String> {
        let clicked = Rc::new(RefCell::new(Vec::new()));
        let clicked_out = Rc::clone(&clicked);
        let command_count = commands_by_idx.len();
        let mut harness = egui_kittest::Harness::builder()
            .with_size(Vec2::new(260.0, 80.0))
            .build(move |ctx| {
                let path_cfg = hoi4_paths::PathConfig::with_game_path(std::env::temp_dir());
                let mut icon_bank = IconBank::new(ctx.clone(), path_cfg);
                let gfx_index =
                    crate::vanilla_gui::GfxIndex::from_files(Vec::<std::path::PathBuf>::new());
                let renderer = VanillaGuiRenderer::new(&gfx_index);
                let instancer = TemplateInstancer::new(template_name, &template);
                egui::Area::new(egui::Id::new(("gate17_instance_click", template_name)))
                    .fixed_pos(Pos2::ZERO)
                    .show(ctx, |ui| {
                        let _ = ui.allocate_exact_size(Vec2::new(260.0, 80.0), Sense::hover());
                        let instances =
                            instancer.grid_instances(&grid, &grid_layout, command_count);
                        let stats = instancer.paint_instances_with_bindings(
                            ui,
                            &renderer,
                            instances,
                            |instance| {
                                let mut bindings = GuiBindingMap::default();
                                bindings.insert_path(
                                    instance.path.child(if template_name == "decision_item" {
                                        "btn_select"
                                    } else {
                                        "symbol"
                                    }),
                                    GuiBinding::default()
                                        .click(commands_by_idx[instance.index].clone()),
                                );
                                bindings
                            },
                            &mut icon_bank,
                        );
                        clicked_out.borrow_mut().extend(stats.clicked_commands);
                    });
            });
        push_click(&mut harness, click_pos);
        harness.run_steps(2);
        let commands = clicked.borrow().clone();
        commands
    }

    fn push_click<State>(harness: &mut egui_kittest::Harness<'_, State>, pos: Pos2) {
        let modifiers = harness.input().modifiers;
        let input = harness.input_mut();
        input.events.push(egui::Event::PointerMoved(pos));
        input.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers,
        });
        input.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers,
        });
    }

    fn instance_index_from_path(path: &str, template_name: &str) -> Option<usize> {
        let marker = format!("{template_name}[");
        let start = path.find(&marker)? + marker.len();
        let rest = &path[start..];
        let end = rest.find(']')?;
        rest[..end].parse().ok()
    }
}
