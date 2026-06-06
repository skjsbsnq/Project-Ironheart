use crate::*;

impl App {
    pub(crate) fn update_menu_hover_from_cursor(&mut self, x: f32, y: f32) {
        let mut new_btn: Option<&'static str> = None;
        let mut new_row: Option<usize> = None;
        match self.game_phase {
            GamePhase::MainMenu => {
                for btn in &self.last_main_buttons {
                    if btn.enabled && btn.contains(x, y) {
                        new_btn = Some(btn.id);
                        break;
                    }
                }
            }
            GamePhase::CountrySelect => {
                if let Some(layout) = &self.last_country_layout {
                    if layout.start_button.contains(x, y) {
                        new_btn = Some(layout.start_button.id);
                    } else if layout.back_button.contains(x, y) {
                        new_btn = Some(layout.back_button.id);
                    }
                    for (rect, idx) in &layout.list_rows {
                        let (rx, ry, rw, rh) = *rect;
                        if x >= rx && x < rx + rw && y >= ry && y < ry + rh {
                            new_row = Some(*idx);
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
        if new_btn != self.menu_hovered_btn || new_row != self.menu_hovered_row {
            self.menu_hovered_btn = new_btn;
            self.menu_hovered_row = new_row;
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
        }
    }
}
