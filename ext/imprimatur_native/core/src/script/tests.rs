use super::*;
use crate::style::TextDirection;

#[test]
fn knows_the_right_to_left_scripts() {
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
}

#[test]
fn finds_the_direction_of_text() {
    use TextDirection::{LeftToRight, RightToLeft};
    // (case, text, its direction)
    const CASES: &[(&str, &str, Option<TextDirection>)] = &[
        ("Arabic", "شهادة المطابقة", Some(RightToLeft)),
        ("Hebrew after figures", "2024 אישור", Some(RightToLeft)),
        ("Latin first", "ISO 9001 אישור", Some(LeftToRight)),
        ("Arabic first", "شركة Foxconn Group", Some(RightToLeft)),
        ("a right-to-left mark", "\u{200F}ISO", Some(RightToLeft)),
        ("figures only", "1,250.50 %", None),
        ("Arabic-Indic figures only", "٢٠٢٤", None),
        ("nothing", "", None),
    ];
    for (case, text, expected) in CASES {
        assert_eq!(TextDirection::of(text), *expected, "{case}");
    }
}

#[test]
fn weighs_letters() {
    let of = |text: &str| {
        let mut counts = Counts::default();
        counts.add(text);
        counts
    };
    let counts = of("Supplier: شركة الأمل 2024");
    assert_eq!((counts.right, counts.left, counts.numbers), (9, 8, 4));
    assert!(counts.right_to_left());
    assert!(!of("ISO 9001").right_to_left());
}
