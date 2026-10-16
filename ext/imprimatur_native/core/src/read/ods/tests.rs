use std::assert_matches;

use crate::Error;
use crate::read::{Content, ImageFormat, Limits, Note, Value, content, images};
use crate::test_support::zip_of;
use crate::xlsx::DateTime;

fn package(kind: &str, extra: &[(&str, &str)]) -> Vec<u8> {
    let mut parts = vec![("mimetype", kind), ("content.xml", include_str!("tests/content.xml"))];
    parts.extend_from_slice(extra);
    zip_of(&parts)
}

const SPREADSHEET: &str = "application/vnd.oasis.opendocument.spreadsheet";

#[test]
fn reads_values_text_and_comments() {
    let Content::Ods(sheets) = content(&package(SPREADSHEET, &[]), &Limits::default()).unwrap()
    else {
        panic!("not read as ODS")
    };
    let names: Vec<(&str, bool)> =
        sheets.iter().map(|sheet| (sheet.name.as_str(), sheet.hidden)).collect();
    assert_eq!(names, [("Données", false), ("Hidden", true)]);
    let cells = |index: usize| -> Vec<(u32, u32, String, Value)> {
        let grid = &sheets[index].cells;
        grid.cells()
            .iter()
            .map(|cell| (cell.row, cell.column, grid.text(cell).to_owned(), cell.value))
            .collect()
    };
    let date = DateTime::date(2023, 3, 15).unwrap();
    let morning = date.at(9, 30, 0, 0).unwrap();
    let repeated = |row| {
        [
            (row, 0, "2023-03-15".to_owned(), Value::Date(date)),
            (row, 1, "2023-03-15 09:30:00".to_owned(), Value::Date(morning)),
            (row, 2, "18:00:00".to_owned(), Value::Number(0.75)),
            (row, 3, "€12.00".to_owned(), Value::Number(12.0)),
            (row, 4, "=SUM([.B2:.B3])".to_owned(), Value::Formula),
        ]
    };
    let mut expected = vec![
        (0, 0, "Name".to_owned(), Value::Text),
        (0, 1, "Amount".to_owned(), Value::Text),
        (0, 4, "Note".to_owned(), Value::Text),
        (1, 0, "Two  spaces\nsecond line".to_owned(), Value::Text),
        (1, 1, "1,250.50".to_owned(), Value::Number(1250.5)),
        (1, 2, "25%".to_owned(), Value::Number(0.25)),
        (1, 3, "TRUE".to_owned(), Value::Bool(true)),
        (1, 4, "Due 15/03/2023".to_owned(), Value::Text),
    ];
    expected.extend(repeated(2));
    expected.extend(repeated(3));
    assert_eq!(cells(0), expected, "a drawing's text and the empty repeats are left out");
    let note = Note { row: 1, column: 0, author: Some("Finance".into()), text: "check".into() };
    assert_eq!(sheets[0].comments, [note]);
    let seven: Vec<_> =
        (0..3).map(|column| (0, column, "7".to_owned(), Value::Number(7.0))).collect();
    assert_eq!(cells(1), seven, "a repeated cell with a value is one cell per column");
}

#[test]
fn tells_other_opendocument_files_apart() {
    let template = content(
        &package("application/vnd.oasis.opendocument.spreadsheet-template", &[]),
        &Limits::default(),
    );
    assert_matches!(template, Ok(Content::Ods(_)));
    let text =
        content(&package("application/vnd.oasis.opendocument.text", &[]), &Limits::default());
    assert_matches!(text, Err(Error::UnsupportedDocument(_)));
}

#[test]
fn finds_pictures() {
    let ods = package(
        SPREADSHEET,
        &[("Pictures/logo.gif", "GIF89a\x02\x00\x03\x00"), ("Thumbnails/thumbnail.png", "")],
    );
    let mut found = Vec::new();
    images(&ods, &Limits::default(), |image| {
        found.push((image.format, image.width, image.height));
        Ok(())
    })
    .unwrap();
    assert_eq!(
        found,
        [(ImageFormat::Gif, Some(2), Some(3))],
        "the thumbnail is no picture of the document"
    );
}
