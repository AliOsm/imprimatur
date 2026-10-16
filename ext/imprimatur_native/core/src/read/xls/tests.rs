use std::assert_matches;

use crate::Error;
use crate::read::{Content, Limits, Sheet, Value, content};
use crate::test_support::ole_of;
use crate::xlsx::DateTime;

fn record(kind: u16, body: &[u8]) -> Vec<u8> {
    let length = u16::try_from(body.len()).unwrap();
    [&kind.to_le_bytes()[..], &length.to_le_bytes(), body].concat()
}

fn bof(version: u16, kind: u16) -> Vec<u8> {
    record(0x0809, &[&version.to_le_bytes()[..], &kind.to_le_bytes(), &[0; 12]].concat())
}

/// A cell record's position and format, then its rest.
fn cell(kind: u16, row: u16, column: u16, xf: u16, rest: &[u8]) -> Vec<u8> {
    record(kind, &[&row.to_le_bytes()[..], &column.to_le_bytes(), &xf.to_le_bytes(), rest].concat())
}

/// An `XLUnicodeString` (2-byte count) or a BIFF5 byte string.
fn text(chars: &[u8], count: usize, flags: Option<u8>) -> Vec<u8> {
    let count = u16::try_from(count).unwrap().to_le_bytes();
    [&count[..], flags.as_slice(), chars].concat()
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// A workbook: its globals, then each sheet (name, visibility, kind,
/// records), its BOUNDSHEET offsets filled in.
fn workbook(version: u16, globals: &[Vec<u8>], sheets: &[(&[u8], u8, u8, Vec<u8>)]) -> Vec<u8> {
    let listed = |offsets: &[u32]| -> Vec<u8> {
        sheets
            .iter()
            .zip(offsets)
            .flat_map(|((name, state, kind, _), offset)| {
                let body = [&offset.to_le_bytes()[..], &[*state, *kind], name].concat();
                record(0x0085, &body)
            })
            .collect()
    };
    let head = [bof(version, 5), globals.concat()].concat();
    let length = head.len() + listed(&vec![0; sheets.len()]).len() + 4;
    let mut offsets = Vec::new();
    let mut at = length;
    for (_, _, _, records) in sheets {
        offsets.push(u32::try_from(at).unwrap());
        at += records.len();
    }
    let mut stream = [head, listed(&offsets), record(0x000A, &[])].concat();
    for (_, _, _, records) in sheets {
        stream.extend_from_slice(records);
    }
    stream
}

fn sheets(stream: &[u8]) -> Result<Vec<Sheet>, Error> {
    match content(&ole_of("Workbook", stream), &Limits::default())? {
        Content::Xls(sheets) => Ok(sheets),
        other => panic!("{other:?}"),
    }
}

/// Every cell: row, column, text and value.
fn cells(sheet: &Sheet) -> Vec<(u32, u32, String, Value)> {
    sheet
        .cells
        .cells()
        .iter()
        .map(|c| (c.row, c.column, sheet.cells.text(c).to_owned(), c.value))
        .collect()
}

fn rk_integer(value: i32, hundredths: bool) -> Vec<u8> {
    ((value.cast_unsigned() << 2) | 2 | u32::from(hundredths)).to_le_bytes().to_vec()
}

/// The global records of a BIFF8 workbook: a date format and four cell
/// formats (General, that date, a percentage, a date and time), and shared
/// strings: Latin-1, UTF-16, one whose last characters move to a CONTINUE
/// record as UTF-16, and a rich one whose run follows its text.
fn biff8_globals() -> Vec<Vec<u8>> {
    let format = |id: u16, code: &str| {
        record(
            0x041E,
            &[&id.to_le_bytes()[..], &text(code.as_bytes(), code.len(), Some(0))].concat(),
        )
    };
    let xf = |id: u16| record(0x00E0, &[&[0, 0][..], &id.to_le_bytes(), &[0; 16]].concat());
    let sst = [
        &5_u32.to_le_bytes()[..],
        &4_u32.to_le_bytes(),
        &[6, 0, 0],
        b"R\xE9gion",
        &[4, 0, 1],
        &utf16("שלום"),
        &[10, 0, 0],
        b"abcde",
    ]
    .concat();
    let continued = [&[1][..], &utf16("fghij"), &[4, 0, 8, 1, 0], b"bold", &[0; 4]].concat();
    vec![
        record(0x0022, &[0, 0]),
        format(164, "yyyy-mm-dd"),
        xf(0),
        xf(164),
        xf(9),
        xf(22),
        record(0x00FC, &sst),
        record(0x003C, &continued),
    ]
}

#[test]
fn reads_biff8_workbooks() {
    let globals = biff8_globals();
    let datetime = (45_000.5_f64.to_bits() >> 32) as u32;
    let data = [
        bof(0x0600, 0x10),
        cell(0x00FD, 0, 0, 0, &0_u32.to_le_bytes()),
        cell(0x00FD, 0, 1, 0, &1_u32.to_le_bytes()),
        cell(0x00FD, 0, 2, 0, &2_u32.to_le_bytes()),
        cell(0x00FD, 0, 3, 0, &3_u32.to_le_bytes()),
        cell(0x0203, 1, 0, 0, &1250.5_f64.to_le_bytes()),
        cell(0x027E, 1, 1, 0, &rk_integer(42, false)),
        cell(0x027E, 1, 2, 2, &rk_integer(25, true)),
        record(
            0x00BD,
            &[
                &2_u16.to_le_bytes()[..],
                &[0, 0, 1, 0],
                &rk_integer(45_000, false),
                &[3, 0],
                &datetime.to_le_bytes(),
                &[1, 0],
            ]
            .concat(),
        ),
        cell(0x0205, 3, 0, 0, &[1, 0]),
        cell(0x0205, 3, 1, 0, &[0x2A, 1]),
        cell(0x0006, 4, 0, 0, &[&3.0_f64.to_le_bytes()[..], &[0; 6]].concat()),
        cell(0x0006, 4, 1, 0, &[&[0, 0, 0, 0, 0, 0, 0xFF, 0xFF][..], &[0; 6]].concat()),
        record(0x0207, &text(b"total", 5, Some(0))),
        cell(0x0006, 4, 2, 0, &[&[1, 0, 0, 0, 0, 0, 0xFF, 0xFF][..], &[0; 6]].concat()),
        record(0x000A, &[]),
    ]
    .concat();
    let chart = [bof(0x0600, 0x20), record(0x000A, &[])].concat();
    let hidden = [
        bof(0x0600, 0x10),
        cell(0x0204, 0, 0, 0, &text(b"inline", 6, Some(0))),
        record(0x000A, &[]),
    ]
    .concat();
    let stream = workbook(
        0x0600,
        &globals,
        &[
            (b"\x04\x00Data", 0, 0, data),
            (b"\x06\x00Chart1", 0, 2, chart),
            (b"\x06\x00Hidden", 1, 0, hidden),
        ],
    );
    let read = sheets(&stream).unwrap();
    let names: Vec<(&str, bool)> =
        read.iter().map(|sheet| (sheet.name.as_str(), sheet.hidden)).collect();
    assert_eq!(names, [("Data", false), ("Hidden", true)], "the chart sheet is left out");
    let day = DateTime::date(2023, 3, 15).unwrap();
    let text = |value: &str| value.to_owned();
    assert_eq!(
        cells(&read[0]),
        [
            (0, 0, text("Région"), Value::Text),
            (0, 1, text("שלום"), Value::Text),
            (0, 2, text("abcdefghij"), Value::Text),
            (0, 3, text("bold"), Value::Text),
            (1, 0, text("1250.5"), Value::Number(1250.5)),
            (1, 1, text("42"), Value::Number(42.0)),
            (1, 2, text("25%"), Value::Number(0.25)),
            (2, 0, text("2023-03-15"), Value::Date(day)),
            (2, 1, text("2023-03-15 12:00:00"), Value::Date(day.at(12, 0, 0, 0).unwrap())),
            (3, 0, text("TRUE"), Value::Bool(true)),
            (3, 1, text("#N/A"), Value::Error),
            (4, 0, text("3"), Value::Number(3.0)),
            (4, 1, text("total"), Value::Text),
            (4, 2, text("FALSE"), Value::Bool(false)),
        ]
    );
    assert_eq!(cells(&read[1]), [(0, 0, text("inline"), Value::Text)]);
}

#[test]
fn reads_excel_95_workbooks_in_windows_1252() {
    let sheet = [
        bof(0x0500, 0x10),
        cell(0x0204, 0, 0, 0, &text(b"Prix 5\x80", 7, None)),
        record(0x000A, &[]),
    ]
    .concat();
    let stream = workbook(
        0x0500,
        &[record(0x0042, &1252_u16.to_le_bytes())],
        &[(b"\x07Feuille", 0, 0, sheet)],
    );
    let read = sheets(&stream).unwrap();
    assert_eq!(read[0].name, "Feuille");
    assert_eq!(cells(&read[0]), [(0, 0, "Prix 5€".to_owned(), Value::Text)]);
}

#[test]
fn refuses_what_it_cannot_read() {
    let cyrillic = workbook(0x0500, &[record(0x0042, &1251_u16.to_le_bytes())], &[]);
    let encrypted = workbook(0x0600, &[record(0x002F, &[0; 6])], &[]);
    let ancient = workbook(0x0400, &[], &[]);
    for stream in [cyrillic, ancient] {
        assert_matches!(sheets(&stream), Err(Error::UnsupportedDocument(_)));
    }
    assert_matches!(sheets(&encrypted), Err(Error::PasswordRequired));
    let document = content(&ole_of("WordDocument", b"not a workbook"), &Limits::default());
    assert_matches!(document, Err(Error::UnsupportedDocument(reason)) if reason.contains(".doc"));
}
