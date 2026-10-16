//! Column widths from the cells. There is no font to measure text with, so
//! a column is as wide as its widest cell's character count says, scaled by
//! that cell's font.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use super::cell::Cell;
use super::format::Font;
use super::worksheet::Worksheet;

/// Excel's widest column, in characters.
const MAX_WIDTH: f64 = 255.0;

/// The width each column without one of its own needs for its widest cell:
/// `(characters + 3) × points / 10`, a bold cell counting one and a half
/// times, at most 255. Formulas are left out: their results are unknown.
pub fn widths(sheet: &Worksheet, base: &Font) -> BTreeMap<u16, f64> {
    let mut widths = BTreeMap::new();
    for (_, cells) in sheet.rows() {
        for (&column, entry) in cells {
            if sheet.column_widths().contains_key(&column) {
                continue;
            }
            let Some(characters) = characters(&entry.cell) else {
                continue;
            };
            let font = usize::try_from(entry.style)
                .ok()
                .and_then(|style| style.checked_sub(1))
                .and_then(|index| sheet.formats().get(index))
                .map_or(base, |format| &format.font);
            let bold = if font.bold { 1.5 } else { 1.0 };
            #[expect(
                clippy::cast_precision_loss,
                reason = "a cell holds at most 32 767 characters"
            )]
            let width = (characters as f64 + 3.0) * f64::from(font.half_points(base)) / 20.0 * bold;
            let width = (width.min(MAX_WIDTH) * 100.0).round() / 100.0;
            let widest = widths.entry(column).or_insert(0.0_f64);
            *widest = widest.max(width);
        }
    }
    widths
}

/// How many characters a cell shows unformatted: a text's longest line,
/// a number's digits, `TRUE`, a date as `yyyy-mm-dd` and a time
/// `yyyy-mm-dd hh:mm:ss`.
fn characters(cell: &Cell) -> Option<usize> {
    match cell {
        Cell::Text(text) => text.split('\n').map(|line| line.chars().count()).max(),
        Cell::Number(number) => Some(number.to_string().len()),
        Cell::Bool(value) => Some(if *value { 4 } else { 5 }),
        Cell::DateTime(date) => Some(if date.has_time() { 19 } else { 10 }),
        Cell::Formula(_) => None,
    }
}
