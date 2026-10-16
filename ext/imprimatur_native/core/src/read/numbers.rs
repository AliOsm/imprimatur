//! How spreadsheet number formats show numbers: which formats are dates,
//! times or percentages, and Excel's General format. XLSX and XLS share
//! Excel's format codes and built-in formats.

use std::fmt::Write as _;

use crate::read::content::Value;
use crate::xlsx::DateTime;

/// How a number format shows a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    General,
    Date,
    Time,
    DateTime,
    /// A fixed number of decimals, thousands grouped with `,` or not, as a
    /// percentage or not: `0.00`, `#,##0`, `0%`.
    Fixed {
        decimals: u8,
        grouping: bool,
        percent: bool,
    },
}

/// Excel's built-in number formats (ECMA-376 Part 1, 18.8.30), with the
/// East Asian date formats.
pub const fn built_in(id: u32) -> Shown {
    match id {
        2 => fixed(2, false, false),
        3 | 37 | 38 => fixed(0, true, false),
        4 | 39 | 40 => fixed(2, true, false),
        9 => fixed(0, false, true),
        10 => fixed(2, false, true),
        14..=17 | 27..=31 | 34..=36 | 50..=58 => Shown::Date,
        18..=21 | 32 | 33 | 45..=47 => Shown::Time,
        22 => Shown::DateTime,
        _ => Shown::General,
    }
}

pub const fn fixed(decimals: u8, grouping: bool, percent: bool) -> Shown {
    Shown::Fixed { decimals, grouping, percent }
}

/// What a custom format code shows, from its first section: date and time
/// tokens, decimals, `,` grouping and `%`, outside quoted text, escapes and
/// `[...]` (colors, locales; `[h]` is elapsed time). Scientific formats
/// (`0.00E+00`) are shown as General.
pub fn classify(code: &str) -> Shown {
    let (mut date, mut time, mut month, mut percent) = (false, false, false, false);
    let (mut digits, mut point, mut decimals, mut grouping, mut scientific) =
        (false, false, 0_u8, false, false);
    let mut chars = code.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch.to_ascii_lowercase() {
            '0' | '#' | '?' if point => decimals = decimals.saturating_add(1),
            '0' | '#' | '?' => digits = true,
            '.' => point = true,
            ',' if digits && !point => grouping = true,
            'e' if matches!(chars.peek(), Some('+' | '-')) => scientific = true,
            ';' => break,
            '"' => {
                chars.by_ref().find(|&ch| ch == '"');
            }
            '\\' | '_' | '*' => {
                chars.next();
            }
            '[' => {
                let inner: String = chars.by_ref().take_while(|&ch| ch != ']').collect();
                time |= matches!(inner.to_ascii_lowercase().chars().next(), Some('h' | 'm' | 's'));
            }
            'y' | 'd' => date = true,
            'h' | 's' => time = true,
            'm' => month = true,
            '%' => percent = true,
            _ => {}
        }
    }
    match (date || (month && !time), time) {
        (true, true) => Shown::DateTime,
        (true, false) => Shown::Date,
        (false, true) => Shown::Time,
        (false, false) if scientific || !(decimals > 0 || grouping || percent) => Shown::General,
        (false, false) => Shown::Fixed { decimals, grouping, percent },
    }
}

/// Writes `number` as `shown` says; returns the value, a date when it is
/// shown as one. (Writing to a String cannot fail: the results of `write!`
/// here are dropped on purpose.)
pub fn push_number(
    out: &mut String,
    number: f64,
    shown: Shown,
    date1904: bool,
    scratch: &mut String,
) -> Value {
    let date = || DateTime::from_serial(number, date1904);
    match shown {
        Shown::Date if let Some(date) = date() => {
            let date = date.date_only();
            // Writing to a String cannot fail.
            let _ = write!(out, "{date}");
            return Value::Date(date);
        }
        Shown::DateTime if let Some(date) = date() => {
            // Writing to a String cannot fail.
            let _ = write!(out, "{date}");
            return Value::Date(date);
        }
        Shown::Time => {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "0 to 86400 after rem_euclid"
            )]
            let seconds = (number.fract() * 86_400.0).round().rem_euclid(86_400.0) as u32;
            // Writing to a String cannot fail.
            let _ =
                write!(out, "{:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60);
        }
        Shown::Fixed { decimals, grouping, percent } => {
            let number = if percent { number * 100.0 } else { number };
            push_fixed(out, number, decimals, grouping, scratch);
            if percent {
                out.push('%');
            }
        }
        _ => push_general(out, number, scratch),
    }
    Value::Number(number)
}

/// A number with `decimals` decimals, its thousands grouped with `,`,
/// rounded half away from zero as Excel does (`12.5` shows `13`, `1.005`
/// `1.01`): a few units in the last place toward the tie undo the binary
/// error that makes `1.005` `1.00499...`, which Rust would round down, as it
/// rounds an exact tie to even.
fn push_fixed(out: &mut String, number: f64, decimals: u8, grouping: bool, scratch: &mut String) {
    let number = (number.abs() * 4.0 * f64::EPSILON).mul_add(number.signum(), number);
    scratch.clear();
    // Writing to a String cannot fail.
    let _ = write!(scratch, "{number:.*}", usize::from(decimals));
    if !grouping {
        out.push_str(scratch);
        return;
    }
    let (sign, digits) =
        scratch.strip_prefix('-').map_or(("", scratch.as_str()), |rest| ("-", rest));
    let (whole, fraction) = digits.split_once('.').map_or((digits, None), |(w, f)| (w, Some(f)));
    out.push_str(sign);
    for (index, digit) in whole.char_indices() {
        if index > 0 && (whole.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    if let Some(fraction) = fraction {
        out.push('.');
        out.push_str(fraction);
    }
}

/// A number as Excel's General format shows it: 15 significant digits.
pub fn push_general(out: &mut String, number: f64, scratch: &mut String) {
    #[expect(clippy::cast_possible_truncation, reason = "whole and under 1e15: exact in i64")]
    if number.fract() == 0.0 && number.abs() < 1e15 {
        // Writing to a String cannot fail.
        let _ = write!(out, "{}", number as i64);
        return;
    }
    scratch.clear();
    // Writing to a String cannot fail.
    let _ = write!(scratch, "{number:.14e}");
    let rounded = scratch.parse::<f64>().unwrap_or(number);
    // Writing to a String cannot fail.
    let _ = write!(out, "{rounded}");
}
