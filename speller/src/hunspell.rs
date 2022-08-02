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
mod suggestions;
mod wordflags;

use crate::hunspell::affixdata::{AffixData, AffixFlag};
use crate::hunspell::parse_aff::parse_affix_data;
use crate::hunspell::suggestions::{
    delete_char_suggestions, related_char_suggestions,
};
use crate::hunspell::wordflags::WordFlags;
use crate::Speller;

/// A limit on the recursive attempts to break a word at breakpoints such as -
const MAX_WORD_BREAK_ATTEMPTS: u16 = 1000;
/// A limit on the effort put into making related-character suggestions
const MAX_RELATED_CHAR_SUGGESTIONS: u32 = 1000;

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

#[derive(Clone, Copy, PartialEq)]
pub enum CapStyle {
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
                let word_flags =
                    dict.affix_data.special_flags.word_flags(&affix_flags);
                let winfo = WordInfo::new(word_flags, affix_flags);
                dict.words.entry(word.to_string()).or_default().push(winfo);
            }
        }
        // Ensure capitalized and all-caps versions of all words are in the
        // dictionary.
        // Any word might be capitalized at the beginning of a sentence,
        // and any phrase might be written in all caps for emphasis,
        // so those should all be detected as correctly spelled.
        let mut addvec = Vec::new();
        for (word, homonyms) in dict.words.iter() {
            for winfo in homonyms.iter() {
                // "forbidden" entries are case sensitive, so don't upcase them
                if !winfo.word_flags.contains(WordFlags::Forbidden) {
                    // Only add the upcased words if they are not themselves forbidden
                    let allcaps = word.to_uppercase();
                    if !dict.is_forbidden(&allcaps) {
                        addvec.push((allcaps, winfo.clone()));
                    }
                    let capitalized = word.to_titlecase();
                    if !dict.is_forbidden(&capitalized) {
                        addvec.push((capitalized, winfo.clone()));
                    }
                }
            }
        }
        // Ensure a stable result regardless of hash order above
        addvec.sort_by(|(a, _), (b, _)| b.cmp(a));
        for (word, winfo) in addvec.drain(..) {
            dict.words.entry(word).or_default().push(winfo);
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

    fn is_forbidden(&self, word: &str) -> bool {
        if let Some(homonyms) = self.words.get(word) {
            for winfo in homonyms.iter() {
                if !winfo.word_flags.contains(WordFlags::Forbidden) {
                    return false;
                }
            }
            return true;
        }
        false
    }

    /// Check a word against the dictionary and try affix combinations
    fn _spellcheck_affixes(&self, word: &str, caps: CapStyle) -> bool {
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
            if pfx.check_prefix(word, caps, self) {
                return true;
            }
        }
        for sfx in self.affix_data.suffixes.iter() {
            if sfx.check_suffix(word, caps, self, None, false) {
                return true;
            }
        }
        false
    }

    // Check a word against the dictionary and try word breaks and affixes
    fn _spellcheck(&self, word: &str, caps: CapStyle, count: &mut u16) -> bool {
        if *count > MAX_WORD_BREAK_ATTEMPTS {
            return false;
        }
        *count += 1;

        if self._spellcheck_affixes(word, caps) {
            return true;
        }

        // break patterns may be anchored with ^ or $
        // Try those first.
        for brk in self.affix_data.word_breaks.iter() {
            if let Some(brk) = brk.strip_prefix('^') {
                if let Some(bword) = word.strip_prefix(brk) {
                    if self._spellcheck(bword, caps, count) {
                        return true;
                    }
                }
            } else if let Some(brk) = brk.strip_suffix('$') {
                if let Some(bword) = word.strip_suffix(brk) {
                    if self._spellcheck(bword, caps, count) {
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
                if self._spellcheck(worda, caps, count)
                    && self._spellcheck(wordb, caps, count)
                {
                    return true;
                }
            }
        }
        false
    }

    fn check_suggestion(
        &self,
        word: &str,
        origword: &str,
        suggs: &Vec<String>,
    ) -> bool {
        if word == origword || suggs.iter().any(|w| w == word) {
            return false;
        }

        if let Some(homonyms) = self.words.get(word) {
            for winfo in homonyms.iter() {
                if !winfo
                    .word_flags
                    .intersects(WordFlags::Forbidden | WordFlags::NoSuggest)
                {
                    return true;
                }
            }
        }

        let caps = CapStyle::from_str(word);
        let mut count = 0u16;
        if self._spellcheck(word, caps, &mut count) {
            return true;
        }

        // If the suggestion is two words, check both
        if let Some((worda, wordb)) = word.split_once(' ') {
            self.check_suggestion(worda, origword, suggs)
                && self.check_suggestion(wordb, origword, suggs)
        } else {
            false
        }
    }

    fn _add_word(&mut self, word: String, force: bool) {
        let homonyms = self.words.entry(word).or_default();
        if let Some(winfo) = homonyms.iter_mut().next() {
            if force {
                winfo.word_flags.remove(WordFlags::Forbidden);
            }
        } else {
            homonyms.push(WordInfo::default());
        }
    }
}

impl Speller for SpellerHunspellDict {
    fn spellcheck(&self, word: &str) -> bool {
        let word = self.affix_data.iconv.conv(word.trim());
        if word.is_empty() || Self::is_numeric(&word) {
            return true;
        }
        if self.is_forbidden(&word) {
            return false;
        }
        let caps = CapStyle::from_str(&word);
        let mut count = 0u16;
        self._spellcheck(&word, caps, &mut count)
    }

    fn suggestions(&self, word: &str, max: usize) -> Vec<String> {
        let word = self.affix_data.iconv.conv(word.trim());
        let mut suggs = Vec::default();
        if word.is_empty() || max == 0 {
            return suggs;
        }

        // Try lowercased, capitalized, or all caps
        // TODO: also match mixed case words, such as "ipod" -> "iPod"
        if self.check_suggestion(&word.to_lowercase(), &word, &suggs) {
            suggs.push(word.to_lowercase());
        } else if self.check_suggestion(
            &word.to_titlecase_lower_rest(),
            &word,
            &suggs,
        ) {
            suggs.push(word.to_titlecase_lower_rest());
        } else if self.check_suggestion(&word.to_uppercase(), &word, &suggs) {
            suggs.push(word.to_uppercase());
        }
        if suggs.len() == max {
            return suggs;
        }

        self.affix_data.replacements.suggest(&word, |sugg| {
            if self.check_suggestion(&sugg, &word, &suggs) {
                suggs.push(sugg);
            }
            suggs.len() < max
        });
        if suggs.len() == max {
            return suggs;
        }

        let mut count = 0u32;
        related_char_suggestions(
            &self.affix_data.related_chars,
            &word,
            |sugg| {
                if self.check_suggestion(&sugg, &word, &suggs) {
                    suggs.push(sugg.to_string());
                }
                count += 1;
                suggs.len() < max && count < MAX_RELATED_CHAR_SUGGESTIONS
            },
        );

        delete_char_suggestions(&word, |sugg| {
            if self.check_suggestion(&sugg, &word, &suggs) {
                suggs.push(sugg.to_string());
            }
            suggs.len() < max
        });

        suggs
    }

    fn add_word(&mut self, word: &str) -> bool {
        let word = self.affix_data.iconv.conv(word.trim());
        if word.is_empty() {
            return false;
        }

        match CapStyle::from_str(&word) {
            CapStyle::Lowercase => {
                self._add_word(word.to_uppercase(), false);
                self._add_word(word.to_titlecase(), false);
            }
            CapStyle::Capitalized | CapStyle::MixedCase => {
                self._add_word(word.to_uppercase(), false);
            }
            _ => (),
        }

        self._add_word(word, true);

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
