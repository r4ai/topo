//! Due-date input and display relative to today.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};

use crate::theme;

pub fn today() -> Date {
    jiff::Zoned::now().date()
}

/// Parses what the user typed as a due date: empty (clear), `today`,
/// `tomorrow`, `+3d` / `+2w`, `MM-DD` (this year) or `YYYY-MM-DD`.
pub fn parse_due(input: &str, today: Date) -> Result<Option<Date>, String> {
    let input = input.trim().to_lowercase();
    // `3d` or `2w` from today.
    let offset = |amount: &str| -> Option<Date> {
        let unit = amount.chars().last()?;
        let n: i64 = amount[..amount.len() - unit.len_utf8()].parse().ok()?;
        let span = match unit {
            'd' => n.days(),
            'w' => n.weeks(),
            _ => return None,
        };
        today.checked_add(span).ok()
    };
    let date = match input.as_str() {
        "" => return Ok(None),
        "today" => Some(today),
        "tomorrow" => today.tomorrow().ok(),
        s if s.starts_with('+') => offset(&s[1..]),
        s if s.len() == 5 => format!("{}-{s}", today.year()).parse().ok(),
        s => s.parse().ok(),
    };
    date.map(Some).ok_or_else(|| format!("`{input}` is not a date (try 2026-10-31, 10-31, +3d, +2w or tomorrow)"))
}

/// A moment as `2026-10-03 14:05` in the time zone `tz`.
pub fn moment(at: Timestamp, tz: TimeZone) -> String {
    at.to_zoned(tz).strftime("%Y-%m-%d %H:%M").to_string()
}

/// Short date such as `10/31`, with the year only when it is not this year.
pub fn short(date: Date, today: Date) -> String {
    match date.year() == today.year() {
        true => format!("{}/{}", date.month(), date.day()),
        false => format!("{}/{}/{}", date.year(), date.month(), date.day()),
    }
}

/// `today`, `in 3 days`, `2 days overdue`, …
pub fn relative(date: Date, today: Date) -> String {
    let days = (date - today).get_days();
    match days {
        0 => "today".into(),
        1 => "tomorrow".into(),
        -1 => "1 day overdue".into(),
        d if d < 0 => format!("{} days overdue", -d),
        d => format!("in {d} days"),
    }
}

/// Color for a due date of an open node: red when overdue, amber when close.
pub fn urgency_color(date: Date, today: Date) -> u32 {
    match (date - today).get_days() {
        d if d < 0 => theme::RED,
        d if d <= 3 => theme::AMBER,
        _ => theme::MUTED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn parses_relative_and_absolute_dates() {
        let today = date(2026, 9, 27);
        assert_eq!(parse_due("", today), Ok(None));
        assert_eq!(parse_due(" Today ", today), Ok(Some(today)));
        assert_eq!(parse_due("tomorrow", today), Ok(Some(date(2026, 9, 28))));
        assert_eq!(parse_due("+5d", today), Ok(Some(date(2026, 10, 2))));
        assert_eq!(parse_due("+2w", today), Ok(Some(date(2026, 10, 11))));
        assert_eq!(parse_due("10-31", today), Ok(Some(date(2026, 10, 31))));
        assert_eq!(parse_due("2027-01-05", today), Ok(Some(date(2027, 1, 5))));
        assert!(parse_due("soon", today).is_err());
        assert!(parse_due("+xd", today).is_err());
    }

    #[test]
    fn rejects_non_ascii_input_without_panicking() {
        let today = date(2026, 9, 27);
        for input in ["+あ", "+3日", "あした", "10月31", "+"] {
            assert!(parse_due(input, today).is_err(), "{input}");
        }
    }

    #[test]
    fn describes_dates_relative_to_today() {
        let today = date(2026, 9, 27);
        assert_eq!(relative(date(2026, 9, 25), today), "2 days overdue");
        assert_eq!(relative(today, today), "today");
        assert_eq!(relative(date(2026, 10, 7), today), "in 10 days");
        assert_eq!(short(date(2026, 10, 7), today), "10/7");
        assert_eq!(short(date(2027, 1, 2), today), "2027/1/2");
    }
}
