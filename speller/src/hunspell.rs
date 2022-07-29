use anyhow::{Context, Result};
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use crate::hunspell_aff::{parse_affix_data, AffixData};
use crate::Speller;

/// A speller that loads Hunspell dictionaries
pub struct SpellerHunspellDict {
    affix_data: AffixData,
    user_dict: Option<PathBuf>,
}

impl SpellerHunspellDict {
    /// Returns a Speller that uses a Hunspell-format dictionary and affix file.
    pub fn new(dictionary: &Path, affixes: &Path) -> Result<Self> {
        let affixes_text = read_to_string(affixes)
            .map_err(anyhow::Error::from)
            .with_context(|| {
                format!("Could not read affix data from {}", affixes.display())
            })?;
        let affix_data = parse_affix_data(&affixes_text)?;
        SpellerHunspellDict {
            affix_data,
            user_dict: None,
        };
        todo!();
    }
}

impl Speller for SpellerHunspellDict {
    fn spellcheck(&self, word: &str) -> bool {
        return true;
        todo!();
    }

    fn suggestions(&self, word: &str) -> Vec<String> {
        return Vec::new();
        todo!();
    }

    fn add_word(&self, word: &str) -> bool {
        todo!();
    }

    fn set_user_dict(&mut self, path: &Path) -> Result<i32> {
        todo!();
    }

    fn add_word_to_user_dict(&self, word: &str) -> Result<bool> {
        todo!();
    }
}
