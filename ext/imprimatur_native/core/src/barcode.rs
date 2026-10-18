//! Barcodes: QR Code and Data Matrix, EAN-13, EAN-8, UPC-A and Code 128,
//! and the GS1 forms of QR Code, Data Matrix and Code 128 (GS1-128), drawn
//! as SVG, as PNG, or into a [`Document`](crate::doc::Document) as shapes.
//!
//! QR Code and Data Matrix come from the `qrcode` and `datamatrix` crates;
//! the linear symbologies, GS1 element strings and the drawing are ours.
//!
//! ```
//! use imprimatur::barcode::{Barcode, Level, Look};
//!
//! let qr = Barcode::qr("https://id.gs1.org/01/09506000134352", Level::Medium)?;
//! assert!(qr.svg(4.0, &Look::default())?.starts_with("<svg"));
//!
//! let ean = Barcode::ean13("950600013435")?;
//! assert_eq!(ean.text(), "9506000134352"); // the check digit, added
//!
//! let label = Barcode::gs1_128("(00)106141412345678908")?;
//! assert!(label.png(3, &Look::default())?.starts_with(b"\x89PNG"));
//! # Ok::<(), imprimatur::Error>(())
//! ```

mod code128;
mod draw;
mod ean;
mod gs1;
mod matrix;
#[cfg(test)]
mod tests;

pub(crate) use draw::Shapes;
pub use gs1::check_digit;
pub use matrix::Level;

use crate::style::Color;
use crate::{Error, Result};

/// The kinds of barcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Symbology {
    QrCode,
    DataMatrix,
    Ean13,
    Ean8,
    UpcA,
    Code128,
}

impl Symbology {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::QrCode => "QR Code",
            Self::DataMatrix => "Data Matrix",
            Self::Ean13 => "EAN-13",
            Self::Ean8 => "EAN-8",
            Self::UpcA => "UPC-A",
            Self::Code128 => "Code 128",
        }
    }

    /// Whether it is one row of bars, read across.
    #[must_use]
    pub const fn is_linear(self) -> bool {
        matches!(self, Self::Ean13 | Self::Ean8 | Self::UpcA | Self::Code128)
    }

    /// The blank margin readers need, in modules: left, right, and above
    /// and below.
    const fn quiet_zone(self) -> (usize, usize, usize) {
        match self {
            Self::QrCode => (4, 4, 4),
            Self::DataMatrix => (1, 1, 1),
            Self::Ean13 => (11, 7, 0),
            Self::Ean8 => (7, 7, 0),
            Self::UpcA => (9, 9, 0),
            Self::Code128 => (10, 10, 0),
        }
    }

    pub(crate) fn invalid(self, reason: impl Into<String>) -> Error {
        Error::InvalidBarcode { symbology: self.name(), reason: reason.into() }
    }
}

/// A barcode symbol: its modules, dark or light, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Barcode {
    symbology: Symbology,
    gs1: bool,
    text: String,
    columns: usize,
    rows: usize,
    /// Row by row; a linear code has one row.
    modules: Vec<bool>,
}

impl Barcode {
    /// A QR Code of text, such as a URL (a GS1 Digital Link is one).
    ///
    /// # Errors
    /// [`Error::InvalidBarcode`] when the text does not fit at `level`.
    pub fn qr(text: &str, level: Level) -> Result<Self> {
        let (side, modules) = matrix::qr(text.as_bytes(), level)?;
        Ok(Self::matrix(Symbology::QrCode, false, text, (side, side), modules))
    }

    /// A GS1 QR Code of an element string such as
    /// `(01)09506000134352(10)AB12`.
    ///
    /// # Errors
    /// [`Error::InvalidElementString`], or [`Error::InvalidBarcode`] when it
    /// does not fit at `level`.
    pub fn gs1_qr(element_string: &str, level: Level) -> Result<Self> {
        let units = gs1::element_units(element_string)?;
        let (side, modules) = matrix::gs1_qr(&units, level)?;
        Ok(Self::matrix(Symbology::QrCode, true, element_string, (side, side), modules))
    }

    /// A square Data Matrix of text: Latin-1 as is, anything else as UTF-8.
    ///
    /// # Errors
    /// [`Error::InvalidBarcode`] when the text does not fit.
    pub fn data_matrix(text: &str) -> Result<Self> {
        let (size, modules) = matrix::data_matrix(text)?;
        Ok(Self::matrix(Symbology::DataMatrix, false, text, size, modules))
    }

    /// A square GS1 `DataMatrix` of an element string such as
    /// `(01)09506000134352(17)251231(10)AB12`.
    ///
    /// # Errors
    /// [`Error::InvalidElementString`], or [`Error::InvalidBarcode`] when it
    /// does not fit.
    pub fn gs1_data_matrix(element_string: &str) -> Result<Self> {
        let units = gs1::element_units(element_string)?;
        let (size, modules) = matrix::gs1_data_matrix(&units)?;
        Ok(Self::matrix(Symbology::DataMatrix, true, element_string, size, modules))
    }

    /// An EAN-13 of a GTIN-13: 12 digits and the check digit is added, or
    /// 13 and it is checked.
    ///
    /// # Errors
    /// [`Error::InvalidBarcode`] for anything else, or a wrong check digit.
    pub fn ean13(digits: &str) -> Result<Self> {
        let digits = ean::digits(Symbology::Ean13, digits, 13)?;
        Ok(Self::linear(Symbology::Ean13, false, ean::text(&digits), ean::ean13(&digits)))
    }

    /// An EAN-8 of a GTIN-8: 7 digits, or 8 with the check digit.
    ///
    /// # Errors
    /// [`Error::InvalidBarcode`] for anything else, or a wrong check digit.
    pub fn ean8(digits: &str) -> Result<Self> {
        let digits = ean::digits(Symbology::Ean8, digits, 8)?;
        Ok(Self::linear(Symbology::Ean8, false, ean::text(&digits), ean::ean8(&digits)))
    }

    /// A UPC-A of a GTIN-12: 11 digits, or 12 with the check digit. Its bars
    /// are those of the EAN-13 with a leading zero.
    ///
    /// # Errors
    /// [`Error::InvalidBarcode`] for anything else, or a wrong check digit.
    pub fn upc_a(digits: &str) -> Result<Self> {
        let digits = ean::digits(Symbology::UpcA, digits, 12)?;
        let mut ean = Vec::with_capacity(13);
        ean.push(0);
        ean.extend_from_slice(&digits);
        Ok(Self::linear(Symbology::UpcA, false, ean::text(&digits), ean::ean13(&ean)))
    }

    /// A Code 128 of ASCII text, switching to code set C for runs of digits.
    ///
    /// # Errors
    /// [`Error::InvalidBarcode`] for text beyond ASCII, empty, or longer than
    /// 128 characters.
    pub fn code128(text: &str) -> Result<Self> {
        let units: Vec<gs1::Unit> = text.bytes().map(gs1::Unit::Char).collect();
        (!units.is_empty()).ok_or_else(|| Symbology::Code128.invalid("no text"))?;
        (units.len() <= 128).ok_or_else(|| Symbology::Code128.invalid("128 characters at most"))?;
        Ok(Self::linear(Symbology::Code128, false, text.to_owned(), code128::encode(&units)?))
    }

    /// A GS1-128 of an element string such as `(00)106141412345678908`:
    /// Code 128 opening with FNC1, at most 48 characters with separators.
    ///
    /// # Errors
    /// [`Error::InvalidElementString`], or [`Error::InvalidBarcode`] when it
    /// is too long.
    pub fn gs1_128(element_string: &str) -> Result<Self> {
        let mut units = vec![gs1::Unit::Fnc1];
        units.extend(gs1::element_units(element_string)?);
        let characters = units.len() - 1;
        (characters <= 48).ok_or_else(|| {
            Symbology::Code128
                .invalid(format!("GS1-128 holds 48 characters with separators, not {characters}"))
        })?;
        let modules = code128::encode(&units)?;
        Ok(Self::linear(Symbology::Code128, true, element_string.to_owned(), modules))
    }

    fn matrix(
        symbology: Symbology,
        gs1: bool,
        text: &str,
        (columns, rows): (usize, usize),
        modules: Vec<bool>,
    ) -> Self {
        Self { symbology, gs1, text: text.to_owned(), columns, rows, modules }
    }

    const fn linear(symbology: Symbology, gs1: bool, text: String, modules: Vec<bool>) -> Self {
        Self { symbology, gs1, text, columns: modules.len(), rows: 1, modules }
    }

    #[must_use]
    pub const fn symbology(&self) -> Symbology {
        self.symbology
    }

    /// Whether it carries a GS1 element string (its FNC1 mode).
    #[must_use]
    pub const fn is_gs1(&self) -> bool {
        self.gs1
    }

    /// What it encodes: the text or element string given, or a GTIN's
    /// digits with the check digit.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Columns and rows of modules, without the quiet zone; a linear code
    /// is one row.
    #[must_use]
    pub const fn size(&self) -> (usize, usize) {
        (self.columns, self.rows)
    }

    /// Whether the module at column `x`, row `y` is dark; false outside.
    #[must_use]
    pub fn is_dark(&self, x: usize, y: usize) -> bool {
        x < self.columns && self.modules.get(y * self.columns + x).copied().unwrap_or(false)
    }

    /// SVG markup, `module` user units (pixels) a module.
    ///
    /// # Errors
    /// [`Error::InvalidBarcode`] for a module outside 0.01 to 1000.
    pub fn svg(&self, module: f64, look: &Look) -> Result<String> {
        (0.01..=1000.0)
            .contains(&module)
            .ok_or_else(|| self.symbology.invalid("a module of 0.01 to 1000 units"))?;
        Ok(draw::svg(&self.shapes(look.quiet_zone, look.bar_height), module, look))
    }

    /// A PNG, `scale` pixels a module.
    ///
    /// # Errors
    /// [`Error::InvalidBarcode`] for a scale of 0, or an image above 65 536
    /// pixels a side.
    pub fn png(&self, scale: u32, look: &Look) -> Result<Vec<u8>> {
        draw::png(&self.shapes(look.quiet_zone, look.bar_height), scale, look)
            .ok_or_else(|| self.symbology.invalid("a scale of 1 or more, under 65 536 pixels"))
    }

    /// A module's size in points when a document is not told: 0.33 mm for
    /// retail EAN and UPC, 0.495 mm for GS1-128 (logistics labels), 0.35 mm
    /// for other Code 128, 0.7 mm for QR Code and Data Matrix.
    pub(crate) const fn default_module(&self) -> f64 {
        match self.symbology {
            Symbology::Ean13 | Symbology::Ean8 | Symbology::UpcA => 0.935,
            Symbology::Code128 if self.gs1 => 1.4,
            Symbology::Code128 => 1.0,
            Symbology::QrCode | Symbology::DataMatrix => 2.0,
        }
    }

    /// A linear code's bar height in modules when not told: GS1's nominal
    /// 22.85 mm (EAN-13, UPC-A) and 18.23 mm (EAN-8) at 0.33 mm, and for
    /// Code 128 ISO/IEC 15417's 15% of its width, 20 modules at least.
    const fn default_bar_height(&self) -> usize {
        match self.symbology {
            Symbology::Ean13 | Symbology::UpcA => 69,
            Symbology::Ean8 => 55,
            _ => {
                let height = (self.columns * 15).div_ceil(100);
                if height > 20 { height } else { 20 }
            }
        }
    }
}

/// How a barcode is drawn as SVG or PNG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    pub dark: Color,
    /// The background; `None` leaves it transparent.
    pub light: Option<Color>,
    /// Keep the blank margin the symbology needs around it.
    pub quiet_zone: bool,
    /// A linear code's bar height in modules; `None` takes the
    /// symbology's nominal height.
    pub bar_height: Option<u32>,
}

impl Default for Look {
    /// Black on white, with its quiet zone.
    fn default() -> Self {
        Self { dark: Color::BLACK, light: Some(Color::WHITE), quiet_zone: true, bar_height: None }
    }
}
