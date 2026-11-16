//! Calendar dates for the date filters. The caller passes today and the local offset; nothing here reads a clock.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CivilDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

const MONTHS: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

impl CivilDate {
    pub fn new(year: i32, month: u32, day: u32) -> Option<CivilDate> {
        let d = CivilDate { year, month, day };
        ((1..=12).contains(&month) && day >= 1 && day <= d.days_in_month()).then_some(d)
    }

    fn days_in_month(self) -> u32 {
        match self.month {
            4 | 6 | 9 | 11 => 30,
            2 if (self.year % 4 == 0 && self.year % 100 != 0) || self.year % 400 == 0 => 29,
            2 => 28,
            _ => 31,
        }
    }

    /// Days since 1970-01-01 (H. Hinnant's days_from_civil).
    pub fn days(self) -> i64 {
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// Local midnight at the start of this day, as ms since the epoch.
    pub fn start_ms(self, utc_offset_min: i32) -> i64 {
        self.days() * 86_400_000 - i64::from(utc_offset_min) * 60_000
    }

    pub fn next_day(self) -> CivilDate {
        if self.day < self.days_in_month() {
            CivilDate { day: self.day + 1, ..self }
        } else if self.month < 12 {
            CivilDate { month: self.month + 1, day: 1, ..self }
        } else {
            CivilDate { year: self.year + 1, month: 1, day: 1 }
        }
    }

    pub fn next_month(self) -> CivilDate {
        if self.month < 12 {
            CivilDate { month: self.month + 1, day: 1, ..self }
        } else {
            CivilDate { year: self.year + 1, month: 1, day: 1 }
        }
    }

    /// "2026-06-01".
    pub fn parse_iso(s: &str) -> Option<CivilDate> {
        let mut it = s.split('-');
        let (y, m, d) = (it.next()?, it.next()?, it.next()?);
        if it.next().is_some() || y.len() != 4 {
            return None;
        }
        CivilDate::new(y.parse().ok()?, m.parse().ok()?, d.parse().ok()?)
    }
}

/// "June", "jun": the month number. Case-blind; a three-letter prefix is enough, a longer word must be a prefix of the name.
pub fn month_of(word: &str) -> Option<u32> {
    let w = word.to_lowercase();
    if w.len() < 3 {
        return None;
    }
    MONTHS.iter().position(|m| m.starts_with(w.as_str())).map(|i| i as u32 + 1)
}

/// A date span typed in words: one day ("2026-06-01") or one month ("June", "June 2025").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub first: CivilDate,
    /// The first day after the span.
    pub end: CivilDate,
}

/// A bare month means its latest start not after today ("before June" on 2026-11-07 is before 2026-06-01;
/// "before December" on the same day is before 2025-12-01). Returns the span and how many words it took.
pub fn span_of(words: &[&str], today: CivilDate) -> Option<(Span, usize)> {
    let first = *words.first()?;
    if let Some(d) = CivilDate::parse_iso(first) {
        return Some((Span { first: d, end: d.next_day() }, 1));
    }
    let month = month_of(first)?;
    let (year, used) = match words.get(1).and_then(|w| w.parse::<i32>().ok()).filter(|y| (1000..=9999).contains(y)) {
        Some(y) => (y, 2),
        None => (if month <= today.month { today.year } else { today.year - 1 }, 1),
    };
    let start = CivilDate::new(year, month, 1)?;
    Some((Span { first: start, end: start.next_month() }, used))
}
