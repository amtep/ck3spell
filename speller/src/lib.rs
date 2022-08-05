#![warn(missing_debug_implementations)]

use anyhow::Result;
use std::path::Path;

mod hunspell;

pub use crate::hunspell::SpellerHunspellDict;

pub trait Speller {
    /// Returns true if the word is in the dictionary, otherwise false.
    fn spellcheck(&self, word: &str) -> bool;

    /// Returns a list of possible corrections to a misspelled word.
    /// The list may be empty.
    fn suggestions(&self, word: &str, max: usize) -> Vec<String>;

    /// Accept `word` into the dictionary.
    /// Returns false if the word could not be accepted (for example
    /// if it contained characters the dictionary can't handle),
    /// otherwise returns true.
    fn add_word(&mut self, word: &str) -> bool;

    /// Load words from `path` (one word per line), and in the future
    /// append words to that file when `add_word_to_user_dict` is called.
    /// The file is created if it does not exist yet.
    /// Returns the number of words loaded from the file.
    fn set_user_dict(&mut self, path: &Path) -> Result<i32>;

    /// Accept `word` into the dictionary and add it to the user dict file
    /// that was set with `set_user_dict`.
    fn add_word_to_user_dict(&mut self, word: &str) -> Result<bool>;
}
