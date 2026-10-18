use super::*;

/// The units as text, FNC1 as `|`.
fn spelled(input: &str) -> String {
    let fields = parse(input).unwrap();
    units(&fields)
        .into_iter()
        .map(|unit| match unit {
            Unit::Char(byte) => char::from(byte),
            Unit::Fnc1 => '|',
        })
        .collect()
}

#[test]
fn check_digits() {
    const CASES: &[(&str, Option<u8>)] = &[
        // GS1 General Specifications' worked examples: GTIN-13, SSCC, GTIN-8.
        ("950600013435", Some(2)),
        ("10614141234567890", Some(8)),
        ("9638507", Some(4)),
        ("03600029145", Some(2)),
        ("", Some(0)),
        ("12a4", None),
    ];
    for (digits, expected) in CASES {
        assert_eq!(check_digit(digits), *expected, "digits: {digits:?}");
    }
}

#[test]
fn runs_fields_together_with_separators_after_variable_ones() {
    const CASES: &[(&str, &str)] = &[
        ("(01)09506000134352", "0109506000134352"),
        // Predefined lengths need no separator, even before another field.
        ("(01)09506000134352(17)251231(10)AB12", "01095060001343521725123110AB12"),
        // A variable field is closed by FNC1 unless last.
        ("(10)AB12(21)XYZ", "10AB12|21XYZ"),
        ("(21)A%B(3103)000189", "21A%B|3103000189"),
        // A parenthesis in a value is data unless it opens an AI.
        ("(21)A(B)C", "21A(B)C"),
        ("(00)106141412345678908", "00106141412345678908"),
        // 410 to 417 (GLNs) are predefined too.
        ("(414)0614141000005(8200)https://example.com", "41406141410000058200https://example.com"),
    ];
    for (input, expected) in CASES {
        assert_eq!(spelled(input), *expected, "input: {input:?}");
    }
}

#[test]
fn rejects_what_gs1_does_not_allow() {
    const CASES: &[(&str, &str)] = &[
        ("", "it is empty"),
        ("0109506000134352", "expected an AI such as (01) at character 1"),
        ("(1)ABC", "expected an AI such as (01) at character 1"),
        ("(12345)ABC", "expected an AI such as (01) at character 1"),
        ("(10)", "(10) has no value"),
        ("(10)AB(21)", "(21) has no value"),
        ("(10)AB C", "(10) holds ' ', outside GS1's character set"),
        ("(10)café", "(10) holds 'é', outside GS1's character set"),
        ("(01)0950600013435", "(01) takes 14 digits, not \"0950600013435\""),
        ("(17)25123A", "(17) takes 6 digits, not \"25123A\""),
        ("(01)09506000134353", "(01) 09506000134353 has check digit 3, not 2"),
        ("(00)106141412345678909", "(00) 106141412345678909 has check digit 9, not 8"),
        ("(414)0614141000006", "(414) 0614141000006 has check digit 6, not 5"),
    ];
    for (input, expected) in CASES {
        let error = parse(input).unwrap_err().to_string();
        assert_eq!(error, format!("invalid GS1 element string: {expected}"), "input: {input:?}");
    }
    let long = format!("(10){}", "A".repeat(91));
    assert!(parse(&long).unwrap_err().to_string().contains("more than 90"));
}
