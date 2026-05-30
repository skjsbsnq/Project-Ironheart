//! V9 data-table helpers kept for legacy callers that only need key/value rows.

use egui::Color32;

use crate::v9::tokens::{palette, TextRole};

pub use crate::v9::primitives::{DataTable, TableAlign, TableCell, TableColumn, TableRow};

pub fn header(ui: &mut egui::Ui, text: &str) {
    ui.colored_label(palette::GOLD, egui::RichText::new(text).strong());
}

pub fn key_value(ui: &mut egui::Ui, key: &str, value: impl ToString) {
    ui.colored_label(
        palette::MUTED,
        egui::RichText::new(key).font(TextRole::Caption.font_id()),
    );
    ui.colored_label(palette::PARCHMENT, value.to_string());
    ui.end_row();
}

pub fn signed_money(ui: &mut egui::Ui, value: f64, suffix: &str) {
    let color = if value >= 0.0 {
        palette::GOOD
    } else {
        palette::BAD
    };
    ui.colored_label(color, format!("{:+.1}{}", value, suffix));
}

pub fn signed_color(value: f64) -> Color32 {
    if value >= 0.0 {
        palette::GOOD
    } else {
        palette::BAD
    }
}
