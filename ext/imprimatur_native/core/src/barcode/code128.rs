//! Code 128 (ISO/IEC 15417): ASCII in code sets A (controls, upper case)
//! and B (upper and lower case), and pairs of digits in set C, switching
//! as Annex E says; FNC1 first makes it GS1-128.

use super::Symbology;
use super::gs1::Unit;
use crate::Result;

/// Each symbol's bar and space widths, a bar first: values 0 to 102, then
/// Start A, B and C (103 to 105).
const PATTERNS: [&[u8; 6]; 106] = [
    b"212222", b"222122", b"222221", b"121223", b"121322", b"131222", b"122213", b"122312",
    b"132212", b"221213", b"221312", b"231212", b"112232", b"122132", b"122231", b"113222",
    b"123122", b"123221", b"223211", b"221132", b"221231", b"213212", b"223112", b"312131",
    b"311222", b"321122", b"321221", b"312212", b"322112", b"322211", b"212123", b"212321",
    b"232121", b"111323", b"131123", b"131321", b"112313", b"132113", b"132311", b"211313",
    b"231113", b"231311", b"112133", b"112331", b"132131", b"113123", b"113321", b"133121",
    b"313121", b"211331", b"231131", b"213113", b"213311", b"213131", b"311123", b"311321",
    b"331121", b"312113", b"312311", b"332111", b"314111", b"221411", b"431111", b"111224",
    b"111422", b"121124", b"121421", b"141122", b"141221", b"112214", b"112412", b"122114",
    b"122411", b"142112", b"142211", b"241211", b"221114", b"413111", b"241112", b"134111",
    b"111242", b"121142", b"121241", b"114212", b"124112", b"124211", b"411212", b"421112",
    b"421211", b"212141", b"214121", b"412121", b"111143", b"111341", b"131141", b"114113",
    b"114311", b"411113", b"411311", b"113141", b"114131", b"311141", b"411131", b"211412",
    b"211214", b"211232",
];
const STOP: &[u8; 7] = b"2331112";
const CODE_C: u8 = 99;
const CODE_B: u8 = 100;
const CODE_A: u8 = 101;
const FNC1: u8 = 102;
const START_A: u8 = 103;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Set {
    A,
    B,
    C,
}

/// The modules of `units`: start, data, check symbol, stop.
pub fn encode(units: &[Unit]) -> Result<Vec<bool>> {
    if let Some(byte) = units.iter().find_map(|unit| match unit {
        Unit::Char(byte) if !byte.is_ascii() => Some(*byte),
        _ => None,
    }) {
        let reason = format!("it holds ASCII only, not byte 0x{byte:02X}");
        return Err(Symbology::Code128.invalid(reason));
    }
    let mut set = start(units);
    let mut values = vec![START_A + set as u8];
    let mut at = 0;
    while let Some(&unit) = units.get(at) {
        match (set, unit) {
            (_, Unit::Fnc1) => {
                values.push(FNC1);
                at += 1;
            }
            (Set::C, Unit::Char(_)) => {
                if let Some(pair) = digit_pair(units, at) {
                    values.push(pair);
                    at += 2;
                } else {
                    set = text_set(units, at);
                    values.push(code_symbol(set));
                }
            }
            (Set::A | Set::B, Unit::Char(byte)) => {
                // Annex E: four digits or more go to set C, the first of an
                // odd run first.
                let run = digit_run(units, at);
                if run >= 4 && run.is_multiple_of(2) {
                    set = Set::C;
                    values.push(CODE_C);
                    continue;
                }
                if !encodable(set, byte) {
                    set = if set == Set::A { Set::B } else { Set::A };
                    values.push(code_symbol(set));
                }
                values.push(value(set, byte));
                at += 1;
            }
        }
    }
    let weighted: u32 = values
        .iter()
        .zip(0_u32..)
        .map(|(&value, position)| u32::from(value) * position.max(1))
        .sum();
    values.push(u8::try_from(weighted % 103).unwrap_or_default());

    let mut modules = Vec::with_capacity(values.len() * 11 + 13);
    for value in values {
        widths(&mut modules, PATTERNS[usize::from(value)]);
    }
    widths(&mut modules, STOP);
    Ok(modules)
}

/// Set C when the data, past a leading FNC1, opens with four digits or more
/// (or is an even number of digits); else as [`text_set`].
fn start(units: &[Unit]) -> Set {
    let first = units.iter().position(|unit| *unit != Unit::Fnc1).unwrap_or(units.len());
    let run = digit_run(units, first);
    let all_digits = first + run == units.len() && run.is_multiple_of(2) && run > 0;
    if run >= 4 || all_digits { Set::C } else { text_set(units, first) }
}

/// Set A when a control character comes before any lower case letter, else
/// set B.
fn text_set(units: &[Unit], from: usize) -> Set {
    let first = units[from..].iter().find_map(|unit| match unit {
        Unit::Char(byte) if *byte < 32 => Some(Set::A),
        Unit::Char(byte) if *byte >= 96 => Some(Set::B),
        _ => None,
    });
    first.unwrap_or(Set::B)
}

fn digit_run(units: &[Unit], from: usize) -> usize {
    let digit = |unit: &&Unit| matches!(unit, Unit::Char(byte) if byte.is_ascii_digit());
    units.get(from..).map_or(0, |rest| rest.iter().take_while(digit).count())
}

fn digit_pair(units: &[Unit], at: usize) -> Option<u8> {
    match units.get(at..at + 2)? {
        [Unit::Char(tens), Unit::Char(ones)] if tens.is_ascii_digit() && ones.is_ascii_digit() => {
            Some((tens - b'0') * 10 + (ones - b'0'))
        }
        _ => None,
    }
}

/// The symbol that switches into set A or B (sets are otherwise only
/// entered from set C, by [`CODE_C`]).
const fn code_symbol(set: Set) -> u8 {
    if matches!(set, Set::A) { CODE_A } else { CODE_B }
}

const fn encodable(set: Set, byte: u8) -> bool {
    match set {
        Set::A => byte < 96,
        Set::B | Set::C => byte >= 32,
    }
}

/// A character's value in set A or B.
const fn value(set: Set, byte: u8) -> u8 {
    match set {
        Set::A if byte < 32 => byte + 64,
        _ => byte - 32,
    }
}

/// Bars and spaces of these widths, a bar first.
fn widths(modules: &mut Vec<bool>, widths: &[u8]) {
    for (index, width) in widths.iter().enumerate() {
        let bar = index % 2 == 0;
        modules.extend(std::iter::repeat_n(bar, usize::from(width - b'0')));
    }
}
