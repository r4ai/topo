//! Due-date input and display relative to today.

use gpui::Rgba;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};

use crate::theme;

pub fn today() -> Date {
    jiff::Zoned::now().date()
}

const WEEKDAYS: [&str; 7] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];

/// The first day after `today` that is the weekday `name`, given in full or by its first three letters.
fn next_weekday(name: &str, today: Date) -> Option<Date> {
    let index = WEEKDAYS.iter().position(|day| *day == name || day[..3] == *name)?;
    (1..=7)
        .filter_map(|days| today.checked_add(days.days()).ok())
        .find(|date| usize::try_from(date.weekday().to_monday_zero_offset()).is_ok_and(|offset| offset == index))
}

/// Due dates to offer for what is typed so far, each as text that
/// [`parse_due`] accepts and the date it means. They follow the input the way
/// an editor completes code: nothing typed offers the common ones, a number
/// offers that many days, weeks or months and that day of the month, letters
/// offer the words they start, and a whole date offers itself.
pub fn suggestions(typed: &str, today: Date) -> Vec<(String, Date)> {
    let typed = typed.trim().to_lowercase();
    let words = ["today", "tomorrow"].into_iter().chain(WEEKDAYS);
    let mut texts: Vec<String> = match typed.trim_start_matches('+') {
        "" if typed.is_empty() => ["today", "tomorrow", "+3d", "+1w", "+2w", "+1m"].map(str::to_owned).into(),
        number if !number.is_empty() && number.len() <= 3 && number.bytes().all(|b| b.is_ascii_digit()) => {
            let offsets = ["d", "w", "m"].map(|unit| format!("+{number}{unit}"));
            // The next day of a month with that number.
            let day = number.parse::<i8>().ok().filter(|_| !typed.starts_with('+'));
            let dated = day.and_then(|day| {
                let this_month = today.with().day(day).build().ok().filter(|date| *date > today);
                let first = today.first_of_month().checked_add(1.months()).ok()?;
                this_month.or_else(|| first.with().day(day).build().ok())
            });
            offsets.into_iter().chain(dated.map(|date| date.to_string())).collect()
        }
        _ => words.filter(|word| word.starts_with(&typed)).map(str::to_owned).collect(),
    };
    let dated = |text: String| Some((parse_due(&text, today).ok()??, text));
    let mut suggestions: Vec<(String, Date)> = texts.drain(..).filter_map(dated).map(|(d, t)| (t, d)).collect();
    // What is typed is itself a date that no suggestion spells out.
    if let Ok(Some(date)) = parse_due(&typed, today)
        && !suggestions.iter().any(|(_, suggested)| *suggested == date)
    {
        suggestions.insert(0, (typed, date));
    }
    suggestions
}

/// Parses what the user typed as a due date: empty (clear), `today`,
/// `tomorrow`, a weekday (`friday`, `fri`: the next one), `+3d` / `+2w` /
/// `+1m`, `MM-DD` (this year) or `YYYY-MM-DD`.
pub fn parse_due(input: &str, today: Date) -> Result<Option<Date>, String> {
    let input = input.trim().to_lowercase();
    // `3d` or `2w` from today.
    let offset = |amount: &str| -> Option<Date> {
        let unit = amount.chars().last()?;
        let n: i64 = amount[..amount.len() - unit.len_utf8()].parse().ok()?;
        let span = match unit {
            'd' => n.days(),
            'w' => n.weeks(),
            'm' => n.months(),
            _ => return None,
        };
        today.checked_add(span).ok()
    };
    let date = match input.as_str() {
        "" => return Ok(None),
        "today" => Some(today),
        "tomorrow" => today.tomorrow().ok(),
        s if s.starts_with('+') => offset(&s[1..]),
        s if s.is_ascii() && s.len() >= 3 && next_weekday(s, today).is_some() => next_weekday(s, today),
        s if s.len() == 5 => format!("{}-{s}", today.year()).parse().ok(),
        s => s.parse().ok(),
    };
    date.map(Some)
        .ok_or_else(|| format!("`{input}` is not a date (try 2026-10-31, 10-31, +3d, +2w, friday or tomorrow)"))
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
pub fn urgency_color(date: Date, today: Date) -> Rgba {
    let t = theme::current();
    match (date - today).get_days() {
        d if d < 0 => t.danger,
        d if d <= 3 => t.warn,
        _ => t.fg_muted,
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
    fn parses_weekdays_and_months() {
        // 2026-09-27 is a Sunday.
        let today = date(2026, 9, 27);
        assert_eq!(parse_due("mon", today), Ok(Some(date(2026, 9, 28))));
        assert_eq!(parse_due("Friday", today), Ok(Some(date(2026, 10, 2))));
        assert_eq!(parse_due("sunday", today), Ok(Some(date(2026, 10, 4))), "never today");
        assert_eq!(parse_due("+1m", today), Ok(Some(date(2026, 10, 27))));
        assert!(parse_due("fr", today).is_err());
    }

    #[test]
    fn suggestions_follow_what_is_typed() {
        let today = date(2026, 9, 27);
        let texts = |typed: &str| suggestions(typed, today).into_iter().map(|(text, _)| text).collect::<Vec<_>>();
        assert_eq!(texts(""), ["today", "tomorrow", "+3d", "+1w", "+2w", "+1m"]);
        assert_eq!(texts("t"), ["today", "tomorrow", "tuesday", "thursday"]);
        assert_eq!(texts("3"), ["+3d", "+3w", "+3m", "2026-10-03"]);
        assert_eq!(texts("30"), ["+30d", "+30w", "+30m", "2026-09-30"]);
        assert_eq!(texts("+2"), ["+2d", "+2w", "+2m"]);
        assert_eq!(texts("fri"), ["friday"]);
        assert_eq!(texts("10-31"), ["10-31"]);
        assert_eq!(texts("2027-01-05"), ["2027-01-05"]);
        assert_eq!(suggestions("fri", today)[0].1, date(2026, 10, 2));
        assert!(texts("soon").is_empty() && texts("あした").is_empty() && texts("+").is_empty());
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
