use anyhow::{bail, Result};
use itertools::Itertools;
use std::num::ParseIntError;

/// Represents the format of the flags after words in the dictionary file.
#[derive(Clone, Copy)]
pub enum FlagMode {
    /// Single-character flags
    CharFlags,
    /// Two-character flags
    DoubleCharFlags,
    /// Flags are comma-separated ASCII integers
    NumericFlags,
    /// Flags are Unicode codepoints in UTF-8 format
    Utf8Flags,
}

pub type AffixFlag = u32;

const DEFAULT_FORBIDDEN: AffixFlag = 0x110000;

pub struct AffixData {
    pub flag_mode: FlagMode,
    /// forbidden is the flag for invalid words.
    pub forbidden: AffixFlag,
    /// keyboard layout, used to suggest spelling fixes.
    pub keyboard_string: Option<String>,
    /// letters to try when suggesting fixes, from common to rare.
    pub try_string: Option<String>,
    /// The flag for words that may appear at the beginning of compound words.
    pub compound_begin: Option<AffixFlag>,
    /// The flag for words that may appear as middle words in compound words.
    pub compound_middle: Option<AffixFlag>,
    /// The flag for words that may appear at the end of compound words.
    pub compound_end: Option<AffixFlag>,
    /// The flag for words that may have affixes inside compound words.
    pub compound_permit: Option<AffixFlag>,
    /// The flag for words that may appear only inside compound words.
    pub only_in_compound: Option<AffixFlag>,
    /// The flag for words that should not be suggested.
    pub no_suggest: Option<AffixFlag>,
    /// The flag for affixes that may surround a word.
    pub circumfix: Option<AffixFlag>,
    /// The flag for words that must have an affix.
    pub need_affix: Option<AffixFlag>,
    /// The minimum length of words in compound words.
    pub compound_min: u8,
}

impl AffixData {
    pub fn new() -> Self {
        AffixData {
            flag_mode: FlagMode::CharFlags,
            forbidden: DEFAULT_FORBIDDEN,
            keyboard_string: None,
            try_string: None,
            compound_begin: None,
            compound_middle: None,
            compound_end: None,
            compound_permit: None,
            only_in_compound: None,
            no_suggest: None,
            circumfix: None,
            need_affix: None,
            compound_min: 0,
        }
    }

    pub fn parse_flags(&self, flags: &str) -> Result<Vec<AffixFlag>> {
        match self.flag_mode {
            FlagMode::CharFlags | FlagMode::Utf8Flags => {
                Ok(flags.chars().map(|c| c as u32).collect())
            }
            FlagMode::DoubleCharFlags => flags
                .chars()
                .chunks(2)
                .into_iter()
                .map(|mut pair| {
                    let c1 = pair.next().unwrap() as u32;
                    let c2 = pair.next().unwrap() as u32;
                    if c1 > 255 || c2 > 255 {
                        bail!("Invalid characters in double flag");
                    }
                    Ok(c1 * 256 + c2)
                })
                .collect(),
            FlagMode::NumericFlags => flags
                .split(',')
                .map(|d| u32::from_str_radix(d, 10))
                .collect::<Result<Vec<AffixFlag>, ParseIntError>>()
                .map_err(anyhow::Error::from),
        }
    }
}
