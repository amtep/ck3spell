use anyhow::{bail, Result};
use itertools::Itertools;
use std::num::ParseIntError;

use crate::hunspell::replacements::Replacements;
use crate::hunspell::SpellerHunspellDict;

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
    pub prefixes: Vec<AffixEntry>,
    /// Affixes that can be applied to the end of a word
    pub suffixes: Vec<AffixEntry>,
    /// Replacements to try when suggesting words
    pub replacements: Replacements,
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
    pub iconv: Replacements,
    /// Characters that should be converted after matching.
    pub oconv: Replacements,
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
    flag: AffixFlag,
    strip: String,
    affix: String,
    condition: String,
    cond_chars: usize,
}

impl AffixEntry {
    pub fn new(
        cross: bool,
        flag: AffixFlag,
        strip: &str,
        affix: &str,
        cond: &str,
    ) -> Self {
        AffixEntry {
            allow_cross: cross,
            flag,
            strip: strip.to_string(),
            affix: affix.to_string(),
            condition: cond.to_string(),
            cond_chars: _count_cond_chars(cond),
        }
    }

    pub fn check_prefix(&self, word: &str, dict: &SpellerHunspellDict) -> bool {
        if let Some(root) = word.strip_prefix(&self.affix) {
            if root.len() > 0 || dict.affix_data.fullstrip {
                let pword = self.strip.clone() + root;
                if self._prefix_condition(&pword) {
                    if let Some(winfo) = dict.words.get(&pword) {
                        if winfo.has_flag(self.flag)
                            && !winfo.is_forbidden(&dict.affix_data)
                        {
                            return true;
                        }
                    }
                }
                // TODO: check combination with suffixes, if allow_cross
            }
        }
        false
    }

    pub fn check_suffix(&self, word: &str, dict: &SpellerHunspellDict) -> bool {
        if let Some(root) = word.strip_suffix(&self.affix) {
            if root.len() > 0 || dict.affix_data.fullstrip {
                let sword = root.to_string() + &self.strip;
                if self._suffix_condition(&sword) {
                    if let Some(winfo) = dict.words.get(&sword) {
                        if winfo.has_flag(self.flag)
                            && !winfo.is_forbidden(&dict.affix_data)
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn _prefix_condition(&self, word: &str) -> bool {
        let count = word.chars().count();
        let witer = word.chars();
        count >= self.cond_chars && _test_condition(&self.condition, witer)
    }

    fn _suffix_condition(&self, word: &str) -> bool {
        let count = word.chars().count();
        if count >= self.cond_chars {
            let witer = word.chars().skip(count - self.cond_chars);
            return _test_condition(&self.condition, witer);
        }
        return false;
    }
}

fn _count_cond_chars(cond: &str) -> usize {
    let mut ingroup = false;
    let mut count = 0;
    for c in cond.chars() {
        if ingroup {
            if c == ']' {
                ingroup = false;
            }
        } else {
            count += 1;
            if c == '[' {
                ingroup = true;
            }
        }
    }
    count
}

enum CondState {
    Matching,
    GroupStart,
    InGroup,
    InGroupFound,
    InNegatedGroup,
}

/// Takes a rudimentary regexp (containing [] groups and [^] negated groups)
/// and matches it against the given word.
fn _test_condition(cond: &str, mut witer: impl Iterator<Item = char>) -> bool {
    let mut state = CondState::Matching;
    let mut wc = witer.next();
    for c in cond.chars() {
        if wc.is_none() {
            // word is too short to match
            return false;
        }
        match state {
            CondState::Matching => {
                if c == '[' {
                    state = CondState::GroupStart;
                } else if wc != Some(c) {
                    return false;
                } else {
                    wc = witer.next();
                }
            }
            CondState::GroupStart => {
                if c == '^' {
                    state = CondState::InNegatedGroup;
                } else if wc == Some(c) {
                    state = CondState::InGroupFound;
                } else {
                    state = CondState::InGroup;
                }
            }
            CondState::InGroup => {
                if c == ']' {
                    // No group member found
                    return false;
                } else if wc == Some(c) {
                    state = CondState::InGroupFound;
                }
            }
            CondState::InGroupFound => {
                if c == ']' {
                    state = CondState::Matching;
                    wc = witer.next();
                }
            }
            CondState::InNegatedGroup => {
                if c == ']' {
                    state = CondState::Matching;
                    wc = witer.next();
                } else if wc == Some(c) {
                    // hit negated char
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod test {
    use super::*;

    fn help_prefix_condition(cond: &str, word: &str) -> bool {
        let affix = AffixEntry::new(false, 0, "", "", cond);
        affix._prefix_condition(word)
    }

    fn help_suffix_condition(cond: &str, word: &str) -> bool {
        let affix = AffixEntry::new(false, 0, "", "", cond);
        affix._suffix_condition(word)
    }

    #[test]
    fn test_prefix_condition() {
        assert!(help_prefix_condition("", "anything"));
        assert!(help_prefix_condition("[aeoui]", "a vowel"));
        assert!(help_prefix_condition("[^hx]", "a negation"));
        assert!(help_prefix_condition("literal", "literal matching"));
        assert!(help_prefix_condition("l[ix]", "li"));
        assert!(help_prefix_condition("c[om]pli[^ca]ted", "cmplixted"));
        // a caret not at the start of a group is a normal member;
        assert!(help_prefix_condition("[ae^oui]", "^ vowel"));
        // test rejections too;
        assert!(!help_prefix_condition("[^hx]", "h fails"));
        assert!(!help_prefix_condition("literal", "litteral"));
        assert!(!help_prefix_condition("c[om]pli[^ca]t", "cmplict"));
    }

    #[test]
    fn test_suffix_condition() {
        assert!(help_suffix_condition("", "anything"));
        assert!(help_suffix_condition("[aeoui]", "vowel a"));
        assert!(help_suffix_condition("[^hx]", "negation a"));
        assert!(help_suffix_condition("literal", "matching literal"));
        assert!(help_suffix_condition("l[ix]", "li"));
        assert!(help_suffix_condition("c[om]pli[^ca]ted", "cmplixted"));
        assert!(help_suffix_condition("c[om]pli[^ca]ted", "very cmplixted"));
        // a caret not at the start of a group is a normal member;
        assert!(help_suffix_condition("[ae^oui]", "vowel ^"));
        // test rejections too;
        assert!(!help_suffix_condition("[^hx]", "fails h"));
        assert!(!help_suffix_condition("literal", "litteral"));
        assert!(!help_suffix_condition("c[om]pli[^ca]t", "very cmplict"));
    }
}
