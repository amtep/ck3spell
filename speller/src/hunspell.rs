use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::fs::{read_to_string, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

mod affixdata;
mod parse_aff;
mod replacements;

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

impl WordInfo {
    fn is_forbidden(&self, ad: &AffixData) -> bool {
        self.flags.contains(&ad.forbidden)
    }

    fn need_affix(&self, ad: &AffixData) -> bool {
        match ad.need_affix {
            Some(flag) => self.flags.contains(&flag),
            None => false,
        }
    }

    fn only_in_compound(&self, ad: &AffixData) -> bool {
        match ad.only_in_compound {
            Some(flag) => self.flags.contains(&flag),
            None => false,
        }
    }

    fn has_flag(&self, flag: AffixFlag) -> bool {
        self.flags.contains(&flag)
    }
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
        for line in dict_text.lines().skip(1) {
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
                if dict.words.contains_key(word) {
                    // There is a use case for having two identical words
                    // with different affixes. We don't handle this yet.
                    bail!(format!("Duplicate word {}", word));
                }
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

    fn _user_dict_adder(&self, word: &str) -> Result<()> {
        if let Some(user_dict) = &self.user_dict {
            let mut file = OpenOptions::new().append(true).open(user_dict)?;
            file.write_all(word.as_bytes())?;
            file.write_all("\n".as_bytes())?;
        }
        Ok(())
    }

    fn is_numeric(word: &str) -> bool {
        // allow numbers with dots or commas
        // allow -- at the end and - at the front
        let word = word.strip_suffix("--").unwrap_or(word);
        let word = word.strip_prefix('-').unwrap_or(word);
        let mut seen_sep = false;
        for c in word.chars() {
            // TODO check for unicode number separators here
            if c == '.' || c == ',' {
                if seen_sep {
                    return false;
                }
                seen_sep = true;
            } else if c.is_digit(10) {
                seen_sep = false;
            } else {
                return false;
            }
        }
        true
    }
}

impl Speller for SpellerHunspellDict {
    fn spellcheck(&self, word: &str) -> bool {
        let word = self.affix_data.iconv.conv(word.trim());
        if word.len() == 0 || Self::is_numeric(&word) {
            return true;
        }
        if let Some(winfo) = self.words.get(&word) {
            return !winfo.is_forbidden(&self.affix_data)
                && !winfo.need_affix(&self.affix_data)
                && !winfo.only_in_compound(&self.affix_data);
        }
        for pfx in self.affix_data.prefixes.iter() {
            if pfx.check_prefix(&word, self) {
                return true;
            }
        }
        // TODO
        false
    }

    fn suggestions(&self, word: &str) -> Vec<String> {
        return Vec::new(); // TODO
    }

    fn add_word(&self, word: &str) -> bool {
        return false; // TODO
    }

    fn set_user_dict(&mut self, path: &Path) -> Result<i32> {
        if !path.exists() {
            File::create(path).with_context(|| {
                format!("Could not create {}", path.display())
            })?;
        }
        let dict = read_to_string(path)
            .with_context(|| format!("Could not read {}", path.display()))?;

        self.user_dict = Some(path.to_path_buf());

        let mut added = 0;
        for word in dict.lines() {
            if self.add_word(word) {
                added += 1;
            }
        }
        Ok(added)
    }

    fn add_word_to_user_dict(&self, word: &str) -> Result<bool> {
        if !self.add_word(word) {
            return Ok(false);
        }

        if let Some(user_dict) = &self.user_dict {
            self._user_dict_adder(word).with_context(|| {
                format!("Could not append to {}", user_dict.display())
            })?;
        }
        Ok(true)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_is_numeric() {
        assert_eq!(true, SpellerHunspellDict::is_numeric("54"));
        assert_eq!(true, SpellerHunspellDict::is_numeric("-1,000.00"));
        assert_eq!(true, SpellerHunspellDict::is_numeric("-1,000.--"));
        assert_eq!(false, SpellerHunspellDict::is_numeric("1,ooo"));
        assert_eq!(false, SpellerHunspellDict::is_numeric("100,,000"));
        assert_eq!(false, SpellerHunspellDict::is_numeric(".."));
    }
}
