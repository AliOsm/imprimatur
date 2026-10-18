//! GS1 element strings as people write them, `(01)09506000134352(10)AB12`:
//! Application Identifiers in parentheses, each followed by its value.

#[cfg(test)]
mod tests;

use crate::{Error, Result};

/// What a symbol carries: characters, and FNC1, which ends every
/// variable-length field but the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Char(u8),
    Fnc1,
}

/// One Application Identifier and its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field<'a> {
    pub ai: &'a str,
    pub value: &'a str,
}

/// Element strings whose length, AI included, the AI's first two digits
/// fix (GS1 General Specifications, figure 7.8.5-2). All are numeric; the
/// rest end with FNC1 unless last.
const PREDEFINED: [(&str, usize); 22] = [
    ("00", 20),
    ("01", 16),
    ("02", 16),
    ("03", 16),
    ("04", 18),
    ("11", 8),
    ("12", 8),
    ("13", 8),
    ("14", 8),
    ("15", 8),
    ("16", 8),
    ("17", 8),
    ("18", 8),
    ("19", 8),
    ("20", 4),
    ("31", 10),
    ("32", 10),
    ("33", 10),
    ("34", 10),
    ("35", 10),
    ("36", 10),
    ("41", 16),
];

/// The longest value any AI takes.
const MAX_VALUE: usize = 90;

/// The GS1 check digit (modulo 10, weights 3 and 1 from the right) of
/// `digits`, as GTINs, SSCCs and GLNs end with; `None` unless all are
/// digits.
///
/// ```
/// assert_eq!(imprimatur::barcode::check_digit("950600013435"), Some(2));
/// ```
#[must_use]
pub fn check_digit(digits: &str) -> Option<u8> {
    let mut sum = 0_u32;
    for (index, byte) in digits.bytes().rev().enumerate() {
        byte.is_ascii_digit().then_some(())?;
        let weight = if index % 2 == 0 { 3 } else { 1 };
        sum += weight * u32::from(byte - b'0');
    }
    u8::try_from((10 - sum % 10) % 10).ok()
}

/// Reads an element string into its fields.
///
/// # Errors
/// [`Error::InvalidElementString`] for anything but AIs of 2 to 4 digits in
/// parentheses, each with a value in GS1's character set, of the length
/// and check digit its AI takes.
pub fn parse(input: &str) -> Result<Vec<Field<'_>>> {
    let invalid = |reason: String| Error::InvalidElementString(reason);
    let bytes = input.as_bytes();
    (!bytes.is_empty()).ok_or_else(|| invalid("it is empty".to_owned()))?;
    let mut fields = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let length = ai_at(bytes, at).ok_or_else(|| {
            invalid(format!("expected an AI such as (01) at character {}", at + 1))
        })?;
        let start = at + length + 2;
        let end = (start..bytes.len()).find(|&next| ai_at(bytes, next).is_some());
        let end = end.unwrap_or(bytes.len());
        let field = Field { ai: &input[at + 1..at + 1 + length], value: &input[start..end] };
        validate(field)?;
        fields.push(field);
        at = end;
    }
    Ok(fields)
}

/// `element_string`, parsed and run together as [`units`] does: what every
/// encoder needs before laying its symbol out.
///
/// # Errors
/// [`Error::InvalidElementString`], as [`parse`].
pub fn element_units(element_string: &str) -> Result<Vec<Unit>> {
    Ok(units(&parse(element_string)?))
}

/// The fields run together: no parentheses, FNC1 after each field of
/// variable length but the last.
pub fn units(fields: &[Field<'_>]) -> Vec<Unit> {
    let mut units = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        units.extend(field.ai.bytes().chain(field.value.bytes()).map(Unit::Char));
        if predefined(field.ai).is_none() && index + 1 < fields.len() {
            units.push(Unit::Fnc1);
        }
    }
    units
}

/// The length of the AI in parentheses at `at`: 2 to 4 digits.
fn ai_at(bytes: &[u8], at: usize) -> Option<usize> {
    (bytes.get(at) == Some(&b'(')).then_some(())?;
    let digits = bytes[at + 1..].iter().take_while(|byte| byte.is_ascii_digit()).count();
    ((2..=4).contains(&digits) && bytes.get(at + 1 + digits) == Some(&b')')).then_some(digits)
}

/// The length of the element string, AI included, when the AI fixes it.
fn predefined(ai: &str) -> Option<usize> {
    PREDEFINED.iter().find(|(prefix, _)| ai.starts_with(prefix)).map(|&(_, length)| length)
}

fn validate(field: Field<'_>) -> Result<()> {
    let Field { ai, value } = field;
    let invalid = |reason: String| Err(Error::InvalidElementString(format!("({ai}) {reason}")));
    if value.is_empty() {
        return invalid("has no value".to_owned());
    }
    let outside = |character: char| !u8::try_from(character).is_ok_and(in_character_set);
    if let Some(character) = value.chars().find(|&character| outside(character)) {
        return invalid(format!("holds {character:?}, outside GS1's character set"));
    }
    if value.len() > MAX_VALUE {
        return invalid(format!("holds {} characters, more than {MAX_VALUE}", value.len()));
    }
    let Some(length) = predefined(ai) else { return Ok(()) };
    let digits = length - ai.len();
    if value.len() != digits || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return invalid(format!("takes {digits} digits, not {value:?}"));
    }
    // SSCC, GTINs and GLNs end with a check digit.
    let checked = matches!(ai, "00" | "01" | "02") || ai.starts_with("41");
    let (body, last) = value.split_at(value.len() - 1);
    let expected = check_digit(body).unwrap_or(0);
    if checked && last.as_bytes()[0] - b'0' != expected {
        return invalid(format!("{value} has check digit {last}, not {expected}"));
    }
    Ok(())
}

/// GS1 AI encodable character set 82: letters, digits and
/// ``!"%&'()*+,-./:;<=>?_``.
const fn in_character_set(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'"'
                | b'%'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b'-'
                | b'.'
                | b'/'
                | b':'
                | b';'
                | b'<'
                | b'='
                | b'>'
                | b'?'
                | b'_'
        )
}
