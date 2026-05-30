//! `map/seasons.txt` parser — tree season date ranges.
//!
//! Vanilla `map/seasons.txt` declares four pairs of tree-season blocks
//! (`tree_winter` / `tree_winter2`, `tree_spring` / `tree_spring2`, etc.)
//! with `start_date = YY.MM.DD` and `end_date = YY.MM.DD` fields.
//!
//! The `SeasonsTxt` struct extracts these and provides a method to compute
//! `season_lerp` (0..1 blend factor) and `season_column` (0..7 index into
//! the `Tree_season.bmp` atlas columns) for any given in-game date.

/// One tree-season date range parsed from `map/seasons.txt`.
#[derive(Debug, Clone, Copy)]
pub struct TreeSeasonRange {
    /// Season name (e.g. "tree_winter", "tree_spring2").
    pub name: &'static str,
    /// Start date as (month, day). Year is always 00 in vanilla.
    pub start_md: (u32, u32),
    /// End date as (month, day).
    pub end_md: (u32, u32),
}

/// Parsed result from `map/seasons.txt`.
#[derive(Debug, Clone)]
pub struct SeasonsTxt {
    pub tree_seasons: Vec<TreeSeasonRange>,
}

/// Season column indices matching vanilla `Tree_season.bmp` layout.
///
/// The atlas is 8 columns × N rows. Columns 0..3 are the "primary" seasons,
/// columns 4..7 are the "secondary" (transition) seasons. The mapping is:
/// - 0 = tree_winter
/// - 1 = tree_spring
/// - 2 = tree_summer
/// - 3 = tree_autumn
/// - 4 = tree_winter2
/// - 5 = tree_spring2
/// - 6 = tree_summer2
/// - 7 = tree_autumn2
pub const SEASON_COLUMNS: &[&str; 8] = &[
    "tree_winter",
    "tree_spring",
    "tree_summer",
    "tree_autumn",
    "tree_winter2",
    "tree_spring2",
    "tree_summer2",
    "tree_autumn2",
];

/// Season result: which columns in `Tree_season.bmp` and blend between them.
#[derive(Debug, Clone, Copy)]
pub struct SeasonResult {
    /// Primary column index 0..7 into the `Tree_season.bmp` atlas.
    pub season_column: f32,
    /// Interpolation factor 0..1 within the current season column (intra-season).
    pub season_lerp: f32,
    /// Secondary column index 0..7 for inter-season blending (next season).
    pub season_column_next: f32,
    /// Inter-season blend factor 0..1: 0 = fully season_column, 1 = fully season_column_next.
    pub season_blend: f32,
}

impl SeasonsTxt {
    /// Compute the season column and lerp for a given date.
    ///
    /// `month` is 1..=12, `day` is 1..=31. The function finds the
    /// active tree-season range that contains this date and returns
    /// its column + a linear blend based on how far into the range we are.
    ///
    /// If no range matches (gap between seasons), returns the *closest*
    /// season with a lerp that indicates how close we are to entering it.
    pub fn season_for_date(&self, month: u32, day: u32) -> SeasonResult {
        let today = month as i32 * 100 + day as i32;

        let mut found_idx: Option<usize> = None;
        let mut found_lerp: f32 = 0.0;

        for (col_idx, &name) in SEASON_COLUMNS.iter().enumerate() {
            if let Some(range) = self.tree_seasons.iter().find(|r| r.name == name) {
                let start = range.start_md.0 as i32 * 100 + range.start_md.1 as i32;
                let end = range.end_md.0 as i32 * 100 + range.end_md.1 as i32;
                let in_range = if start <= end {
                    today >= start && today <= end
                } else {
                    today >= start || today <= end
                };
                if in_range {
                    let total = if start <= end {
                        end - start
                    } else {
                        (1200 - start) + end
                    };
                    let elapsed = if start <= end {
                        today - start
                    } else if today >= start {
                        1200 - start + today
                    } else {
                        today
                    };
                    found_lerp = if total > 0 {
                        (elapsed as f32 / total as f32).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    found_idx = Some(col_idx);
                    break;
                }
            }
        }

        let (col, lerp) = match found_idx {
            Some(i) => (i as f32, found_lerp),
            None => (2.0, 0.0),
        };

        let (next_col, blend) = self.compute_season_blend(today);

        SeasonResult {
            season_column: col,
            season_lerp: lerp,
            season_column_next: next_col,
            season_blend: blend,
        }
    }

    fn compute_season_blend(&self, today: i32) -> (f32, f32) {
        if self.tree_seasons.len() < 2 {
            let col = self.find_active_column(today);
            return (col, 0.0);
        }

        for (col_idx, &name) in SEASON_COLUMNS.iter().enumerate() {
            if let Some(range) = self.tree_seasons.iter().find(|r| r.name == name) {
                let start = range.start_md.0 as i32 * 100 + range.start_md.1 as i32;
                let end = range.end_md.0 as i32 * 100 + range.end_md.1 as i32;
                let in_range = if start <= end {
                    today >= start && today <= end
                } else {
                    today >= start || today <= end
                };
                if in_range {
                    let total = if start <= end {
                        end - start
                    } else {
                        (1200 - start) + end
                    };
                    let elapsed = if start <= end {
                        today - start
                    } else if today >= start {
                        1200 - start + today
                    } else {
                        today
                    };
                    let lerp = if total > 0 {
                        (elapsed as f32 / total as f32).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };

                    let transition_width = 0.15;
                    let blend;
                    let prev_col_idx;
                    if lerp < transition_width {
                        blend = (1.0 - lerp / transition_width).clamp(0.0, 1.0);
                        prev_col_idx = Self::prev_primary_season(col_idx);
                    } else if lerp > 1.0 - transition_width {
                        blend =
                            ((lerp - (1.0 - transition_width)) / transition_width).clamp(0.0, 1.0);
                        prev_col_idx = Self::next_primary_season(col_idx);
                    } else {
                        return (col_idx as f32, 0.0);
                    }
                    return (prev_col_idx as f32, blend);
                }
            }
        }

        (2.0, 0.0)
    }

    fn next_primary_season(current_col: usize) -> usize {
        let primary = current_col % 4;
        (primary + 1) % 4
    }

    fn prev_primary_season(current_col: usize) -> usize {
        let primary = current_col % 4;
        (primary + 3) % 4
    }

    fn find_active_column(&self, today: i32) -> f32 {
        for (col_idx, &name) in SEASON_COLUMNS.iter().enumerate() {
            if let Some(range) = self.tree_seasons.iter().find(|r| r.name == name) {
                let start = range.start_md.0 as i32 * 100 + range.start_md.1 as i32;
                let end = range.end_md.0 as i32 * 100 + range.end_md.1 as i32;
                let in_range = if start <= end {
                    today >= start && today <= end
                } else {
                    today >= start || today <= end
                };
                if in_range {
                    return col_idx as f32;
                }
            }
        }
        2.0
    }
}

/// Parse `map/seasons.txt` from raw text.
pub fn parse_seasons_txt(text: &str) -> SeasonsTxt {
    let mut seasons = Vec::new();

    let block = clausewitz_parser::parse(text);

    for &name in SEASON_COLUMNS {
        if let Some(sub) = block.get_block(name) {
            let start = parse_date_value(sub.get_string("start_date"));
            let end = parse_date_value(sub.get_string("end_date"));
            if let (Some(s), Some(e)) = (start, end) {
                seasons.push(TreeSeasonRange {
                    name: name,
                    start_md: s,
                    end_md: e,
                });
            }
        }
    }

    SeasonsTxt {
        tree_seasons: seasons,
    }
}

/// Parse a vanilla date string `"YY.MM.DD"` → `(month, day)`.
/// Year is ignored (always 00 in vanilla).
fn parse_date_value(s: Option<&str>) -> Option<(u32, u32)> {
    let s = s?;
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let month = parts.get(1).and_then(|v| v.parse::<u32>().ok())?;
    let day = parts.get(2).and_then(|v| v.parse::<u32>().ok())?;
    Some((month, day))
}

/// Load `map/seasons.txt` from disk. Returns a default (all-summer) on error.
pub fn load_seasons_txt(path: &std::path::Path) -> SeasonsTxt {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_seasons_txt(&text),
        Err(e) => {
            eprintln!(
                "[seasons] cannot read {}: {} — defaulting to summer",
                path.display(),
                e
            );
            SeasonsTxt {
                tree_seasons: Vec::new(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vanilla_seasons_txt() {
        let txt = r#"
tree_winter = {
    start_date=00.12.01
    end_date=00.02.10
}
tree_spring = {
    start_date=00.03.10
    end_date=00.04.22
}
tree_summer = {
    start_date=00.05.20
    end_date=00.09.10
}
tree_autumn = {
    start_date=00.10.10
    end_date=00.10.31
}
tree_winter2 = {
    start_date=00.11.15
    end_date=00.12.01
}
tree_spring2 = {
    start_date=00.02.20
    end_date=00.03.01
}
tree_summer2 = {
    start_date=00.06.20
    end_date=00.09.10
}
tree_autumn2 = {
    start_date=00.10.25
    end_date=00.11.01
}
"#;
        let s = parse_seasons_txt(txt);
        assert_eq!(s.tree_seasons.len(), 8);
        assert_eq!(s.tree_seasons[0].name, "tree_winter");
        assert_eq!(s.tree_seasons[0].start_md, (12, 1));
        assert_eq!(s.tree_seasons[0].end_md, (2, 10));
        assert_eq!(s.tree_seasons[1].name, "tree_spring");
        assert_eq!(s.tree_seasons[1].start_md, (3, 10));
    }

    #[test]
    fn season_for_date_summer() {
        let s = parse_seasons_txt(
            r#"
tree_winter = { start_date=00.12.01 end_date=00.02.10 }
tree_spring = { start_date=00.03.10 end_date=00.04.22 }
tree_summer = { start_date=00.05.20 end_date=00.09.10 }
tree_autumn = { start_date=00.10.10 end_date=00.10.31 }
"#,
        );
        let r = s.season_for_date(7, 1);
        assert_eq!(r.season_column, 2.0);
        assert!(r.season_lerp > 0.0);
        assert_eq!(r.season_blend, 0.0);
    }

    #[test]
    fn season_for_date_winter_wraps() {
        let s = parse_seasons_txt(
            r#"
tree_winter = { start_date=00.12.01 end_date=00.02.10 }
tree_spring = { start_date=00.03.10 end_date=00.04.22 }
tree_summer = { start_date=00.05.20 end_date=00.09.10 }
tree_autumn = { start_date=00.10.10 end_date=00.10.31 }
"#,
        );
        let r = s.season_for_date(1, 15);
        assert_eq!(r.season_column, 0.0);
        assert!(r.season_lerp > 0.0);
    }

    #[test]
    fn season_for_date_gap_defaults_to_summer() {
        let s = parse_seasons_txt(
            r#"
tree_summer = { start_date=00.06.01 end_date=00.08.31 }
"#,
        );
        let r = s.season_for_date(1, 1);
        assert_eq!(r.season_column, 2.0);
        assert_eq!(r.season_lerp, 0.0);
    }

    #[test]
    fn season_blend_at_start_of_season() {
        let s = parse_seasons_txt(
            r#"
tree_winter = { start_date=00.12.01 end_date=00.02.10 }
tree_spring = { start_date=00.03.10 end_date=00.04.22 }
tree_summer = { start_date=00.05.20 end_date=00.09.10 }
tree_autumn = { start_date=00.10.10 end_date=00.10.31 }
"#,
        );
        let r = s.season_for_date(5, 22);
        assert_eq!(r.season_column, 2.0);
        assert!(
            r.season_blend > 0.0,
            "start of summer should blend from spring"
        );
        assert_eq!(r.season_column_next, 1.0, "blend from spring (idx 1)");
    }

    #[test]
    fn season_blend_mid_season_is_zero() {
        let s = parse_seasons_txt(
            r#"
tree_winter = { start_date=00.12.01 end_date=00.02.10 }
tree_spring = { start_date=00.03.10 end_date=00.04.22 }
tree_summer = { start_date=00.05.20 end_date=00.09.10 }
tree_autumn = { start_date=00.10.10 end_date=00.10.31 }
"#,
        );
        let r = s.season_for_date(7, 15);
        assert_eq!(r.season_blend, 0.0, "mid-summer should have zero blend");
    }

    #[test]
    fn season_blend_at_end_of_season() {
        let s = parse_seasons_txt(
            r#"
tree_winter = { start_date=00.12.01 end_date=00.02.10 }
tree_spring = { start_date=00.03.10 end_date=00.04.22 }
tree_summer = { start_date=00.05.20 end_date=00.09.10 }
tree_autumn = { start_date=00.10.10 end_date=00.10.31 }
"#,
        );
        let r = s.season_for_date(9, 5);
        assert_eq!(r.season_column, 2.0);
        assert!(
            r.season_blend > 0.0,
            "end of summer should blend toward autumn"
        );
        assert_eq!(r.season_column_next, 3.0, "blend toward autumn (idx 3)");
    }

    #[test]
    fn parse_date_value_cases() {
        assert_eq!(parse_date_value(Some("00.12.01")), Some((12, 1)));
        assert_eq!(parse_date_value(Some("00.03.10")), Some((3, 10)));
        assert_eq!(parse_date_value(None), None);
        assert_eq!(parse_date_value(Some("garbage")), None);
    }
}
