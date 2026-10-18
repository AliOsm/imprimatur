//! QR Code and Data Matrix, through the `qrcode` and `datamatrix` crates.

use datamatrix::{DataMatrix, SymbolList};
use qrcode::bits::Bits;
use qrcode::{Color, EcLevel, QrCode, QrResult, Version};

use super::Symbology;
use super::gs1::Unit;
use crate::Result;

/// How much of a QR Code may be damaged and still read: about 7%, 15%, 25%
/// or 30% of it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Level {
    Low,
    #[default]
    Medium,
    Quartile,
    High,
}

impl From<Level> for EcLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::Low => Self::L,
            Level::Medium => Self::M,
            Level::Quartile => Self::Q,
            Level::High => Self::H,
        }
    }
}

/// Where FNC1 separates fields, GS1 symbols carry `GS` (ASCII 29), which
/// is also what scanners transmit for it.
const GS: u8 = 0x1D;
/// Digits shorter than this stay in byte mode: switching to numeric mode
/// and back costs more than it saves.
const NUMERIC_RUN: usize = 6;

/// A QR Code's side in modules and its modules, row by row.
pub fn qr(data: &[u8], level: Level) -> Result<(usize, Vec<bool>)> {
    let code = QrCode::with_error_correction_level(data, level.into())
        .map_err(|_| too_long(Symbology::QrCode))?;
    Ok(qr_modules(code))
}

/// A GS1 QR Code (FNC1 in first position) in the smallest version it fits.
pub fn gs1_qr(units: &[Unit], level: Level) -> Result<(usize, Vec<bool>)> {
    let level = EcLevel::from(level);
    let data = bytes(units);
    let bits = (1..=40)
        .find_map(|version| gs1_bits(&data, Version::Normal(version), level).ok())
        .ok_or_else(|| too_long(Symbology::QrCode))?;
    let code = QrCode::with_bits(bits, level).map_err(|_| too_long(Symbology::QrCode))?;
    Ok(qr_modules(code))
}

/// Long runs of digits in numeric mode, the rest in byte mode: alphanumeric
/// mode would read a `%` in the data as a separator.
fn gs1_bits(data: &[u8], version: Version, level: EcLevel) -> QrResult<Bits> {
    let mut bits = Bits::new(version);
    bits.push_fnc1_first_position()?;
    let mut rest = data;
    while !rest.is_empty() {
        let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
        if digits >= NUMERIC_RUN {
            bits.push_numeric_data(&rest[..digits])?;
            rest = &rest[digits..];
            continue;
        }
        let mut end = 0;
        while end < rest.len() {
            let run = rest[end..].iter().take_while(|byte| byte.is_ascii_digit()).count();
            if run >= NUMERIC_RUN {
                break;
            }
            end += run.max(1);
        }
        bits.push_byte_data(&rest[..end])?;
        rest = &rest[end..];
    }
    bits.push_terminator(level)?;
    Ok(bits)
}

/// A square Data Matrix's columns and rows, and its modules row by row.
pub fn data_matrix(text: &str) -> Result<((usize, usize), Vec<bool>)> {
    let code =
        DataMatrix::encode_str(text, square()).map_err(|_| too_long(Symbology::DataMatrix))?;
    Ok(modules(&code))
}

/// A square GS1 `DataMatrix`: FNC1 first, then the data.
pub fn gs1_data_matrix(units: &[Unit]) -> Result<((usize, usize), Vec<bool>)> {
    let code = DataMatrix::encode_gs1(&bytes(units), square())
        .map_err(|_| too_long(Symbology::DataMatrix))?;
    Ok(modules(&code))
}

fn qr_modules(code: QrCode) -> (usize, Vec<bool>) {
    (code.width(), code.into_colors().into_iter().map(|color| color == Color::Dark).collect())
}

fn square() -> SymbolList {
    SymbolList::default().enforce_square()
}

fn modules(code: &DataMatrix) -> ((usize, usize), Vec<bool>) {
    let bitmap = code.bitmap();
    let (columns, rows) = (bitmap.width(), bitmap.height());
    let mut modules = vec![false; columns * rows];
    for (x, y) in bitmap.pixels() {
        modules[y * columns + x] = true;
    }
    ((columns, rows), modules)
}

fn bytes(units: &[Unit]) -> Vec<u8> {
    units
        .iter()
        .map(|unit| match unit {
            Unit::Char(byte) => *byte,
            Unit::Fnc1 => GS,
        })
        .collect()
}

fn too_long(symbology: Symbology) -> crate::Error {
    symbology.invalid("too much data for the largest symbol")
}
