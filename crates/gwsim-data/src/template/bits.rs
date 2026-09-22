//! The bit stream a template code is written in.
//!
//! The format is custom rather than ordinary Base64: each character carries
//! six bits **lowest bit first**, and each field's value is assembled lowest
//! bit first as well. A Base64 crate would decode to the wrong bytes, so this
//! is hand-rolled.
//!
//! Getting the bit order backwards decodes to plausible nonsense rather than
//! an obvious failure, which is why [`super`] checks itself against real
//! codes rather than only against round trips.

use std::fmt;

/// The alphabet template codes use: standard Base64, with no padding.
pub const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The six-bit value of a Base64 character.
fn value_of(character: char) -> Option<u8> {
    let byte = u8::try_from(u32::from(character)).ok()?;
    ALPHABET
        .iter()
        .position(|candidate| *candidate == byte)
        .map(|index| index as u8)
}

/// The character for a six-bit value.
fn character_of(value: u8) -> char {
    ALPHABET[usize::from(value & 0b11_1111)] as char
}

/// Something wrong with a template code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// A character that is not in the alphabet.
    BadCharacter { character: char, index: usize },
    /// The code ended while a field was still being read.
    Truncated {
        /// What was being read when it ran out.
        field: &'static str,
        /// How many bits were wanted.
        wanted: u32,
        /// How many were left.
        available: u32,
    },
    /// The type nibble says this is something else.
    WrongType {
        found: u8,
        expected: u8,
        /// What the found nibble actually is, where we know.
        found_is: Option<&'static str>,
    },
    /// A version this code does not know how to read.
    UnsupportedVersion { version: u8 },
    /// A field held a value that names nothing.
    BadValue {
        field: &'static str,
        value: u32,
        reason: &'static str,
    },
    /// The code is empty.
    Empty,
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemplateError::BadCharacter { character, index } => write!(
                f,
                "character {index} is {character:?}, which is not part of a template code"
            ),
            TemplateError::Truncated {
                field,
                wanted,
                available,
            } => write!(
                f,
                "the code ends in the middle of {field}: {wanted} more bits were needed \
                 but only {available} remain, so it is cut short"
            ),
            TemplateError::WrongType {
                found,
                expected,
                found_is,
            } => match found_is {
                Some(what) => write!(
                    f,
                    "this is {what} (type {found}), not a template of type {expected}"
                ),
                None => write!(f, "this has template type {found}, not {expected}"),
            },
            TemplateError::UnsupportedVersion { version } => {
                write!(f, "template version {version} is not one this can read")
            }
            TemplateError::BadValue {
                field,
                value,
                reason,
            } => write!(f, "{field} is {value}, which {reason}"),
            TemplateError::Empty => write!(f, "the code is empty"),
        }
    }
}

impl std::error::Error for TemplateError {}

/// Reads a template code bit by bit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitReader {
    bits: Vec<bool>,
    position: usize,
}

impl BitReader {
    /// Decodes a template code into its bits.
    pub fn new(code: &str) -> Result<Self, TemplateError> {
        if code.is_empty() {
            return Err(TemplateError::Empty);
        }

        let mut bits = Vec::with_capacity(code.len() * 6);
        for (index, character) in code.chars().enumerate() {
            let value =
                value_of(character).ok_or(TemplateError::BadCharacter { character, index })?;
            for bit in 0..6 {
                bits.push((value >> bit) & 1 == 1);
            }
        }

        Ok(BitReader { bits, position: 0 })
    }

    /// Reads `count` bits, lowest bit first.
    pub fn read(&mut self, count: u32, field: &'static str) -> Result<u32, TemplateError> {
        let available = self.remaining();
        if count > available {
            return Err(TemplateError::Truncated {
                field,
                wanted: count,
                available,
            });
        }

        let mut value = 0u32;
        for bit in 0..count {
            if self.bits[self.position] {
                value |= 1 << bit;
            }
            self.position += 1;
        }
        Ok(value)
    }

    /// How many bits are left.
    pub fn remaining(&self) -> u32 {
        (self.bits.len() - self.position) as u32
    }

    /// Whether everything left is zero.
    ///
    /// A well-formed code ends with zero padding, so anything else means the
    /// code was built wrong or read wrong.
    pub fn rest_is_zero(&self) -> bool {
        self.bits[self.position..].iter().all(|bit| !*bit)
    }

    /// How many bits the code holds in total.
    pub fn len(&self) -> usize {
        self.bits.len()
    }

    /// Whether the code holds no bits.
    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }
}

/// Builds a template code bit by bit.
#[derive(Debug, Default)]
pub struct BitWriter {
    bits: Vec<bool>,
}

impl BitWriter {
    /// A new, empty writer.
    pub fn new() -> Self {
        BitWriter::default()
    }

    /// Writes `count` bits of `value`, lowest bit first.
    ///
    /// Bits above `count` are dropped. Callers check ranges before writing;
    /// this is the low-level layer.
    pub fn write(&mut self, value: u32, count: u32) {
        for bit in 0..count {
            self.bits.push((value >> bit) & 1 == 1);
        }
    }

    /// How many bits have been written.
    pub fn len(&self) -> usize {
        self.bits.len()
    }

    /// Whether nothing has been written.
    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    /// Finishes the code, padding the last character with zero bits.
    pub fn finish(self) -> String {
        let mut code = String::with_capacity(self.bits.len().div_ceil(6));
        for chunk in self.bits.chunks(6) {
            let mut value = 0u8;
            for (bit, set) in chunk.iter().enumerate() {
                if *set {
                    value |= 1 << bit;
                }
            }
            code.push(character_of(value));
        }
        code
    }
}

/// The number of bits needed to hold a value.
///
/// Zero needs one bit, not none: a field holding zero still has to be written.
pub fn bits_needed(value: u32) -> u32 {
    match value {
        0 => 1,
        _ => u32::BITS - value.leading_zeros(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_alphabet_is_standard_base64() {
        assert_eq!(ALPHABET.len(), 64);
        assert_eq!(ALPHABET[0], b'A');
        assert_eq!(ALPHABET[25], b'Z');
        assert_eq!(ALPHABET[26], b'a');
        assert_eq!(ALPHABET[51], b'z');
        assert_eq!(ALPHABET[52], b'0');
        assert_eq!(ALPHABET[61], b'9');
        assert_eq!(ALPHABET[62], b'+');
        assert_eq!(ALPHABET[63], b'/');
    }

    #[test]
    fn a_character_decodes_to_its_bits_lowest_first() {
        // 'B' is value 1, so the first bit read is 1 and the rest are 0.
        let mut reader = BitReader::new("B").unwrap();
        assert_eq!(reader.read(1, "test").unwrap(), 1);
        assert_eq!(reader.read(5, "test").unwrap(), 0);

        // 'C' is value 2: 0 then 1.
        let mut reader = BitReader::new("C").unwrap();
        assert_eq!(reader.read(1, "test").unwrap(), 0);
        assert_eq!(reader.read(1, "test").unwrap(), 1);

        // 'P' is value 15: four 1s then two 0s.
        let mut reader = BitReader::new("P").unwrap();
        assert_eq!(reader.read(4, "test").unwrap(), 15);
        assert_eq!(reader.read(2, "test").unwrap(), 0);

        // '/' is value 63: all six bits set.
        let mut reader = BitReader::new("/").unwrap();
        assert_eq!(reader.read(6, "test").unwrap(), 63);
    }

    #[test]
    fn the_first_nibble_of_a_skill_template_is_fourteen() {
        // Every real skill code starts with 'O', which is value 14.
        let mut reader = BitReader::new("OQBTAUBPQaJ4EY6x0BAAAAAAuE").unwrap();
        assert_eq!(reader.read(4, "type").unwrap(), 14);
    }

    #[test]
    fn every_six_bit_value_round_trips() {
        // T1.3.2's done criterion.
        for value in 0..64u32 {
            let mut writer = BitWriter::new();
            writer.write(value, 6);
            let code = writer.finish();
            assert_eq!(code.chars().count(), 1);

            let mut reader = BitReader::new(&code).unwrap();
            assert_eq!(reader.read(6, "test").unwrap(), value, "value {value}");
        }
    }

    #[test]
    fn values_spanning_characters_round_trip() {
        // The case a per-character implementation would get wrong: a 12-bit
        // skill id straddles two characters.
        for value in [0u32, 1, 39, 979, 2416, 4095] {
            let mut writer = BitWriter::new();
            writer.write(value, 12);
            let code = writer.finish();

            let mut reader = BitReader::new(&code).unwrap();
            assert_eq!(reader.read(12, "test").unwrap(), value, "value {value}");
        }
    }

    #[test]
    fn a_sequence_of_odd_width_fields_round_trips() {
        let fields = [
            (14u32, 4u32),
            (0, 4),
            (0, 2),
            (5, 4),
            (0, 4),
            (3, 4),
            (1, 4),
        ];
        let mut writer = BitWriter::new();
        for (value, width) in fields {
            writer.write(value, width);
        }
        let code = writer.finish();

        let mut reader = BitReader::new(&code).unwrap();
        for (value, width) in fields {
            assert_eq!(reader.read(width, "test").unwrap(), value);
        }
    }

    #[test]
    fn an_invalid_character_says_where_it_is() {
        let error = BitReader::new("OQBT-AU").unwrap_err();
        assert_eq!(
            error,
            TemplateError::BadCharacter {
                character: '-',
                index: 4
            }
        );
        assert!(error.to_string().contains("character 4"));
    }

    #[test]
    fn an_empty_code_is_rejected() {
        assert_eq!(BitReader::new(""), Err(TemplateError::Empty));
    }

    #[test]
    fn running_out_of_bits_says_what_was_being_read() {
        let mut reader = BitReader::new("A").unwrap();
        let error = reader.read(12, "skill id").unwrap_err();
        assert_eq!(
            error,
            TemplateError::Truncated {
                field: "skill id",
                wanted: 12,
                available: 6
            }
        );
        assert!(error.to_string().contains("skill id"), "{error}");
        assert!(error.to_string().contains("cut short"), "{error}");
    }

    #[test]
    fn padding_is_zero_and_detectable() {
        let mut writer = BitWriter::new();
        writer.write(14, 4);
        let code = writer.finish();
        assert_eq!(code.chars().count(), 1);

        let mut reader = BitReader::new(&code).unwrap();
        assert_eq!(reader.read(4, "type").unwrap(), 14);
        assert_eq!(reader.remaining(), 2);
        assert!(reader.rest_is_zero());
    }

    #[test]
    fn bits_needed_counts_a_zero_as_one_bit() {
        // A field holding zero still has to be written, so zero needs a bit.
        assert_eq!(bits_needed(0), 1);
        assert_eq!(bits_needed(1), 1);
        assert_eq!(bits_needed(2), 2);
        assert_eq!(bits_needed(3), 2);
        assert_eq!(bits_needed(10), 4);
        assert_eq!(bits_needed(15), 4);
        assert_eq!(bits_needed(16), 5);
        assert_eq!(bits_needed(2416), 12);
        assert_eq!(bits_needed(4095), 12);
        assert_eq!(bits_needed(4096), 13);
    }

    #[test]
    fn writing_nothing_gives_an_empty_code() {
        assert_eq!(BitWriter::new().finish(), "");
        assert!(BitWriter::new().is_empty());
    }
}
