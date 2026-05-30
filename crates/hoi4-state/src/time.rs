//! 游戏时间系统。HOI4 以小时为最小单位，每天 24 tick。

use std::fmt;

/// 游戏内日期，对应 HOI4 的 "1936.1.1.12" 格式
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GameDate {
    pub year: u16,
    pub month: u8, // 1-12
    pub day: u8,   // 1-31
    pub hour: u8,  // 0-23
}

impl GameDate {
    pub const START: Self = Self {
        year: 1936,
        month: 1,
        day: 1,
        hour: 12,
    };

    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() < 3 {
            return None;
        }
        Some(Self {
            year: parts[0].parse().ok()?,
            month: parts[1].parse().ok()?,
            day: parts[2].parse().ok()?,
            hour: parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0),
        })
    }

    /// Days since year 0 (rough — for delta calculations)
    pub fn days_since_epoch(&self) -> i64 {
        let mut days = self.year as i64 * 365;
        // Simple days-per-month (no leap year handling for now)
        const DAYS_PER_MONTH: [i64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
        days += DAYS_PER_MONTH[(self.month - 1).min(11) as usize];
        days += self.day as i64 - 1;
        days
    }

    /// Total hours since epoch
    pub fn hours_since_epoch(&self) -> i64 {
        self.days_since_epoch() * 24 + self.hour as i64
    }

    /// Advance one hour
    pub fn advance_hour(&mut self) {
        self.hour += 1;
        if self.hour >= 24 {
            self.hour = 0;
            self.advance_day();
        }
    }

    fn advance_day(&mut self) {
        self.day += 1;
        let days_in_month = match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if Self::is_leap(self.year) {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        };
        if self.day > days_in_month {
            self.day = 1;
            self.month += 1;
            if self.month > 12 {
                self.month = 1;
                self.year += 1;
            }
        }
    }

    fn is_leap(year: u16) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    pub fn is_new_day(&self) -> bool {
        self.hour == 0
    }
    pub fn is_new_month(&self) -> bool {
        self.is_new_day() && self.day == 1
    }
    pub fn is_new_year(&self) -> bool {
        self.is_new_month() && self.month == 1
    }

    /// 真正的周判定：基于 days_since_epoch 的 7 天循环，避免依赖月内日号
    /// 1/8/15/22 这种"伪周"（在 30/31/28 天月份会出现 8-10 天的不均匀步长）。
    ///
    /// 锚点是 START（1936-01-01），其 `days_since_epoch() % 7 == 4`，
    /// 因此该日期不是 anchor day；anchor 序列从 START 开始的相对偏移上每隔 7 天命中一次。
    pub fn is_week_anchor(&self) -> bool {
        self.days_since_epoch() % 7 == 0
    }
}

impl fmt::Debug for GameDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{:02}.{:02}.{:02}",
            self.year, self.month, self.day, self.hour
        )
    }
}

impl fmt::Display for GameDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}, {}", month_name(self.month), self.day, self.year)
    }
}

fn month_name(m: u8) -> &'static str {
    [
        "",
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ][m.min(12) as usize]
}

/// 游戏速度
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameSpeed {
    Paused,
    Speed1, // 慢
    Speed2,
    Speed3, // 默认
    Speed4,
    Speed5, // 最快
}

impl GameSpeed {
    /// Real seconds per game hour at this speed
    pub fn seconds_per_hour(&self) -> f32 {
        match self {
            Self::Paused => f32::INFINITY,
            Self::Speed1 => 5.0 / 24.0,
            Self::Speed2 => 2.5 / 24.0,
            Self::Speed3 => 1.0 / 24.0,
            Self::Speed4 => 0.5 / 24.0,
            Self::Speed5 => 0.25 / 24.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date() {
        let d = GameDate::parse("1936.1.1.12").unwrap();
        assert_eq!(d.year, 1936);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 1);
        assert_eq!(d.hour, 12);
    }

    #[test]
    fn test_advance_hour() {
        let mut d = GameDate {
            year: 1936,
            month: 1,
            day: 31,
            hour: 23,
        };
        d.advance_hour();
        assert_eq!(
            d,
            GameDate {
                year: 1936,
                month: 2,
                day: 1,
                hour: 0
            }
        );

        let mut d = GameDate {
            year: 1936,
            month: 12,
            day: 31,
            hour: 23,
        };
        d.advance_hour();
        assert_eq!(
            d,
            GameDate {
                year: 1937,
                month: 1,
                day: 1,
                hour: 0
            }
        );
    }

    #[test]
    fn test_leap_year() {
        // 1936 is a leap year
        let mut d = GameDate {
            year: 1936,
            month: 2,
            day: 28,
            hour: 23,
        };
        d.advance_hour();
        assert_eq!(d.day, 29);
        d.advance_hour(); // advance through hours of feb 29
                          // Skip to next day
        for _ in 0..23 {
            d.advance_hour();
        }
        assert_eq!(d.month, 3);
        assert_eq!(d.day, 1);
    }
}
