//! Pictures as inline drawings, their files in `word/media`.

use crate::doc::Picture;
use crate::doc::picture::Kind;
use crate::xml::XmlWriter;

pub const WP_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
pub const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const PIC_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
/// English Metric Units a point, as drawings measure.
const EMU_PER_POINT: f64 = 12_700.0;

/// Its file's extension.
pub const fn extension(picture: &Picture) -> &'static str {
    match picture.kind {
        Kind::Png(_) => "png",
        Kind::Jpeg { .. } => "jpeg",
    }
}

/// Its part in `word/media`, by its number from 1.
pub fn file_name(picture: &Picture, number: usize) -> String {
    format!("media/image{number}.{}", extension(picture))
}

/// A paragraph holding picture `number`, its file `relationship`, at its
/// size within `room`.
pub fn paragraph(
    xml: &mut XmlWriter,
    picture: &Picture,
    room: (f64, f64),
    number: usize,
    relationship: &str,
) {
    let (width, height) = picture.size(room);
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "under 2**31")]
    let emu = |points: f64| ((points * EMU_PER_POINT).round() as u64).to_string();
    let (cx, cy, id, name) =
        (emu(width), emu(height), number.to_string(), format!("Picture {number}"));
    let extent = [("cx", cx.as_str()), ("cy", cy.as_str())];
    super::open_paragraph(xml, None, false, picture.align);
    xml.open("w:r", &[]);
    xml.open("w:drawing", &[]);
    xml.open("wp:inline", &[("distT", "0"), ("distB", "0"), ("distL", "0"), ("distR", "0")]);
    xml.empty("wp:extent", &extent);
    xml.empty("wp:docPr", &[("id", &id), ("name", &name)]);
    xml.open("a:graphic", &[]);
    xml.open("a:graphicData", &[("uri", PIC_NS)]);
    xml.open("pic:pic", &[]);
    xml.open("pic:nvPicPr", &[]);
    xml.empty("pic:cNvPr", &[("id", &id), ("name", &name)]);
    xml.empty("pic:cNvPicPr", &[]);
    xml.close("pic:nvPicPr");
    xml.open("pic:blipFill", &[]);
    xml.empty("a:blip", &[("r:embed", relationship)]);
    xml.open("a:stretch", &[]);
    xml.empty("a:fillRect", &[]);
    xml.close("a:stretch");
    xml.close("pic:blipFill");
    xml.open("pic:spPr", &[]);
    xml.open("a:xfrm", &[]);
    xml.empty("a:off", &[("x", "0"), ("y", "0")]);
    xml.empty("a:ext", &extent);
    xml.close("a:xfrm");
    xml.open("a:prstGeom", &[("prst", "rect")]);
    xml.empty("a:avLst", &[]);
    xml.close("a:prstGeom");
    xml.close("pic:spPr");
    for tag in ["pic:pic", "a:graphicData", "a:graphic", "wp:inline", "w:drawing", "w:r", "w:p"] {
        xml.close(tag);
    }
}
