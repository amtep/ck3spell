use anyhow::{bail, Result};
use itertools::Itertools;
use std::num::ParseIntError;

/// Represents the format of the flags after words in the dictionary file.
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
}

impl AffixData {
    pub fn new() -> Self {
        AffixData {
            flag_mode: FlagMode::CharFlags,
            forbidden: DEFAULT_FORBIDDEN,
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
