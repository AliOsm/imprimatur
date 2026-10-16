//! Right-to-left text, from the order a PDF draws it to the order it reads.
//!
//! A PDF draws every line left to right, whatever its script: a Hebrew or
//! Arabic line comes out backwards, and so does an Arabic name inside an
//! English line. Each piece of a line gets the level the Unicode
//! bidirectional algorithm (UAX #9) gives it, even for left to right, odd
//! for right to left, and the runs are reversed the way a renderer reverses
//! them: reversed twice, a line reads as it was written. The page's own
//! direction stands in for the paragraph's, which a PDF does not record.

#[cfg(test)]
mod tests;

use std::ops::Range;

/// What a piece of a line is, for its direction.
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
    /// letters: a number that
    /// only `,` `.` `:` `/` join, and no sign goes with (UAX #9, W2).
    ArabicNumber,
    /// Spaces, punctuation and symbols: they take the direction around them.
    Neutral,
}

impl Class {
    const fn right_to_left(self) -> bool {
        matches!(self, Self::Right | Self::Arabic)
    }

    const fn number(self) -> bool {
        matches!(self, Self::Number | Self::ArabicNumber)
    }

    const fn strong(self) -> bool {
        matches!(self, Self::Right | Self::Arabic | Self::Left)
    }
}

/// Reusable buffers for [`reading_order`].
#[derive(Debug, Default)]
pub struct Scratch {
    classes: Vec<Class>,
    levels: Vec<u8>,
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

/// Combining marks: diacritics, and the vowel points and signs of the
/// right-to-left scripts. Drawn as glyphs of their own, they never start a
/// word.
pub const fn is_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}'..='\u{036F}'
            // Hebrew points and cantillation.
            | '\u{0591}'..='\u{05C7}'
            | '\u{FB1E}'
            // Arabic harakat, Quranic annotation, extended Arabic marks.
            | '\u{0610}'..='\u{061A}'
            | '\u{064B}'..='\u{065F}'
            | '\u{0670}'
            | '\u{06D6}'..='\u{06ED}'
            | '\u{0898}'..='\u{089F}'
            | '\u{08CA}'..='\u{08E1}'
            | '\u{08E3}'..='\u{08FF}'
            // Syriac, Thaana, N'Ko, Samaritan, Mandaic.
            | '\u{0711}'
            | '\u{0730}'..='\u{074A}'
            | '\u{07A6}'..='\u{07B0}'
            | '\u{07EB}'..='\u{07F3}'
            | '\u{07FD}'
            | '\u{0816}'..='\u{0819}'
            | '\u{081B}'..='\u{0823}'
            | '\u{0825}'..='\u{0827}'
            | '\u{0829}'..='\u{082D}'
            | '\u{0859}'..='\u{085B}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{FE20}'..='\u{FE2F}'
            // Hanifi Rohingya, Adlam.
            | '\u{10D24}'..='\u{10D27}'
            | '\u{1E944}'..='\u{1E94A}'
    )
}

/// Whether a line reads right to left, from its letters and its page's
/// direction. On a right-to-left page a line does when it has a
/// right-to-left letter at all, or only figures (a table row reads from the
/// right); on a left-to-right page only when its letters are all, or nearly
/// all, right to left: an Arabic name in an English line stays in an
/// English line.
pub const fn line_right_to_left(line: Counts, page_right_to_left: bool) -> bool {
    if page_right_to_left {
        line.right > 0 || line.left == 0 && line.numbers > 0
    } else {
        line.right > 0 && line.right >= 4 * line.left
    }
}

/// Puts a line's pieces, drawn left to right, in reading order. Lines
/// with nothing right to left in them are left as they are.
pub fn reading_order(pieces: &mut [&str], page_right_to_left: bool, scratch: &mut Scratch) {
    let mut counts = Counts::default();
    for piece in pieces.iter() {
        counts.add(piece);
    }
    let right_to_left = line_right_to_left(counts, page_right_to_left);
    if right_to_left || counts.right > 0 {
        reorder(pieces, right_to_left, scratch);
    }
}

/// UAX #9 on pieces in drawn order: classes, numbers (W2, W7, W4, W5),
/// neutrals (N1, N2), levels (I1, I2), then the runs reversed (L2).
fn reorder(pieces: &mut [&str], right_to_left: bool, scratch: &mut Scratch) {
    let Scratch { classes, levels } = scratch;
    classes.clear();
    classes.extend(pieces.iter().map(|piece| {
        piece.chars().map(class).find(|&class| class != Class::Neutral).unwrap_or(Class::Neutral)
    }));
    resolve_numbers(classes, right_to_left);
    join_numbers(pieces, classes);
    resolve_neutrals(classes, right_to_left);
    levels.clear();
    levels.extend(classes.iter().map(|class| match class {
        Class::Right | Class::Arabic => 1,
        Class::Number | Class::ArabicNumber => 2,
        Class::Left | Class::Neutral => {
            if right_to_left {
                2
            } else {
                0
            }
        }
    }));
    // From the highest level to the lowest odd one, every run at that
    // level or above reversed.
    for level in [2, 1] {
        let mut from = 0;
        while let Some(run) = run(levels, from, |&piece| piece >= level) {
            from = run.end;
            pieces[run].reverse();
        }
    }
}

/// The next run of items, from `from` on, that `wanted` picks.
fn run<T>(items: &[T], from: usize, wanted: impl Fn(&T) -> bool) -> Option<Range<usize>> {
    let start = from + items.get(from..)?.iter().position(&wanted)?;
    let length = items[start..].iter().position(|item| !wanted(item));
    Some(start..length.map_or(items.len(), |length| start + length))
}

/// What joins two digits into one number, Arabic numbers included:
/// `1,234.5`, `9:30`, `3/4`.
const SEPARATORS: [&str; 8] = [",", ".", ":", "/", "\u{00A0}", "\u{060C}", "\u{066B}", "\u{066C}"];

/// What joins two digits into one number in any but Arabic text:
/// `2024-03-15`, `+966`.
const SIGNS_BETWEEN: [&str; 3] = ["+", "-", "\u{2212}"];

/// Signs that go with the number next to them, in any but Arabic text:
/// `50%`, `$5`.
const SIGNS: [&str; 11] = ["%", "‰", "‱", "°", "$", "€", "£", "¥", "¢", "#", "\u{066A}"];

/// What kind of number each run of digits is. A number touching a letter
/// (`ISO9001`) goes with its word. In a right-to-left line, a number after
/// left-to-right text, the next letter to its left, is part of that text
/// (`ISO 9001`); in a left-to-right line, a number is part of a
/// right-to-left run when one follows it, the next letter to its right.
/// Digits following Arabic letters, in reading order, are Arabic numbers.
fn resolve_numbers(classes: &mut [Class], right_to_left: bool) {
    let mut from = 0;
    while let Some(Range { start, end: at }) = run(classes, from, |&class| class == Class::Number) {
        from = at;
        let glued = (start > 0 && classes[start - 1] == Class::Left)
            || classes.get(at) == Some(&Class::Left);
        let before = classes[..start].iter().rev().find(|class| class.strong());
        let after = classes[at..].iter().find(|class| class.strong());
        let left_to_right = if right_to_left {
            before == Some(&Class::Left)
        } else {
            !after.is_some_and(|class| class.right_to_left())
        };
        if glued || left_to_right {
            classes[start..at].fill(Class::Left);
        } else if after == Some(&Class::Arabic) {
            classes[start..at].fill(Class::ArabicNumber);
        }
    }
}

/// Separators between two digits, and signs next to one, as part of the
/// number, as far as its kind allows.
fn join_numbers(pieces: &[&str], classes: &mut [Class]) {
    for at in 1..classes.len().saturating_sub(1) {
        let (before, after) = (classes[at - 1], classes[at + 1]);
        let joins = classes[at] == Class::Neutral
            && before == after
            && match before {
                Class::Number => {
                    SEPARATORS.contains(&pieces[at]) || SIGNS_BETWEEN.contains(&pieces[at])
                }
                Class::ArabicNumber => SEPARATORS.contains(&pieces[at]),
                _ => false,
            };
        if joins {
            classes[at] = before;
        }
    }
    for at in (0..classes.len()).chain((0..classes.len()).rev()) {
        let next_to_number = (at > 0 && classes[at - 1] == Class::Number)
            || classes.get(at + 1) == Some(&Class::Number);
        if classes[at] == Class::Neutral && SIGNS.contains(&pieces[at]) && next_to_number {
            classes[at] = Class::Number;
        }
    }
}

/// Spaces and punctuation between two pieces of one direction take it,
/// numbers counting as right to left; others take the line's.
fn resolve_neutrals(classes: &mut [Class], right_to_left: bool) {
    let line = if right_to_left { Class::Right } else { Class::Left };
    let side = |class: Class| {
        if class.right_to_left() || class.number() { Class::Right } else { Class::Left }
    };
    let mut from = 0;
    while let Some(Range { start, end: at }) = run(classes, from, |&class| class == Class::Neutral)
    {
        from = at;
        let before = if start == 0 { line } else { side(classes[start - 1]) };
        let after = classes.get(at).map_or(line, |&class| side(class));
        classes[start..at].fill(if before == after { before } else { line });
    }
}
