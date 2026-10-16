//! A workbook's cell formats (`styles.xml`): for each, how it shows numbers
//! and how it looks.

use std::borrow::Cow;
use std::sync::Arc;

use crate::Result;
use crate::read::Budget;
use crate::read::content::Look;
use crate::read::numbers::{Shown, built_in, classify};
use crate::xml::reader::{Event, XmlReader};

/// The cell formats (`cellXfs`), by index.
#[derive(Default)]
pub struct Formats {
    pub shown: Vec<Shown>,
    pub looks: Vec<Arc<Look>>,
}

/// Where in `styles.xml` the reader is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Other,
    Fonts,
    Fills,
    Borders,
    CellFormats,
}

/// A font, fill and border by their index, before the formats pick them.
#[derive(Default)]
struct Parts {
    codes: Vec<(u32, String)>,
    fonts: Vec<Look>,
    fills: Vec<Option<String>>,
    borders: Vec<Option<String>>,
}

pub fn formats(text: &str, budget: &Budget<'_>) -> Result<Formats> {
    let mut xml = XmlReader::new(text, budget.max_depth());
    let (mut parts, mut section, mut side) = (Parts::default(), Section::Other, "");
    let mut formats = Formats { shown: Vec::new(), looks: Vec::new() };
    while let Some(event) = xml.next()? {
        match event {
            Event::Start(tag) => match (section, tag.name()) {
                (_, "numFmt") => {
                    if let (Some(id), Some(code)) = (tag.attr("numFmtId"), tag.attr("formatCode")) {
                        parts.codes.push((id.parse().unwrap_or(u32::MAX), code.into_owned()));
                    }
                }
                (_, "fonts") => section = Section::Fonts,
                (_, "fills") => section = Section::Fills,
                (_, "borders") => section = Section::Borders,
                (_, "cellXfs") => section = Section::CellFormats,
                (Section::Fonts, "font") => parts.fonts.push(Look::default()),
                (Section::Fonts, name) => font_part(&mut parts.fonts, name, tag),
                (Section::Fills, "fill") => parts.fills.push(None),
                (Section::Fills, "fgColor") => {
                    if let Some(fill) = parts.fills.last_mut() {
                        *fill = tag.attr("rgb").map(|rgb| rgb.to_uppercase());
                    }
                }
                (Section::Borders, "border") => parts.borders.push(None),
                (Section::Borders, name) if side.is_empty() => {
                    side = name;
                    if name == "left"
                        && let Some(border) = parts.borders.last_mut()
                    {
                        *border = tag.attr("style").map(Cow::into_owned);
                    }
                }
                (Section::CellFormats, "xf") => {
                    let id = |name| tag.parse::<usize>(name);
                    let number = tag.parse("numFmtId").unwrap_or(0);
                    let custom = parts.codes.iter().find(|(id, _)| *id == number);
                    let shown = custom.map_or_else(|| built_in(number), |(_, code)| classify(code));
                    let font = parts.fonts.get(id("fontId").unwrap_or(0));
                    let mut look = font.cloned().unwrap_or_default();
                    look.fill = parts.fills.get(id("fillId").unwrap_or(0)).cloned().flatten();
                    look.border = parts.borders.get(id("borderId").unwrap_or(0)).cloned().flatten();
                    look.number_format = custom.map_or_else(
                        || built_in_code(number).map_or_else(|| number.to_string(), str::to_owned),
                        |(_, code)| code.clone(),
                    );
                    formats.shown.push(shown);
                    formats.looks.push(Arc::new(look));
                }
                (Section::CellFormats, "alignment") => {
                    if let Some(look) = formats.looks.last_mut().map(Arc::make_mut) {
                        look.align = tag.attr("horizontal").map(Cow::into_owned);
                        look.valign = tag.attr("vertical").map(Cow::into_owned);
                        look.wrap = tag.on("wrapText");
                    }
                }
                _ => {}
            },
            Event::End(name) => match name {
                "fonts" | "fills" | "borders" | "cellXfs" => section = Section::Other,
                _ if name == side => side = "",
                _ => {}
            },
            Event::Text(_) => {}
        }
    }
    Ok(formats)
}

/// A property of the font being read.
fn font_part(fonts: &mut [Look], name: &str, tag: crate::xml::reader::Tag<'_>) {
    let Some(font) = fonts.last_mut() else { return };
    match name {
        "b" => font.bold = tag.flag("val"),
        "i" => font.italic = tag.flag("val"),
        "u" => font.underline = tag.attr("val").is_none_or(|style| style != "none"),
        "sz" => font.size = tag.parse("val").unwrap_or(0.0),
        "name" => font.font = tag.attr("val").map(Cow::into_owned).unwrap_or_default(),
        "color" => font.color = tag.attr("rgb").map(|rgb| rgb.to_uppercase()),
        _ => {}
    }
}

/// The codes of Excel's built-in number formats (ECMA-376 Part 1, 18.8.30).
const fn built_in_code(id: u32) -> Option<&'static str> {
    Some(match id {
        0 => "General",
        1 => "0",
        2 => "0.00",
        3 => "#,##0",
        4 => "#,##0.00",
        9 => "0%",
        10 => "0.00%",
        11 => "0.00E+00",
        12 => "# ?/?",
        13 => "# ??/??",
        14 => "mm-dd-yy",
        15 => "d-mmm-yy",
        16 => "d-mmm",
        17 => "mmm-yy",
        18 => "h:mm AM/PM",
        19 => "h:mm:ss AM/PM",
        20 => "h:mm",
        21 => "h:mm:ss",
        22 => "m/d/yy h:mm",
        37 => "#,##0 ;(#,##0)",
        38 => "#,##0 ;[Red](#,##0)",
        39 => "#,##0.00;(#,##0.00)",
        40 => "#,##0.00;[Red](#,##0.00)",
        45 => "mm:ss",
        46 => "[h]:mm:ss",
        47 => "mmss.0",
        48 => "##0.0E+0",
        49 => "@",
        _ => return None,
    })
}
