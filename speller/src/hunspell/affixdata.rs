use anyhow::{bail, Result};
use itertools::Itertools;
use std::collections::HashMap;
use std::num::ParseIntError;

/// Represents the format of the flags after words in the dictionary file.
#[derive(Clone, Copy, Default)]
pub enum FlagMode {
    /// Single-character flags
    #[default]
    CharFlags,
    /// Two-character flags
    DoubleCharFlags,
    /// Flags are comma-separated ASCII integers
    NumericFlags,
    /// Flags are Unicode codepoints in UTF-8 format
    Utf8Flags,
}

pub type AffixFlag = u32;

#[derive(Default)]
pub struct AffixData {
    /// Affixes that can be applied to the front of a word
    pub prefixes: HashMap<AffixFlag, Vec<AffixEntry>>,
    /// Affixes that can be applied to the end of a word
    pub suffixes: HashMap<AffixFlag, Vec<AffixEntry>>,
    /// Replacements to try when suggesting words
    pub replacements: Vec<(String, String)>,
    /// The valid formats for flags used in this affix file
    pub flag_mode: FlagMode,
    /// forbidden is the flag for invalid words.
    pub forbidden: AffixFlag,
    /// keyboard layout, used to suggest spelling fixes.
    pub keyboard_string: Option<String>,
    /// letters to try when suggesting fixes, from common to rare.
    pub try_string: Option<String>,
    /// extra letters that may be part of words.
    pub extra_word_string: Option<String>,
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
    /// The flag for words that should not change case.
    pub keep_case: Option<AffixFlag>,
    /// The minimum length of words in compound words.
    pub compound_min: u8,
    /// Characters that should be converted before matching.
    pub iconv: HashMap<char, char>,
    /// Characters that should be converted after matching.
    pub oconv: HashMap<char, char>,
    /// Not sure what these do.
    pub compound_rules: Vec<Vec<AffixFlag>>,
    /// Groups of related characters,
    pub related_chars: Vec<String>,
    /// Not sure what these do.
    pub word_breaks: Vec<String>,
    /// Allow affixes to completely remove a root
    pub fullstrip: bool,
    /// Not sure what this does. Used by German.
    pub check_sharps: bool,
}

impl AffixData {
    pub fn new() -> Self {
        AffixData {
            flag_mode: FlagMode::CharFlags,
            ..Default::default()
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

pub struct AffixEntry {
    allow_cross: bool,
    strip: String,
    affix: String,
    conditions: String,
}

impl AffixEntry {
    pub fn new(cross: bool, strip: &str, affix: &str, cond: &str) -> Self {
        AffixEntry {
            allow_cross: cross,
            strip: strip.to_string(),
            affix: affix.to_string(),
            conditions: cond.to_string(),
        }
    }
}
