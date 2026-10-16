//! Scripts and the direction they are written in, for reading text back
//! in order and for writing it with the right direction.

#[cfg(test)]
mod tests;

use crate::style::TextDirection;

/// What a character is, for the direction of the text around it (its
/// UAX #9 bidirectional class, simplified).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// A letter of Hebrew or another right-to-left script but Arabic's, or
    /// the right-to-left mark.
    Right,
    /// A letter of the Arabic script (Arabic, Persian, Urdu..., Syriac,
    /// Thaana): right to left, and the digits after it are Arabic numbers.
    Arabic,
    /// Any other letter, or the left-to-right mark.
    Left,
    /// A number: its digits read left to right in any line, but in a
    /// right-to-left run the spaces between two numbers read right to left.
    Number,
    /// Arabic-Indic digits (not the Persian ones), and digits after Arabic
    /// letters: a number that only `,` `.` `:` `/` join, and no sign goes
    /// with (UAX #9, W2).
    ArabicNumber,
    /// Spaces, punctuation and symbols: they take the direction around them.
    Neutral,
}

impl Class {
    pub const fn right_to_left(self) -> bool {
        matches!(self, Self::Right | Self::Arabic)
    }

    pub const fn number(self) -> bool {
        matches!(self, Self::Number | Self::ArabicNumber)
    }

    pub const fn strong(self) -> bool {
        matches!(self, Self::Right | Self::Arabic | Self::Left)
    }
}

/// The letters and digits of some text, by direction.
#[derive(Debug, Default, Clone, Copy)]
pub struct Counts {
    pub right: usize,
    pub left: usize,
    pub numbers: usize,
}

impl Counts {
    pub fn add(&mut self, text: &str) {
        for ch in text.chars() {
            match class(ch) {
                Class::Right | Class::Arabic => self.right += 1,
                Class::Left if ch.is_alphabetic() => self.left += 1,
                Class::Number | Class::ArabicNumber => self.numbers += 1,
                Class::Left | Class::Neutral => {}
            }
        }
    }

    /// Whether more of the letters read right to left than left to right.
    pub const fn right_to_left(self) -> bool {
        self.right > self.left
    }

    /// The direction most of the letters read in; `None` without a letter.
    pub const fn direction(self) -> Option<TextDirection> {
        if self.right > self.left {
            Some(TextDirection::RightToLeft)
        } else if self.left > 0 {
            Some(TextDirection::LeftToRight)
        } else {
            None
        }
    }
}

/// The direction of a character.
///
/// Right to left: Hebrew (and Yiddish, Ladino), Arabic (and Persian, Urdu,
/// Pashto, Sindhi, Kurdish, Uyghur), Syriac, Thaana (Dhivehi), N'Ko,
/// Samaritan, Mandaic, their presentation forms; Adlam (Fulani), Hanifi
/// Rohingya, Mende Kikakui, Yezidi, Garay and the historic scripts
/// (Phoenician, Aramaic, Sogdian...); and the right-to-left mark. Of
/// those, the Arabic script (with Syriac, Thaana, Hanifi Rohingya and the
/// Siyaq numbers) is set apart. Digits are numbers, the Persian ones
/// (Extended Arabic-Indic) too; Arabic-Indic ones are Arabic numbers. The
/// Arabic comma, percent sign and separators are neutral, like their Latin
/// counterparts.
pub fn class(ch: char) -> Class {
    match ch {
        '0'..='9' | '\u{06F0}'..='\u{06F9}' => Class::Number,
        '\u{0660}'..='\u{0669}' => Class::ArabicNumber,
        '\u{060C}' | '\u{066A}'..='\u{066C}' => Class::Neutral,
        '\u{0600}'..='\u{07BF}'
        | '\u{0860}'..='\u{08FF}'
        | '\u{FB50}'..='\u{FDFF}'
        | '\u{FE70}'..='\u{FEFC}'
        | '\u{10D00}'..='\u{10D3F}'
        | '\u{10EC0}'..='\u{10EFF}'
        | '\u{1EC70}'..='\u{1EEFF}' => Class::Arabic,
        '\u{0590}'..='\u{08FF}'
        | '\u{200F}'
        | '\u{FB1D}'..='\u{FB4F}'
        | '\u{10800}'..='\u{10FFF}'
        | '\u{1E800}'..='\u{1EFFF}' => Class::Right,
        '\u{200E}' => Class::Left,
        ch if ch.is_alphanumeric() => Class::Left,
        _ => Class::Neutral,
    }
}
