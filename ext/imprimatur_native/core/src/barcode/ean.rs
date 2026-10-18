//! EAN-13, EAN-8 and UPC-A (ISO/IEC 15420): GTINs as retail scanners read
//! them, digits in seven-module patterns between guard bars.

use super::Symbology;
use super::gs1::check_digit;
use crate::Result;

/// The left-hand, odd-parity patterns (set A) of 0 to 9. The right-hand
/// patterns (set C) are their inverses; the even-parity ones (set B), the
/// right-hand ones mirrored.
const SET_A: [u8; 10] = [
    0b000_1101, 0b001_1001, 0b001_0011, 0b011_1101, 0b010_0011, 0b011_0001, 0b010_1111, 0b011_1011,
    0b011_0111, 0b000_1011,
];
/// Which of EAN-13's left six digits take set B, the first in the high
/// bit, by its leading digit (the one no pattern draws).
const PARITY: [u8; 10] = [
    0b00_0000, 0b00_1011, 0b00_1101, 0b00_1110, 0b01_0011, 0b01_1001, 0b01_1100, 0b01_0101,
    0b01_0110, 0b01_1010,
];
const GUARD: [bool; 3] = [true, false, true];
const CENTER: [bool; 5] = [false, true, false, true, false];

/// `input` as digits: one short of `length` gets its check digit, `length`
/// must end with the right one.
pub fn digits(symbology: Symbology, input: &str, length: usize) -> Result<Vec<u8>> {
    input
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .ok_or_else(|| symbology.invalid(format!("{input:?} is not all digits")))?;
    (input.len() == length - 1 || input.len() == length).ok_or_else(|| {
        let (short, given) = (length - 1, input.len());
        symbology.invalid(format!(
            "it takes {short} digits, or {length} with the check digit, not {given}"
        ))
    })?;
    let check = check_digit(&input[..length - 1]).unwrap_or_default();
    let mut digits: Vec<u8> = input.bytes().map(|byte| byte - b'0').collect();
    match digits.get(length - 1) {
        None => digits.push(check),
        Some(&last) if last != check => {
            return Err(symbology.invalid(format!("{input} has check digit {last}, not {check}")));
        }
        Some(_) => {}
    }
    Ok(digits)
}

pub fn text(digits: &[u8]) -> String {
    digits.iter().map(|digit| char::from(b'0' + digit)).collect()
}

/// 95 modules: guard, six left digits by the first's parity, center, six
/// right digits, guard.
pub fn ean13(digits: &[u8]) -> Vec<bool> {
    let parity = PARITY[usize::from(digits[0])];
    let mut modules = Vec::with_capacity(95);
    modules.extend(GUARD);
    for (index, &digit) in digits[1..7].iter().enumerate() {
        let even = (parity >> (5 - index)) & 1 == 1;
        push(&mut modules, if even { set_b(digit) } else { SET_A[usize::from(digit)] });
    }
    modules.extend(CENTER);
    digits[7..].iter().for_each(|&digit| push(&mut modules, set_c(digit)));
    modules.extend(GUARD);
    modules
}

/// 67 modules: guard, four digits in set A, center, four in set C, guard.
pub fn ean8(digits: &[u8]) -> Vec<bool> {
    let mut modules = Vec::with_capacity(67);
    modules.extend(GUARD);
    digits[..4].iter().for_each(|&digit| push(&mut modules, SET_A[usize::from(digit)]));
    modules.extend(CENTER);
    digits[4..].iter().for_each(|&digit| push(&mut modules, set_c(digit)));
    modules.extend(GUARD);
    modules
}

const fn set_c(digit: u8) -> u8 {
    !SET_A[digit as usize] & 0x7F
}

const fn set_b(digit: u8) -> u8 {
    set_c(digit).reverse_bits() >> 1
}

/// A seven-module pattern, its high bit first.
fn push(modules: &mut Vec<bool>, pattern: u8) {
    modules.extend((0..7).rev().map(|bit| (pattern >> bit) & 1 == 1));
}
