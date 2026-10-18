use std::assert_matches;

use super::*;

/// A symbol as its size and the CRC-32 of its modules, one byte each, row
/// by row.
fn fingerprint(barcode: &Barcode) -> (usize, usize, u32) {
    let (columns, rows) = barcode.size();
    let modules: Vec<u8> = (0..rows)
        .flat_map(|y| (0..columns).map(move |x| u8::from(barcode.is_dark(x, y))))
        .collect();
    (columns, rows, crc32fast::hash(&modules))
}

/// What a scanner transmits for a GS1 element string: no parentheses, GS
/// for each separator.
fn transmitted(element_string: &str) -> Vec<u8> {
    gs1::element_units(element_string)
        .unwrap()
        .into_iter()
        .map(|unit| match unit {
            gs1::Unit::Char(byte) => byte,
            gs1::Unit::Fnc1 => 0x1D,
        })
        .collect()
}

/// Every symbology by the keyword the golden and error-message tables use.
fn decode(kind: &str, data: &str) -> Result<Barcode> {
    match kind {
        "ean13" => Barcode::ean13(data),
        "ean8" => Barcode::ean8(data),
        "upca" => Barcode::upc_a(data),
        "code128" => Barcode::code128(data),
        "gs1_128" => Barcode::gs1_128(data),
        _ => Barcode::gs1_qr(data, Level::Medium),
    }
}

/// The data, read back by the `datamatrix` crate's own decoder.
fn decode_matrix(barcode: &Barcode) -> Vec<u8> {
    let (columns, rows) = barcode.size();
    let modules: Vec<bool> =
        (0..rows).flat_map(|y| (0..columns).map(move |x| barcode.is_dark(x, y))).collect();
    datamatrix::DataMatrix::decode(&modules, columns).unwrap()
}

/// The symbols our own code lays out (EAN, UPC, Code 128 and GS1-128, and
/// GS1 QR Code's bits), each read back by rxing (the Rust port of `ZXing`) to the
/// text or element string given before its fingerprint was taken: Code 128
/// across code sets A, B and C and every value they reach, EAN-13 for every
/// leading digit (its parity table), GS1 separators and FNC1 modes.
#[test]
fn draws_symbols_scanners_read() {
    const GOLDEN: &[(&str, &str, (usize, usize, u32))] = &[
        ("ean13", "012345678901", (95, 1, 3_515_918_774)),
        ("ean13", "112345678901", (95, 1, 3_622_115_097)),
        ("ean13", "212345678901", (95, 1, 2_150_112_367)),
        ("ean13", "312345678901", (95, 1, 2_116_414_761)),
        ("ean13", "412345678901", (95, 1, 3_437_724_742)),
        ("ean13", "512345678901", (95, 1, 1_230_145_168)),
        ("ean13", "612345678901", (95, 1, 380_536_501)),
        ("ean13", "712345678901", (95, 1, 1_793_107_486)),
        ("ean13", "812345678901", (95, 1, 2_468_054_115)),
        ("ean13", "912345678901", (95, 1, 3_076_487_126)),
        ("ean8", "9638507", (67, 1, 3_550_630_461)),
        ("upca", "03600029145", (95, 1, 1_469_614_652)),
        (
            "code128",
            concat!(
                " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ",
                "[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~",
            ),
            (1047, 1, 1_957_065_161),
        ),
        (
            "code128",
            concat!(
                "A\0\u{1}\u{2}\u{3}\u{4}\u{5}\u{6}\u{7}\u{8}\t\n\u{b}\u{c}\r\u{e}\u{f}",
                "\u{10}\u{11}\u{12}\u{13}\u{14}\u{15}\u{16}\u{17}\u{18}\u{19}\u{1a}\u{1b}",
                "\u{1c}\u{1d}\u{1e}\u{1f}a",
            ),
            (420, 1, 1_020_174_387),
        ),
        (
            "code128",
            concat!(
                "00010203040506070809101112131415161718192021222324",
                "25262728293031323334353637383940414243444546474849",
            ),
            (585, 1, 3_757_078_539),
        ),
        (
            "code128",
            concat!(
                "50515253545556575859606162636465666768697071727374",
                "75767778798081828384858687888990919293949596979899",
            ),
            (585, 1, 649_292_837),
        ),
        ("code128", "12", (46, 1, 708_961_080)),
        ("code128", "123", (68, 1, 2_373_092_336)),
        ("code128", "12345", (79, 1, 3_273_840_386)),
        ("code128", "AB123456CD", (134, 1, 2_295_850_879)),
        ("code128", "AB1234567cd", (145, 1, 2_089_954_864)),
        ("code128", "x\u{1}y\u{2}z", (134, 1, 515_487_829)),
        ("code128", "CS-ORDER-000042", (178, 1, 1_564_437_952)),
        ("gs1_128", "(01)09506000134352", (134, 1, 3_719_561_895)),
        ("gs1_128", "(01)09506000134352(17)251231(10)AB12", (244, 1, 2_834_305_618)),
        ("gs1_128", "(10)AB12(21)XYZ", (178, 1, 2_163_671_379)),
        ("gs1_128", "(21)A%B(3103)000189", (178, 1, 1_841_721_631)),
        ("gs1_128", "(00)106141412345678908", (156, 1, 3_822_156_570)),
        ("gs1_qr", "(01)09506000134352", (21, 21, 2_669_143_811)),
        ("gs1_qr", "(01)09506000134352(17)251231(10)AB12", (25, 25, 1_474_166_289)),
        ("gs1_qr", "(10)AB12(21)XYZ", (21, 21, 692_884_921)),
        // `%` goes in byte mode: in alphanumeric mode it would read as GS.
        ("gs1_qr", "(21)A%B(3103)000189", (21, 21, 2_876_880_871)),
        ("gs1_qr", "(00)106141412345678908", (21, 21, 1_568_866_979)),
    ];
    for (kind, data, expected) in GOLDEN {
        let code = decode(kind, data).unwrap();
        assert_eq!(fingerprint(&code), *expected, "{kind} {data:?}");
    }
}

#[test]
fn spells_the_gs1_example_ean13() {
    let ean = Barcode::ean13("950600013435").unwrap();
    assert_eq!(ean.text(), "9506000134352", "the check digit, added");
    let spelled: String = (0..95).map(|x| if ean.is_dark(x, 0) { '1' } else { '0' }).collect();
    // Guard, 506000 as LGGLGL (the 9), center, 134352 in set C, guard.
    assert_eq!(
        spelled,
        "101\
         0110001\
         0100111\
         0000101\
         0001101\
         0100111\
         0001101\
         01010\
         1100110\
         1000010\
         1011100\
         1000010\
         1001110\
         1101100\
         101"
    );
    assert_eq!(Barcode::upc_a("036000291452").unwrap().text(), "036000291452");
    assert_eq!(Barcode::ean8("96385074").unwrap().text(), "96385074");
}

#[test]
fn data_matrix_reads_back_square() {
    const LATIN1: &[(&str, &[u8])] =
        &[("Hello", b"Hello"), ("CS-2026-0001", b"CS-2026-0001"), ("Café", b"Caf\xE9")];
    for (text, bytes) in LATIN1 {
        let code = Barcode::data_matrix(text).unwrap();
        let (columns, rows) = code.size();
        assert_eq!(columns, rows, "square: {text:?}");
        assert_eq!(decode_matrix(&code), *bytes, "{text:?}");
    }
    let long = "A1".repeat(100);
    assert_eq!(decode_matrix(&Barcode::data_matrix(&long).unwrap()), long.as_bytes());
    // Beyond Latin-1 it goes as UTF-8 behind an ECI designator, which this
    // decoder does not read: the encoding is the crate's.
    let (columns, rows) = Barcode::data_matrix("Ünïcödé ✓").unwrap().size();
    assert_eq!(columns, rows);

    for element_string in ["(01)09506000134352(17)251231(10)AB12", "(10)AB12(21)XYZ"] {
        let code = Barcode::gs1_data_matrix(element_string).unwrap();
        assert!(code.is_gs1());
        assert_eq!(decode_matrix(&code), transmitted(element_string), "{element_string}");
    }
}

#[test]
fn qr_codes_take_the_smallest_version_for_their_level() {
    const CASES: &[(&str, Level, usize)] = &[
        ("HELLO WORLD 12345", Level::Low, 21),
        ("HELLO WORLD 12345", Level::High, 25),
        ("https://id.gs1.org/01/09506000134352/10/AB12", Level::Medium, 29),
        ("Café crème, 東京 ✓", Level::Quartile, 29),
    ];
    for (text, level, side) in CASES {
        let code = Barcode::qr(text, *level).unwrap();
        assert_eq!(code.size(), (*side, *side), "{text:?} at {level:?}");
        assert_eq!(code.text(), *text);
        assert!(!code.is_gs1());
        // A finder pattern in three corners: a dark ring, a light one, a
        // dark three by three center.
        for (left, top) in [(0, 0), (side - 7, 0), (0, side - 7)] {
            let ring = (0..7).all(|at| {
                code.is_dark(left + at, top)
                    && code.is_dark(left + at, top + 6)
                    && code.is_dark(left, top + at)
                    && code.is_dark(left + 6, top + at)
            });
            let center = (2..5).all(|y| (2..5).all(|x| code.is_dark(left + x, top + y)));
            assert!(ring && center && !code.is_dark(left + 1, top + 1), "{text:?}");
        }
    }
    assert!(!Barcode::qr("A", Level::Low).unwrap().is_dark(21, 0), "outside");
}

#[test]
fn rejects_what_a_symbology_cannot_hold() {
    const CASES: &[(&str, &str)] = &[
        (
            "ean13:12345",
            "invalid EAN-13 barcode: it takes 12 digits, or 13 with the check digit, not 5",
        ),
        ("ean13:9506000134353", "invalid EAN-13 barcode: 9506000134353 has check digit 3, not 2"),
        ("ean13:95060001343a", "invalid EAN-13 barcode: \"95060001343a\" is not all digits"),
        ("ean8:96385075", "invalid EAN-8 barcode: 96385075 has check digit 5, not 4"),
        (
            "upca:0360002914",
            "invalid UPC-A barcode: it takes 11 digits, or 12 with the check digit, not 10",
        ),
        ("code128:", "invalid Code 128 barcode: no text"),
        ("code128:café", "invalid Code 128 barcode: it holds ASCII only, not byte 0xC3"),
    ];
    let error = |result: Result<Barcode>| result.unwrap_err().to_string();
    for (case, expected) in CASES {
        let (kind, data) = case.split_once(':').unwrap();
        assert_eq!(error(decode(kind, data)), *expected, "case: {case:?}");
    }
    assert!(error(Barcode::code128(&"A".repeat(129))).contains("128 characters at most"));
    let long = format!("(01)09506000134352(10){}(21){}", "A".repeat(20), "B".repeat(20));
    assert!(error(Barcode::gs1_128(&long)).contains("48 characters"));
    assert_matches!(Barcode::qr(&"x".repeat(3000), Level::High), Err(Error::InvalidBarcode { .. }));
    assert_matches!(Barcode::gs1_qr("(01)1", Level::Low), Err(Error::InvalidElementString(_)));
}

#[test]
fn shapes_keep_the_quiet_zone_and_merge_runs() {
    let ean = Barcode::ean13("9506000134352").unwrap();
    let shapes = ean.shapes(true, None);
    assert_eq!((shapes.width, shapes.height), (11 + 95 + 7, 69));
    assert_eq!(shapes.rects[0], [11, 0, 1, 69], "the left guard's first bar");
    assert_eq!(shapes.rects.len(), 30, "EAN-13 has 30 bars");
    let bare = ean.shapes(false, Some(10));
    assert_eq!((bare.width, bare.height, bare.rects[0]), (95, 10, [0, 0, 1, 10]));

    let qr = Barcode::qr("A", Level::Low).unwrap();
    let shapes = qr.shapes(true, None);
    assert_eq!((shapes.width, shapes.height), (21 + 8, 21 + 8));
    assert_eq!(shapes.rects[0], [4, 4, 7, 1], "the finder pattern's top row");
}

#[test]
fn draws_svg_and_png() {
    let qr = Barcode::qr("A", Level::Low).unwrap();
    let svg = qr.svg(4.0, &Look::default()).unwrap();
    assert!(svg.starts_with(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"116\" height=\"116\" \
         viewBox=\"0 0 29 29\" shape-rendering=\"crispEdges\">\
         <rect width=\"29\" height=\"29\" fill=\"#FFFFFF\"/><path fill=\"#000000\" d=\"M4 4h7v1h-7z"
    ));
    let look =
        Look { dark: Color::rgb(0x1F_3864), light: None, quiet_zone: false, bar_height: None };
    let bare = qr.svg(2.5, &look).unwrap();
    assert!(bare.contains("width=\"52.5\"") && bare.contains("fill=\"#1F3864\""));
    assert!(!bare.contains("<rect"), "no background: {bare}");

    let png = qr.png(4, &Look::default()).unwrap();
    let picture = crate::doc::Picture::new(png).unwrap();
    assert_eq!(picture.pixels, (116, 116));
    let transparent = qr.png(1, &look).unwrap();
    assert!(transparent.windows(4).any(|window| window == b"tRNS"));
    assert!(!qr.png(4, &Look::default()).unwrap().windows(4).any(|window| window == b"tRNS"));

    assert_matches!(qr.svg(0.0, &Look::default()), Err(Error::InvalidBarcode { .. }));
    assert_matches!(qr.png(0, &Look::default()), Err(Error::InvalidBarcode { .. }));
    assert_matches!(qr.png(4000, &Look::default()), Err(Error::InvalidBarcode { .. }));
}
