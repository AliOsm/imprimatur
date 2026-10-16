//! Pictures as image `XObject`s: a JPEG as it is; a PNG's compressed pixels
//! as they are, PDF reading them with the PNG predictor, unless an alpha
//! channel needs them decoded into a soft mask.

use std::borrow::Cow;
use std::fmt::Write as _;

use crate::doc::Picture;
use crate::doc::picture::{Kind, Png};
use crate::{Error, Result};

/// The most pixels a PNG with alpha may have: it is decoded in memory.
const MAX_DECODED: u64 = 1 << 26;

/// An image stream: its dictionary after `/Length`, and its data as stored.
pub struct Stream<'a> {
    pub dictionary: String,
    pub data: Cow<'a, [u8]>,
}

/// Whether the picture has a soft mask, written right after it.
pub const fn has_mask(picture: &Picture) -> bool {
    match &picture.kind {
        Kind::Png(png) => alpha(png),
        Kind::Jpeg { .. } => false,
    }
}

/// The image, then its soft mask when it has one; `mask` is the mask's
/// object number.
pub fn streams(picture: &Picture, mask: usize) -> Result<Vec<Stream<'_>>> {
    let (width, height) = picture.pixels;
    let head = format!("/Type /XObject /Subtype /Image /Width {width} /Height {height}");
    match &picture.kind {
        Kind::Jpeg { components, inverted } => {
            let space = ["DeviceGray", "", "DeviceRGB", "DeviceCMYK"][usize::from(*components) - 1];
            let decode = if *inverted { " /Decode [1 0 1 0 1 0 1 0]" } else { "" };
            let dictionary = format!(
                "{head} /ColorSpace /{space} /BitsPerComponent 8 /Filter /DCTDecode{decode}"
            );
            Ok(vec![Stream { dictionary, data: Cow::Borrowed(&picture.data) }])
        }
        Kind::Png(png) if alpha(png) => decoded(png, (width, height), &head, mask),
        Kind::Png(png) => Ok(vec![predicted(png, width, &head)]),
    }
}

/// Alpha that only a soft mask carries: an alpha channel, or a palette's.
const fn alpha(png: &Png) -> bool {
    matches!(png.color, 4 | 6) || (png.color == 3 && !png.transparency.is_empty())
}

/// The compressed pixels as stored, read through the PNG predictor; a
/// transparent color becomes a color key mask.
fn predicted<'a>(png: &'a Png, width: u32, head: &str) -> Stream<'a> {
    let depth = png.depth;
    let (colors, space) = match png.color {
        0 => (1, "/DeviceGray".to_owned()),
        2 => (3, "/DeviceRGB".to_owned()),
        _ => (1, indexed(&png.palette)),
    };
    let mut dictionary = format!(
        "{head} /ColorSpace {space} /BitsPerComponent {depth} /Filter /FlateDecode \
         /DecodeParms << /Predictor 15 /Colors {colors} /BitsPerComponent {depth} \
         /Columns {width} >>"
    );
    if !png.transparency.is_empty() {
        dictionary.push_str(" /Mask [");
        for key in png.transparency.as_chunks::<2>().0 {
            let value = u16::from_be_bytes(*key);
            // Writing to a String cannot fail.
            let _ = write!(dictionary, "{value} {value} ");
        }
        dictionary.push(']');
    }
    Stream { dictionary, data: Cow::Borrowed(&png.compressed) }
}

/// `[/Indexed /DeviceRGB n <palette>]`.
fn indexed(palette: &[u8]) -> String {
    let mut space = format!("[/Indexed /DeviceRGB {} <", palette.len() / 3 - 1);
    for byte in palette {
        // Writing to a String cannot fail.
        let _ = write!(space, "{byte:02X}");
    }
    space.push_str(">]");
    space
}

/// The pixels decoded into 8-bit color and alpha, both compressed again.
fn decoded<'a>(
    png: &Png,
    (width, height): (u32, u32),
    head: &str,
    mask: usize,
) -> Result<Vec<Stream<'a>>> {
    let damaged = || Error::InvalidImage("a damaged PNG");
    (u64::from(width) * u64::from(height) <= MAX_DECODED)
        .ok_or(Error::InvalidImage("a PNG with alpha of at most 67 million pixels"))?;
    let channels = match png.color {
        4 => 2,
        6 => 4,
        _ => 1,
    };
    let bits = channels * usize::from(png.depth);
    let (width, height) = (width as usize, height as usize);
    let stride = (width * bits).div_ceil(8);
    let filtered = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
        &png.compressed,
        height * (stride + 1),
    )
    .map_err(|_| damaged())?;
    let pixels = unfilter(&filtered, stride, bits.div_ceil(8), height).ok_or_else(damaged)?;
    let gray = png.color == 4;
    let (mut color, mut alpha) =
        (Vec::with_capacity(width * height * 3), Vec::with_capacity(width * height));
    for row in pixels.chunks_exact(stride) {
        for x in 0..width {
            if png.color == 3 {
                let index = usize::from(sample(row, x, png.depth));
                let rgb = png.palette.get(index * 3..index * 3 + 3).ok_or_else(damaged)?;
                color.extend_from_slice(rgb);
                alpha.push(png.transparency.get(index).copied().unwrap_or(u8::MAX));
            } else {
                // 16-bit samples keep their high byte.
                let step = usize::from(png.depth / 8);
                let at = |channel: usize| row[(x * channels + channel) * step];
                color.extend((0..channels - 1).map(at));
                alpha.push(at(channels - 1));
            }
        }
    }
    let space = if gray { "DeviceGray" } else { "DeviceRGB" };
    let compress = |data: &[u8]| Cow::Owned(miniz_oxide::deflate::compress_to_vec_zlib(data, 6));
    Ok(vec![
        Stream {
            dictionary: format!(
                "{head} /ColorSpace /{space} /BitsPerComponent 8 /Filter /FlateDecode \
                 /SMask {mask} 0 R"
            ),
            data: compress(&color),
        },
        Stream {
            dictionary: format!(
                "{head} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode"
            ),
            data: compress(&alpha),
        },
    ])
}

/// The `x`th sample of a row of `depth`-bit samples.
fn sample(row: &[u8], x: usize, depth: u8) -> u8 {
    let depth = usize::from(depth);
    let (byte, shift) = (x * depth / 8, 8 - depth - x * depth % 8);
    (row[byte] >> shift) & (u8::MAX >> (8 - depth))
}

/// PNG's filters undone (ISO/IEC 15948, 9): rows of `stride` bytes, `bpp`
/// bytes a pixel.
fn unfilter(data: &[u8], stride: usize, bpp: usize, rows: usize) -> Option<Vec<u8>> {
    let mut out = vec![0_u8; stride * rows];
    for row in 0..rows {
        let line = data.get(row * (stride + 1)..(row + 1) * (stride + 1))?;
        let (done, rest) = out.split_at_mut(row * stride);
        let previous = row.checked_sub(1).map(|_| &done[done.len() - stride..]);
        let current = &mut rest[..stride];
        for at in 0..stride {
            let left = at.checked_sub(bpp).map_or(0, |at| current[at]);
            let up = previous.map_or(0, |previous| previous[at]);
            let up_left = at.checked_sub(bpp).and_then(|at| previous.map(|previous| previous[at]));
            let predicted = match line[0] {
                0 => 0,
                1 => left,
                2 => up,
                3 => left.midpoint(up),
                4 => paeth(left, up, up_left.unwrap_or(0)),
                _ => return None,
            };
            current[at] = line[at + 1].wrapping_add(predicted);
        }
    }
    Some(out)
}

/// The neighbor nearest `left + up - up_left`.
fn paeth(left: u8, up: u8, up_left: u8) -> u8 {
    let estimate = i16::from(left) + i16::from(up) - i16::from(up_left);
    let distance = |byte: u8| (estimate - i16::from(byte)).abs();
    if distance(left) <= distance(up) && distance(left) <= distance(up_left) {
        left
    } else if distance(up) <= distance(up_left) {
        up
    } else {
        up_left
    }
}
