//! `.ods` spreadsheets: a zip whose `mimetype` entry names the format and
//! whose `content.xml` holds the tables. Cells carry their
//! value (`office:value-type` and its attributes) and the text they show;
//! repeated rows and columns are expanded only where they hold something.

#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::ops::Range;

use super::Budget;
use super::content::{Grid, Note, Sheet, Value};
use super::numbers::push_general;
use crate::xlsx::DateTime;
use crate::xml::reader::{Event, Tag, XmlReader};
use crate::zip::reader::ZipReader;
use crate::{Error, Result};

const SPREADSHEET: &str = "application/vnd.oasis.opendocument.spreadsheet";
/// Excel's grid, which repeated rows and columns are cut to.
const MAX_ROWS: u32 = 1_048_576;
const MAX_COLUMNS: u32 = 16_384;

/// Whether a zip is an ODF file, by its `mimetype` entry: `None` when it is
/// not one, `Some(true)` for a spreadsheet (or its template).
///
/// # Errors
/// As reading the entry.
pub fn spreadsheet(zip: &ZipReader<'_>, budget: &Budget<'_>) -> Result<Option<bool>> {
    let Some(kind) = zip.read("mimetype", budget)? else { return Ok(None) };
    let kind = String::from_utf8_lossy(&kind);
    let kind = kind.trim();
    if !kind.starts_with("application/vnd.oasis.opendocument.") {
        return Ok(None);
    }
    Ok(Some(kind.strip_suffix("-template").unwrap_or(kind) == SPREADSHEET))
}

/// The spreadsheet's tables, in order, with their comments.
///
/// # Errors
/// [`Error::Malformed`] for a missing or broken `content.xml`, and the
/// limits' errors.
pub fn content(zip: &ZipReader<'_>, budget: &Budget<'_>) -> Result<Vec<Sheet>> {
    let content = zip
        .read("content.xml", budget)?
        .ok_or_else(|| Error::malformed("ODS", "no content.xml"))?;
    let text = String::from_utf8(content)
        .map_err(|_| Error::malformed("ODS", "content.xml is not UTF-8"))?;
    let mut reader = Reader::default();
    let mut xml = XmlReader::new(&text, budget.max_depth());
    while let Some(event) = xml.next()? {
        budget.tick()?;
        reader.event(event, budget)?;
    }
    Ok(reader.sheets)
}

/// A cell being read: its value attributes, its repeat, and where its text
/// starts in the row's buffer.
#[derive(Default)]
struct Cell {
    kind: String,
    value: String,
    formula: String,
    repeat: u32,
    paragraphs: usize,
    start: usize,
}

#[derive(Default)]
struct Reader {
    sheets: Vec<Sheet>,
    hidden_styles: Vec<String>,
    style: Option<String>,
    grid: Grid,
    comments: Vec<Note>,
    row: u32,
    column: u32,
    row_repeat: u32,
    /// The row's cells, their text in one buffer: column, text, value.
    row_text: String,
    row_cells: Vec<(u32, Range<usize>, Value)>,
    cell: Option<Cell>,
    note: Option<(String, String)>,
    in_creator: bool,
    /// Text counts inside paragraphs only, outside drawings and dates of
    /// annotations.
    paragraphs: usize,
    skipping: usize,
    scratch: String,
}

impl Reader {
    fn event(&mut self, event: Event<'_>, budget: &Budget<'_>) -> Result<()> {
        match event {
            Event::Start(tag) => self.start(&tag),
            Event::End(name) => self.end(name, budget)?,
            Event::Text(text) => self.text(&text),
        }
        Ok(())
    }

    fn start(&mut self, tag: &Tag<'_>) {
        let repeat = |name| tag.attr(name).and_then(|n| n.parse::<u32>().ok()).unwrap_or(1).max(1);
        match tag.name() {
            "style" if tag.attr("family").as_deref() == Some("table") => {
                self.style = tag.attr("name").map(std::borrow::Cow::into_owned);
            }
            "table-properties" if tag.attr("display").as_deref() == Some("false") => {
                if let Some(style) = self.style.take() {
                    self.hidden_styles.push(style);
                }
            }
            "table" => {
                let hidden = tag
                    .attr("style-name")
                    .is_some_and(|style| self.hidden_styles.iter().any(|h| *h == style));
                self.sheets.push(Sheet {
                    name: tag.attr("name").map(std::borrow::Cow::into_owned).unwrap_or_default(),
                    hidden,
                    ..Sheet::default()
                });
                self.row = 0;
            }
            "table-row" => {
                self.column = 0;
                self.row_repeat = repeat("number-rows-repeated");
            }
            "table-cell" | "covered-table-cell" => {
                let attr =
                    |name| tag.attr(name).map(std::borrow::Cow::into_owned).unwrap_or_default();
                let kind = attr("value-type");
                let value = match kind.as_str() {
                    "date" => attr("date-value"),
                    "time" => attr("time-value"),
                    "boolean" => attr("boolean-value"),
                    _ => attr("value"),
                };
                let repeat = repeat("number-columns-repeated");
                let start = self.row_text.len();
                self.cell = Some(Cell {
                    kind,
                    value,
                    formula: attr("formula"),
                    repeat,
                    paragraphs: 0,
                    start,
                });
            }
            "annotation" => self.note = Some((String::new(), String::new())),
            "creator" if self.note.is_some() => self.in_creator = true,
            "date" if self.note.is_some() => self.skipping += 1,
            "frame" | "custom-shape" | "g" => self.skipping += 1,
            "p" | "h" => {
                // A second paragraph starts on a line of its own.
                let separate = match (&mut self.note, &mut self.cell) {
                    (Some((_, text)), _) => !text.is_empty(),
                    (None, Some(cell)) => {
                        cell.paragraphs += 1;
                        cell.paragraphs > 1
                    }
                    (None, None) => false,
                };
                self.paragraphs += 1;
                if separate {
                    self.text("\n");
                }
            }
            "s" => {
                let count = tag
                    .attr("c")
                    .and_then(|count| count.parse::<usize>().ok())
                    .unwrap_or(1)
                    .min(1024);
                self.text(&" ".repeat(count));
            }
            "tab" => self.text("\t"),
            "line-break" => self.text("\n"),
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        if self.skipping > 0 {
            return;
        }
        match &mut self.note {
            Some((author, _)) if self.in_creator => author.push_str(text),
            Some((_, note)) if self.paragraphs > 0 => note.push_str(text),
            None if self.cell.is_some() && self.paragraphs > 0 => self.row_text.push_str(text),
            _ => {}
        }
    }

    fn end(&mut self, name: &str, budget: &Budget<'_>) -> Result<()> {
        match name {
            "style" => self.style = None,
            "creator" => self.in_creator = false,
            "date" if self.note.is_some() => self.skipping = self.skipping.saturating_sub(1),
            "frame" | "custom-shape" | "g" => self.skipping = self.skipping.saturating_sub(1),
            "p" | "h" => self.paragraphs = self.paragraphs.saturating_sub(1),
            "annotation" => {
                if let Some((author, text)) = self.note.take() {
                    let author = (!author.trim().is_empty()).then(|| author.trim().into());
                    self.comments.push(Note { row: self.row, column: self.column, author, text });
                }
            }
            "table-cell" | "covered-table-cell" => self.end_cell(),
            "table-row" => self.end_row(budget)?,
            "table" => {
                let grid = std::mem::take(&mut self.grid).finish();
                if let Some(sheet) = self.sheets.last_mut() {
                    sheet.cells = grid;
                    sheet.comments = std::mem::take(&mut self.comments);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The cell's text, typed by its value; repeated across its columns.
    fn end_cell(&mut self) {
        let Some(cell) = self.cell.take() else { return };
        let start = cell.start;
        let shown = self.row_text.split_off(start);
        let value = self.typed(&cell, &shown);
        let end = self.row_text.len();
        if end > start {
            let repeat = cell.repeat.min(MAX_COLUMNS.saturating_sub(self.column));
            for offset in 0..repeat {
                self.row_cells.push((self.column + offset, start..end, value));
            }
        }
        self.column = self.column.saturating_add(cell.repeat);
    }

    /// Writes the cell's text as shown into the row buffer, returning its
    /// value: numbers keep the text they show, dates and times read as in
    /// XLSX, a formula without a result shows itself.
    fn typed(&mut self, cell: &Cell, shown: &str) -> Value {
        let out = &mut self.row_text;
        match cell.kind.as_str() {
            "float" | "percentage" | "currency" if let Ok(number) = cell.value.parse::<f64>() => {
                if shown.trim().is_empty() {
                    push_general(out, number, &mut self.scratch);
                } else {
                    out.push_str(shown);
                }
                Value::Number(number)
            }
            "date" if let Some(date) = date(&cell.value) => {
                // Writing to a String cannot fail.
                let _ = write!(out, "{date}");
                Value::Date(date)
            }
            "time" if let Some(seconds) = duration(&cell.value) => {
                let clock = seconds.rem_euclid(86_400);
                // Writing to a String cannot fail.
                let _ = write!(out, "{:02}:{:02}:{:02}", clock / 3600, clock / 60 % 60, clock % 60);
                #[expect(clippy::cast_precision_loss, reason = "seconds of a day")]
                Value::Number(seconds as f64 / 86_400.0)
            }
            "boolean" => {
                let on = cell.value == "true" || cell.value == "1";
                out.push_str(if on { "TRUE" } else { "FALSE" });
                Value::Bool(on)
            }
            "" if !cell.formula.is_empty() && shown.trim().is_empty() => {
                let formula =
                    cell.formula.split_once(':').map_or(cell.formula.as_str(), |(_, rest)| rest);
                out.push('=');
                out.push_str(formula.strip_prefix('=').unwrap_or(formula));
                Value::Formula
            }
            _ => {
                out.push_str(shown);
                Value::Text
            }
        }
    }

    /// Writes the row's cells, once per repeat, and moves past it.
    fn end_row(&mut self, budget: &Budget<'_>) -> Result<()> {
        let repeat = if self.row_cells.is_empty() {
            0
        } else {
            self.row_repeat.min(MAX_ROWS.saturating_sub(self.row))
        };
        for offset in 0..repeat {
            budget.tick()?;
            budget.inflate(self.row_text.len())?;
            for (column, text, value) in &self.row_cells {
                if text.end > text.start {
                    self.grid.buffer().push_str(&self.row_text[text.clone()]);
                    self.grid.end_cell(self.row + offset, *column, *value);
                }
            }
        }
        self.row_cells.clear();
        self.row_text.clear();
        self.row = self.row.saturating_add(self.row_repeat);
        Ok(())
    }
}

/// `2023-03-15` or `2023-03-15T09:30:00.5`.
fn date(value: &str) -> Option<DateTime> {
    let (day, time) = value.split_once('T').unwrap_or((value, ""));
    let mut parts = day.splitn(3, '-').map(str::parse::<u16>);
    let (year, month, day) = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
    let date = DateTime::date(year, u8::try_from(month).ok()?, u8::try_from(day).ok()?).ok()?;
    if time.is_empty() {
        return Some(date);
    }
    let mut parts = time.splitn(3, ':');
    let (hour, minute, second) = (parts.next()?, parts.next()?, parts.next().unwrap_or("0"));
    let seconds: f64 = second.parse().ok()?;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "under 60, checked by `at`"
    )]
    let (whole, nanosecond) = (seconds.trunc() as u8, (seconds.fract() * 1e9).round() as u32);
    date.at(hour.parse().ok()?, minute.parse().ok()?, whole, nanosecond).ok()
}

/// An ISO 8601 duration such as `PT12H30M00S` (or with days), in seconds.
fn duration(value: &str) -> Option<i64> {
    let (negative, value) = value.strip_prefix('-').map_or((false, value), |rest| (true, rest));
    let value = value.strip_prefix('P')?;
    let (mut seconds, mut number, mut in_time) = (0_f64, String::new(), false);
    for ch in value.chars() {
        match ch {
            'T' => in_time = true,
            '0'..='9' | '.' => number.push(ch),
            unit => {
                let amount: f64 = number.parse().ok()?;
                number.clear();
                let scale = match (unit, in_time) {
                    ('D', false) => 86_400.0,
                    ('H', true) => 3_600.0,
                    ('M', true) => 60.0,
                    ('S', true) => 1.0,
                    _ => return None,
                };
                seconds = amount.mul_add(scale, seconds);
            }
        }
    }
    #[expect(clippy::cast_possible_truncation, reason = "rounded seconds")]
    let seconds = seconds.round() as i64;
    Some(if negative { -seconds } else { seconds })
}
