use anyhow::{Context, Result};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::fs::{read_to_string, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use unicode_titlecase::StrTitleCase;

mod affixdata;
mod parse_aff;
mod replacements;
mod wordflags;

use crate::hunspell::affixdata::{AffixData, AffixFlag};
use crate::hunspell::parse_aff::parse_affix_data;
use crate::hunspell::wordflags::WordFlags;
use crate::Speller;

/// A limit on the recursive attempts to break a word at breakpoints such as -
const MAX_WORD_BREAK_ATTEMPTS: u16 = 1000;

/// A speller that loads Hunspell dictionaries
pub struct SpellerHunspellDict {
    affix_data: AffixData,
    words: HashMap<String, SmallVec<[WordInfo; 1]>>,
    user_dict: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct WordInfo {
    word_flags: WordFlags,
    affix_flags: Vec<AffixFlag>,
}

impl WordInfo {
    fn new(word_flags: WordFlags, affix_flags: Vec<AffixFlag>) -> Self {
        WordInfo {
            word_flags,
            affix_flags,
        }
    }

    fn has_affix_flag(&self, flag: AffixFlag) -> bool {
        self.affix_flags.contains(&flag)
    }
}

enum CapStyle {
    Lowercase,
    Capitalized,
    AllCaps,
    MixedCase,
    Neutral,
}

impl CapStyle {
    fn from_str(word: &str) -> Self {
        let lcase = word == word.to_lowercase();
        let ucase = word == word.to_uppercase();
        if lcase && ucase {
            CapStyle::Neutral
        } else if lcase {
            CapStyle::Lowercase
        } else if ucase {
            CapStyle::AllCaps
        } else {
            for c in word.chars().skip(1) {
                if c.is_uppercase() {
                    return CapStyle::MixedCase;
                }
            }
            CapStyle::Capitalized
        }
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
            let affix_flags =
                dict.affix_data.parse_flags(flagstr).unwrap_or_default();
            let word = word.trim();
            if !word.is_empty() {
                let mut word_flags = WordFlags::empty();
                for flag in affix_flags.iter() {
                    for (wf, af) in dict.affix_data.special_flags.iter() {
                        if flag == af {
                            word_flags.insert(*wf);
                        }
                    }
                }
                let winfo = WordInfo::new(word_flags, affix_flags);
                dict.words.entry(word.to_string()).or_default().push(winfo);
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
                if (i - spos <= 2 && !c.is_alphanumeric())
                    || (i - spos == 3 && c != ':')
                {
                    last_space = None;
                } else {
                    return (&s[..spos], Some(s[spos + 1..].trim()));
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
            } else if c.is_ascii_digit() {
                seen_sep = false;
            } else {
                return false;
            }
        }
        true
    }

    /// Check a word against the dictionary without changing its capitalization.
    fn _spellcheck_affixes(&self, word: &str) -> bool {
        if let Some(homonyms) = self.words.get(word) {
            for winfo in homonyms.iter() {
                if !winfo.word_flags.intersects(
                    WordFlags::Forbidden
                        | WordFlags::NeedAffix
                        | WordFlags::OnlyInCompound,
                ) {
                    return true;
                }
            }
        }
        for pfx in self.affix_data.prefixes.iter() {
            if pfx.check_prefix(word, self) {
                return true;
            }
        }
        for sfx in self.affix_data.suffixes.iter() {
            if sfx.check_suffix(word, self) {
                return true;
            }
        }
        false
    }

    fn _spellcheck(&self, word: &str, count: &mut u16) -> bool {
        if *count > MAX_WORD_BREAK_ATTEMPTS {
            return false;
        }
        *count += 1;

        if self._spellcheck_affixes(word) {
            return true;
        }

        // break patterns may be anchored with ^ or $
        // Try those first.
        for brk in self.affix_data.word_breaks.iter() {
            if let Some(brk) = brk.strip_prefix('^') {
                if let Some(bword) = word.strip_prefix(brk) {
                    if self._spellcheck(bword, count) {
                        return true;
                    }
                }
            } else if let Some(brk) = brk.strip_suffix('$') {
                if let Some(bword) = word.strip_suffix(brk) {
                    if self._spellcheck(bword, count) {
                        return true;
                    }
                }
            }
        }

        // Try breaking words into pieces.
        for brk in self.affix_data.word_breaks.iter() {
            if brk.starts_with('^') || brk.ends_with('$') {
                continue;
            }
            if let Some((worda, wordb)) = word.split_once(brk) {
                if self._spellcheck(worda, count)
                    && self._spellcheck(wordb, count)
                {
                    return true;
                }
            }
        }
        false
    }
}

impl Speller for SpellerHunspellDict {
    fn spellcheck(&self, word: &str) -> bool {
        let word = self.affix_data.iconv.conv(word.trim());
        if word.is_empty() || Self::is_numeric(&word) {
            return true;
        }
        let mut count = 0u16;
        if self._spellcheck(&word, &mut count) {
            return true;
        }
        count = 0;
        match CapStyle::from_str(&word) {
            CapStyle::AllCaps => {
                let mut count2 = 0u16;
                self._spellcheck(&word.to_titlecase_lower_rest(), &mut count)
                    || self._spellcheck(&word.to_lowercase(), &mut count2)
            }
            CapStyle::Capitalized => {
                self._spellcheck(&word.to_lowercase(), &mut count)
            }
            _ => false,
        }
    }

    fn suggestions(&self, _word: &str) -> Vec<String> {
        Vec::default() // TODO
    }

    fn add_word(&mut self, word: &str) -> bool {
        let word = self.affix_data.iconv.conv(word.trim());
        if word.is_empty() {
            return false;
        }
        let homonyms = self.words.entry(word).or_default();
        if let Some(winfo) = homonyms.iter_mut().next() {
            winfo.word_flags.remove(WordFlags::Forbidden);
        } else {
            homonyms.push(WordInfo::default());
        }
        true
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

    fn add_word_to_user_dict(&mut self, word: &str) -> Result<bool> {
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
