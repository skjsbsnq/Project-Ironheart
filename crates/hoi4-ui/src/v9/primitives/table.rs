//! V9 data table primitive.

use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::v9::{
    paint, profiler, sound,
    text::fit_font_to_width,
    tokens::{palette, spacing, TextRole},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlign {
    Left,
    Center,
    Right,
}

impl TableAlign {
    fn anchor(self) -> Align2 {
        match self {
            Self::Left => Align2::LEFT_CENTER,
            Self::Center => Align2::CENTER_CENTER,
            Self::Right => Align2::RIGHT_CENTER,
        }
    }

    fn x(self, rect: Rect) -> f32 {
        match self {
            Self::Left => rect.left() + spacing::S3,
            Self::Center => rect.center().x,
            Self::Right => rect.right() - spacing::S3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TableColumn<'a> {
    pub label: &'a str,
    pub weight: f32,
    pub align: TableAlign,
}

impl<'a> TableColumn<'a> {
    pub fn new(label: &'a str, weight: f32) -> Self {
        Self {
            label,
            weight,
            align: TableAlign::Left,
        }
    }

    pub fn right(mut self) -> Self {
        self.align = TableAlign::Right;
        self
    }

    pub fn center(mut self) -> Self {
        self.align = TableAlign::Center;
        self
    }
}

#[derive(Debug, Clone)]
pub struct TableCell {
    pub text: String,
    pub color: Color32,
    pub align: Option<TableAlign>,
}

impl TableCell {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: palette::PARCHMENT_DIM,
            align: None,
        }
    }

    pub fn strong(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: palette::PARCHMENT,
            align: None,
        }
    }

    pub fn colored(text: impl Into<String>, color: Color32) -> Self {
        Self {
            text: text.into(),
            color,
            align: None,
        }
    }

    pub fn right(mut self) -> Self {
        self.align = Some(TableAlign::Right);
        self
    }

    pub fn center(mut self) -> Self {
        self.align = Some(TableAlign::Center);
        self
    }
}

#[derive(Debug, Clone)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub accent: Option<Color32>,
}

impl TableRow {
    pub fn new(cells: Vec<TableCell>) -> Self {
        Self {
            cells,
            accent: None,
        }
    }

    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = Some(accent);
        self
    }
}

pub struct DataTable<'a> {
    columns: Vec<TableColumn<'a>>,
    rows: Vec<TableRow>,
    row_height: f32,
}

impl<'a> DataTable<'a> {
    pub fn new(columns: Vec<TableColumn<'a>>, rows: Vec<TableRow>) -> Self {
        Self {
            columns,
            rows,
            row_height: 28.0,
        }
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = row_height;
        self
    }

    pub fn show(self, ui: &mut Ui) -> egui::Response {
        let height = self.row_height * (self.rows.len() as f32 + 1.0).max(2.0);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        sound::hook_response_auto("data_table", &response, true);
        let ctx = ui.ctx().clone();
        profiler::measure_ctx(&ctx, "data_table", || self.show_at(ui, rect));
        response
    }

    pub fn show_at(self, ui: &mut Ui, rect: Rect) {
        if self.columns.is_empty() || rect.width() <= 4.0 || rect.height() <= self.row_height {
            return;
        }

        paint::paint_recessed_panel(ui.painter(), rect, 1.0);
        let total_weight: f32 = self.columns.iter().map(|c| c.weight.max(0.1)).sum();
        let mut col_rects = Vec::with_capacity(self.columns.len());
        let mut x = rect.left();
        for column in &self.columns {
            let width = rect.width() * column.weight.max(0.1) / total_weight;
            let next_x = if col_rects.len() + 1 == self.columns.len() {
                rect.right()
            } else {
                x + width
            };
            col_rects.push(Rect::from_min_max(
                Pos2::new(x, rect.top()),
                Pos2::new(next_x, rect.bottom()),
            ));
            x = next_x;
        }

        let header = Rect::from_min_max(
            rect.min,
            Pos2::new(rect.right(), rect.top() + self.row_height),
        );
        ui.painter().rect_filled(
            header,
            egui::epaint::CornerRadius::ZERO,
            Color32::from_black_alpha(168),
        );
        ui.painter().hline(
            header.left()..=header.right(),
            header.bottom() - 0.5,
            Stroke::new(1.0, palette::BRASS_DARK),
        );

        for (idx, column) in self.columns.iter().enumerate() {
            let cell = Rect::from_min_max(
                Pos2::new(col_rects[idx].left(), header.top()),
                Pos2::new(col_rects[idx].right(), header.bottom()),
            )
            .shrink2(Vec2::new(2.0, 0.0));
            let font = fit_font_to_width(
                column.label,
                TextRole::Subheading.font_id(),
                (cell.width() - spacing::S6).max(8.0),
                0.72,
            );
            let painter = ui.painter().with_clip_rect(cell);
            painter.text(
                Pos2::new(column.align.x(cell), cell.center().y),
                column.align.anchor(),
                column.label,
                font,
                palette::GOLD,
            );
        }

        let max_rows = ((rect.height() - self.row_height) / self.row_height)
            .floor()
            .max(0.0) as usize;
        for (row_idx, row) in self.rows.iter().take(max_rows).enumerate() {
            let top = rect.top() + self.row_height * (row_idx as f32 + 1.0);
            let row_rect = Rect::from_min_max(
                Pos2::new(rect.left(), top),
                Pos2::new(rect.right(), top + self.row_height),
            );
            let fill = if row_idx % 2 == 0 {
                Color32::from_black_alpha(86)
            } else {
                Color32::from_black_alpha(126)
            };
            ui.painter()
                .rect_filled(row_rect, egui::epaint::CornerRadius::ZERO, fill);
            ui.painter().hline(
                row_rect.left()..=row_rect.right(),
                row_rect.bottom() - 0.5,
                Stroke::new(1.0, Color32::from_black_alpha(190)),
            );
            if let Some(accent) = row.accent {
                let bar = Rect::from_min_max(
                    Pos2::new(row_rect.left() + 1.0, row_rect.top() + 3.0),
                    Pos2::new(row_rect.left() + 4.0, row_rect.bottom() - 3.0),
                );
                ui.painter()
                    .rect_filled(bar, egui::epaint::CornerRadius::ZERO, accent);
            }
            for (idx, column) in self.columns.iter().enumerate() {
                let Some(cell_data) = row.cells.get(idx) else {
                    continue;
                };
                let align = cell_data.align.unwrap_or(column.align);
                let cell = Rect::from_min_max(
                    Pos2::new(col_rects[idx].left(), row_rect.top()),
                    Pos2::new(col_rects[idx].right(), row_rect.bottom()),
                )
                .shrink2(Vec2::new(2.0, 0.0));
                let font = if matches!(align, TableAlign::Right) {
                    TextRole::Numeric.font_id()
                } else {
                    TextRole::Body.font_id()
                };
                let font = fit_font_to_width(
                    &cell_data.text,
                    font,
                    (cell.width() - spacing::S6).max(8.0),
                    0.70,
                );
                let painter = ui.painter().with_clip_rect(cell);
                painter.text(
                    Pos2::new(align.x(cell), cell.center().y),
                    align.anchor(),
                    &cell_data.text,
                    font,
                    cell_data.color,
                );
            }
        }
        for boundary in col_rects.iter().take(col_rects.len().saturating_sub(1)) {
            let x = boundary.right();
            ui.painter().line_segment(
                [Pos2::new(x, header.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0, Color32::from_black_alpha(120)),
            );
        }
    }
}
