//! A symbol as rectangles in module units, and those as SVG or PNG; PDF
//! draws the same rectangles.

use std::fmt::Write as _;

use super::{Barcode, Look};
use crate::read::{PngColor, png_with_transparency};

/// The largest side, in pixels, a PNG may have, as pictures.
const MAX_PIXELS: usize = 1 << 16;

/// A symbol in module units: its whole width and height, quiet zone
/// included, and its dark rectangles `[x, y, width, height]` from the top
/// left. Each row's runs are one rectangle; a linear code's bars span its
/// height.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shapes {
    pub width: usize,
    pub height: usize,
    pub rects: Vec<[usize; 4]>,
}

impl Shapes {
    /// Its width and height in modules, for scaling.
    #[expect(clippy::cast_precision_loss, reason = "a symbol is far below 2**52 modules")]
    pub const fn extent(&self) -> (f64, f64) {
        (self.width as f64, self.height as f64)
    }
}

impl Barcode {
    pub(crate) fn shapes(&self, quiet_zone: bool, bar_height: Option<u32>) -> Shapes {
        let (left, right, edge) = if quiet_zone { self.symbology.quiet_zone() } else { (0, 0, 0) };
        let row_height = if self.symbology.is_linear() {
            bar_height.map_or_else(|| self.default_bar_height(), |bars| bars.max(1) as usize)
        } else {
            1
        };
        let mut rects = Vec::new();
        for (y, row) in self.modules.chunks(self.columns).enumerate() {
            let mut x = 0;
            while x < row.len() {
                let run = row[x..].iter().take_while(|&&dark| dark == row[x]).count();
                if row[x] {
                    rects.push([left + x, edge + y * row_height, run, row_height]);
                }
                x += run;
            }
        }
        Shapes {
            width: left + self.columns + right,
            height: 2 * edge + self.rows * row_height,
            rects,
        }
    }
}

/// `module` user units a module, the rectangles as one path.
pub fn svg(shapes: &Shapes, module: f64, look: &Look) -> String {
    let (width, height) = (shapes.width, shapes.height);
    let (columns, rows) = shapes.extent();
    let (pixels_wide, pixels_high) = (number(columns * module), number(rows * module));
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{pixels_wide}\" \
         height=\"{pixels_high}\" viewBox=\"0 0 {width} {height}\" \
         shape-rendering=\"crispEdges\">"
    );
    if let Some(light) = look.light {
        let fill = light.hex();
        // write! into a String cannot fail.
        let _ = write!(out, "<rect width=\"{width}\" height=\"{height}\" fill=\"{fill}\"/>");
    }
    // write! into a String cannot fail.
    let _ = write!(out, "<path fill=\"{}\" d=\"", look.dark.hex());
    for [x, y, run, rows] in &shapes.rects {
        // write! into a String cannot fail.
        let _ = write!(out, "M{x} {y}h{run}v{rows}h-{run}z");
    }
    out.push_str("\"/></svg>");
    out
}

/// `scale` pixels a module, one bit a pixel: palette entry 0 the light
/// color (transparent without one), 1 the dark. `None` past
/// [`MAX_PIXELS`] or at a scale of 0.
pub fn png(shapes: &Shapes, scale: u32, look: &Look) -> Option<Vec<u8>> {
    let (pixels_wide, pixels_high, rows) = bits(shapes, scale)?;
    let light = look.light.unwrap_or(crate::style::Color::WHITE).bytes();
    let palette = [light, look.dark.bytes()].concat();
    let transparency: &[u8] = if look.light.is_none() { &[0] } else { &[] };
    let (width, height) = (u32::try_from(pixels_wide).ok()?, u32::try_from(pixels_high).ok()?);
    Some(png_with_transparency(
        width,
        height,
        PngColor::Palette,
        1,
        Some(&palette),
        transparency,
        &rows,
    ))
}

/// The pixels, one bit each, rows starting on a byte: width, height, rows.
fn bits(shapes: &Shapes, scale: u32) -> Option<(usize, usize, Vec<u8>)> {
    let scale = usize::try_from(scale).ok().filter(|&scale| scale > 0)?;
    let (width, height) = (shapes.width * scale, shapes.height * scale);
    (width <= MAX_PIXELS && height <= MAX_PIXELS).then_some(())?;
    let stride = width.div_ceil(8);
    let mut rows = vec![0_u8; stride * height];
    for &[x, y, run, run_rows] in &shapes.rects {
        for row in y * scale..(y + run_rows) * scale {
            for pixel in x * scale..(x + run) * scale {
                rows[row * stride + pixel / 8] |= 0x80 >> (pixel % 8);
            }
        }
    }
    Some((width, height, rows))
}

/// Up to three decimals, without trailing zeros.
fn number(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}
