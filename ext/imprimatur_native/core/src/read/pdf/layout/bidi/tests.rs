use super::*;

/// A line as a PDF draws it, left to right, from its characters in that
/// order (an editor shows the right-to-left ones in these strings
/// reversed): a piece per character, marks on their letters.
fn drawn(line: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    for (at, ch) in line.char_indices().skip(1) {
        if !is_mark(ch) {
            pieces.push(&line[start..at]);
            start = at;
        }
    }
    if !line.is_empty() {
        pieces.push(&line[start..]);
    }
    pieces
}

#[test]
fn puts_lines_in_reading_order() {
    // (case, the page reads right to left, the line as drawn, as it reads)
    const CASES: &[(&str, bool, &str, &str)] = &[
        ("Arabic", true, "2024 برع", "عرب 2024"),
        ("Hebrew", true, "2024 םולש", "שלום 2024"),
        ("Persian", true, "مالس", "سلام"),
        ("Urdu", true, "ودرا", "اردو"),
        ("Yiddish", true, "שידיי", "יידיש"),
        ("Syriac", true, "ܐܡܠܫ", "ܫܠܡܐ"),
        ("Dhivehi, vowel signs on their letters", true, "ހިވެދި", "ދިވެހި"),
        ("N'Ko", true, "ߏߞߒ", "ߒߞߏ"),
        ("Adlam", true, "𞤥𞤢𞤤𞤣𞤀", "𞤀𞤣𞤤𞤢𞤥"),
        ("Hebrew around Latin and digits", true, "ISO 9001 רושיא", "אישור ISO 9001"),
        ("an Arabic line on an English page", false, "مالس", "سلام"),
        ("an Arabic name in an English line", false, "Supplier: ةكرش", "Supplier: شركة"),
        ("mostly Arabic", false, "Name: هللادبع دمحم", "Name: محمد عبدالله"),
        ("address", false, "Address: كلملا 15 عراش", "Address: شارع 15 الملك"),
        ("a number touching a Latin word", false, "ISO9001 ةكرش", "ISO9001 شركة"),
        ("mostly Latin, Arabic page", true, "Foxconn Group ةكرش", "شركة Foxconn Group"),
        ("a row of figures on an Arabic page", true, "100 200 300", "300 200 100"),
        ("a row of figures on an English page", false, "100 200 300", "100 200 300"),
        ("Arabic-Indic digits", true, "٢٠٢٤ ماع", "عام ٢٠٢٤"),
        ("Arabic numbers", true, "15-03-2024 يف %50 ةبسن", "نسبة 50% في 2024-03-15"),
        ("Hebrew numbers", true, "50% 2024-03-15 ךיראת", "תאריך 2024-03-15 50%"),
        ("an amount", true, "ח״ש 1,250.50 לוכה ךס", "סך הכול 1,250.50 ש״ח"),
        ("Persian digits", true, "۲۵٪ ISO 9001 الاک", "کالا ISO 9001 ۲۵٪"),
        ("Latin on an Arabic page", true, "(1) Hi", "(1) Hi"),
    ];
    let mut scratch = Scratch::default();
    for (case, page_right_to_left, line, reading) in CASES {
        let mut pieces = drawn(line);
        reading_order(&mut pieces, *page_right_to_left, &mut scratch);
        assert_eq!(pieces.concat(), *reading, "{case}");
    }
}

#[test]
fn knows_the_right_to_left_scripts_and_their_marks() {
    const MARKS: &[(char, bool)] = &[
        ('\u{05B4}', true),
        ('\u{064E}', true),
        ('\u{0730}', true),
        ('\u{07A8}', true),
        ('\u{07EB}', true),
        ('\u{0817}', true),
        ('\u{0859}', true),
        ('\u{10D24}', true),
        ('\u{1E944}', true),
        ('\u{05E9}', false),
        ('a', false),
    ];
    const CLASSES: &[(char, Class)] = &[
        ('\u{05D0}', Class::Right),
        ('\u{FB2A}', Class::Right),
        ('\u{07CA}', Class::Right),
        ('\u{0800}', Class::Right),
        ('\u{0840}', Class::Right),
        ('\u{1E900}', Class::Right),
        ('\u{200F}', Class::Right),
        ('\u{0628}', Class::Arabic),
        ('\u{0710}', Class::Arabic),
        ('\u{0780}', Class::Arabic),
        ('\u{FEFB}', Class::Arabic),
        ('\u{10D00}', Class::Arabic),
        ('\u{200E}', Class::Left),
        ('a', Class::Left),
        ('7', Class::Number),
        ('\u{0663}', Class::ArabicNumber),
        ('\u{06F5}', Class::Number),
        ('\u{066A}', Class::Neutral),
        ('\u{FEFF}', Class::Neutral),
        (' ', Class::Neutral),
    ];
    for &(ch, expected) in CLASSES {
        assert_eq!(class(ch), expected, "U+{:04X}", u32::from(ch));
    }
    for &(ch, expected) in MARKS {
        assert_eq!(is_mark(ch), expected, "U+{:04X}", u32::from(ch));
    }
}

#[test]
fn weighs_a_line_against_its_page() {
    // (case, line, the page reads right to left, the line does)
    const CASES: &[(&str, &str, bool, bool)] = &[
        ("only Arabic", "سلام", false, true),
        ("an Arabic name in English", "Supplier شركة", false, false),
        ("the same on an Arabic page", "Supplier شركة", true, true),
        ("only Latin on an Arabic page", "ISO 9001", true, false),
        ("figures on an Arabic page", "100 200", true, true),
        ("figures on an English page", "100 200", false, false),
        ("nothing to go by", "- * -", true, false),
    ];
    for (case, line, page, expected) in CASES {
        let mut counts = Counts::default();
        counts.add(line);
        assert_eq!(line_right_to_left(counts, *page), *expected, "{case}");
    }
}
