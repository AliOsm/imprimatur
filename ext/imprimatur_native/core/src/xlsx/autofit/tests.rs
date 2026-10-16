use super::*;
use crate::test_support::{part, text_parts};
use crate::xlsx::{Cell, DateTime, Format, Workbook};

/// The widths `widths` gives one sheet's row of `cells`, in `format`.
fn fitted(cells: Vec<Cell>, format: &Format, base: &Font) -> Vec<(u16, f64)> {
    let mut book = Workbook::new();
    book.add_sheet("Fit").unwrap().append_row_with_format(cells, format).unwrap();
    widths(&book.sheets()[0], base).into_iter().collect()
}

#[test]
fn sizes_columns_to_their_cells() {
    let date = DateTime::date(2024, 3, 15).unwrap();
    let cells = vec![
        Cell::from("Supplier"),
        Cell::Number(12.5),
        Cell::Bool(false),
        Cell::DateTime(date),
        Cell::DateTime(date.at(9, 30, 0, 0).unwrap()),
        Cell::from("two\nlines here"),
        Cell::formula("SUM(A1:A3)"),
    ];
    // (characters + 3) × 11 / 10; the formula's column is left alone.
    let expected = [(0, 12.1), (1, 7.7), (2, 8.8), (3, 14.3), (4, 24.2), (5, 14.3)];
    assert_eq!(fitted(cells, &Format::new(), &Font::BASE), expected);
}

#[test]
fn scales_with_the_font() {
    let bold = Format::new().bold().font_size(12.0).unwrap();
    // (8 + 3) × 12 / 10 × 1.5
    assert_eq!(fitted(vec!["Supplier".into()], &bold, &Font::BASE), [(0, 19.8)]);
    let base = Font { half_points: Some(20), ..Font::BASE };
    assert_eq!(fitted(vec!["Supplier".into()], &Format::new(), &base), [(0, 11.0)]);
    let long = "x".repeat(400);
    assert_eq!(fitted(vec![long.into()], &Format::new(), &Font::BASE), [(0, 255.0)]);
}

#[test]
fn widens_to_the_widest_cell_and_keeps_widths_set() {
    let mut book = Workbook::new();
    book.set_autofit(true);
    let sheet = book.add_sheet("Fit").unwrap();
    sheet.append_row(["Region", "Q1", "Notes"]).unwrap();
    sheet.append_row(["North America", "12", "n"]).unwrap();
    sheet.set_column_width(2, 30.0).unwrap();
    assert!(sheet.is_autofit(), "from the workbook");
    book.add_sheet("Plain").unwrap().set_autofit(false).append_row(["Region"]).unwrap();

    let parts = text_parts(book.to_bytes().unwrap());
    let cols = concat!(
        r#"<cols><col min="1" max="1" width="17.6" customWidth="1"/>"#,
        r#"<col min="2" max="2" width="5.5" customWidth="1"/>"#,
        r#"<col min="3" max="3" width="30" customWidth="1"/></cols>"#,
    );
    let fit = part(&parts, "xl/worksheets/sheet1.xml");
    assert!(fit.contains(cols), "{fit}");
    assert!(!part(&parts, "xl/worksheets/sheet2.xml").contains("<cols>"));
}
