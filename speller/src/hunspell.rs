use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

mod affixdata;
mod parse_aff;

use crate::hunspell::affixdata::{AffixData, AffixFlag};
use crate::hunspell::parse_aff::parse_affix_data;
use crate::Speller;

/// A speller that loads Hunspell dictionaries
pub struct SpellerHunspellDict {
    affix_data: AffixData,
    words: HashMap<String, WordInfo>,
    user_dict: Option<PathBuf>,
}

struct WordInfo {
    flags: Vec<AffixFlag>,
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
        let mut dict = SpellerHunspellDict {
            affix_data,
            words: HashMap::new(),
            user_dict: None,
        };

        let dict_text = read_to_string(dictionary)
            .map_err(anyhow::Error::from)
            .with_context(|| {
                format!("Could not read words from {}", dictionary.display())
            })?;
        // Skip the first line because it's just the number of words
        for line in dict_text.lines().next() {
            if line.starts_with('\t') {
                // comment
                continue;
            }
            let (word, _morphs) = Self::split_morphological_fields(line);
            let (word, flagstr) = word.split_once('/').unwrap_or((word, ""));
            // If parsing the flags fails, just ignore them.
            // Printing errors isn't worth it.
            // TODO: maybe collect errors in the struct.
            let flags =
                dict.affix_data.parse_flags(flagstr).unwrap_or_default();
            let word = word.trim();
            if word.len() > 0 {
                dict.words.insert(word.to_string(), WordInfo { flags });
            }
        }
        Ok(dict)
    }

    fn split_morphological_fields(s: &str) -> (&str, Option<&str>) {
        // Parsing these is tricky because they are separated from the
        // word by a space, but the word may itself contain a space.
        // Parse them by recognizing the pattern xx:yyy with a two-char tag.
        let mut last_space = None;
        for (i, c) in s.char_indices() {
            if let Some(spos) = last_space {
                if i - spos <= 2 && !c.is_alphanumeric() {
                    last_space = None;
                } else if i - spos == 3 && c != ':' {
                    last_space = None;
                } else {
                    return (&s[..spos], Some(&s[spos + 1..].trim()));
                }
            } else if c == ' ' || c == '\t' {
                last_space = Some(i);
            }
        }
        (s, None)
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
