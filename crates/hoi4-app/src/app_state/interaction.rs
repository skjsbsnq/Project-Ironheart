use std::collections::HashSet;

use crate::map_interaction::{FrontlinePainterState, SelectionBoxState};

pub(crate) struct InteractionState {
    pub(crate) selected_divisions: Vec<usize>,
    pub(crate) selection_box: SelectionBoxState,
    pub(crate) selected_province_ids: HashSet<u32>,
    pub(crate) expanded_stacks: HashSet<u32>,
    pub(crate) counter_right_click_province: Option<u32>,
    pub(crate) pending_move_command: bool,
    pub(crate) selected_combat_bubble: Option<u64>,
    pub(crate) pending_naval_move_fleet: Option<u32>,
    pub(crate) naval_transfer_source_fleet: Option<u32>,
    pub(crate) pending_air_transfer_wing: Option<u32>,
    pub(crate) air_transfer_source_wing: Option<u32>,
    pub(crate) frontline_painter: FrontlinePainterState,
    pub(crate) selected_army_id: Option<hoi4_state::ArmyId>,
    pub(crate) template_editor_open: bool,
    pub(crate) selected_template_idx: Option<u16>,
    pub(crate) template_picker_target: Option<hoi4_ui::military::TemplatePickerTarget>,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            selected_divisions: Vec::new(),
            selection_box: SelectionBoxState::default(),
            selected_province_ids: HashSet::new(),
            expanded_stacks: HashSet::new(),
            counter_right_click_province: None,
            pending_move_command: false,
            selected_combat_bubble: None,
            pending_naval_move_fleet: None,
            naval_transfer_source_fleet: None,
            pending_air_transfer_wing: None,
            air_transfer_source_wing: None,
            frontline_painter: FrontlinePainterState::default(),
            selected_army_id: None,
            template_editor_open: false,
            selected_template_idx: None,
            template_picker_target: None,
        }
    }
}
