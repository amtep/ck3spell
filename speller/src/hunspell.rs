use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::Speller;

/// A speller that loads Hunspell dictionaries
pub struct SpellerHunspellDict {
    user_dict: Option<PathBuf>,
}

impl SpellerHunspellDict {
    /// Returns a Speller that uses a Hunspell-format dictionary and affix file.
    pub fn new(dictionary: &Path, affixes: &Path) -> Result<Self> {
        todo!();
    }

    /// Look for Hunspell-format dictionaries for the given `locale` in the
    /// provided directory search path. Return a tuple of paths to the
    /// dictionary file and the affix file.
    pub fn find_dictionary(search_path: Vec<& str>, locale: &str) -> Option<(PathBuf, PathBuf)> {
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
