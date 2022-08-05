use anyhow::{bail, Result};
use itertools::Itertools;
use std::collections::HashMap;
use std::num::ParseIntError;
use unicode_titlecase::StrTitleCase;

use crate::affix_trie::{PrefixTrie, SuffixTrie};
use crate::hunspell::compoundrule::CompoundRule;
use crate::hunspell::replacements::Replacements;
use crate::hunspell::wordflags::WordFlags;
use crate::hunspell::{CapStyle, Compound, SpellerHunspellDict, WordInfo};

/// Represents the format of the flags after words in the dictionary file.
#[derive(Clone, Copy, Debug, Default)]
pub enum FlagMode {
    /// Single-character flags
    #[default]
    Char,
    /// Two-character flags
    DoubleChar,
    /// Flags are comma-separated ASCII integers
    Numeric,
    /// Flags are Unicode codepoints in UTF-8 format
    Utf8,
}

#[derive(Clone, Debug, Default)]
pub struct SpecialFlags {
    inner: HashMap<WordFlags, AffixFlag>,
    all: WordFlags,
}

impl SpecialFlags {
    pub fn word_flags(&self, affix_flags: &[AffixFlag]) -> WordFlags {
        let mut word_flags = WordFlags::empty();
        for flag in affix_flags.iter() {
            for (wf, af) in self.inner.iter() {
                if flag == af {
                    word_flags.insert(*wf);
                }
            }
        }
        word_flags
    }

    pub fn insert(&mut self, wf: WordFlags, af: AffixFlag) {
        self.inner.insert(wf, af);
        self.all.insert(wf);
    }

    pub fn has_compounds(&self) -> bool {
        self.all.intersects(WordFlags::CompoundBegin)
    }
}

pub type AffixFlag = u32;

#[derive(Clone, Debug, Default)]
pub struct AffixData {
    /// Affixes that can be applied to the front of a word
    pub prefixes: Vec<AffixEntry>,
    /// Affixes that can be applied to the end of a word
    pub suffixes: Vec<AffixEntry>,
    /// Replacements to try when suggesting words
    pub replacements: Replacements,
    /// The valid formats for flags used in this affix file
    pub flag_mode: FlagMode,
    /// Known special word flags
    pub special_flags: SpecialFlags,
    /// keyboard layout, used to suggest spelling fixes.
    pub keyboard_string: Option<String>,
    /// letters to try when suggesting fixes, from common to rare.
    pub try_string: Option<String>,
    /// extra letters that may be part of words.
    pub extra_word_string: Option<String>,
    /// The minimum length of words in compound words.
    pub compound_min: u8,
    /// Limit to ngram suggestions in suggestion list
    pub max_ngram_suggestions: u8,
    /// Characters that should be converted before matching.
    pub iconv: Replacements,
    /// Characters that should be converted after matching.
    pub oconv: Replacements,
    /// Pre-programmed valid patterns for compound words
    pub compound_rules: Vec<CompoundRule>,
    /// Groups of related characters,
    pub related_chars: Vec<String>,
    /// Try to split words at these characters.
    pub word_breaks: Vec<String>,
    /// Allow affixes to completely remove a root
    pub fullstrip: bool,
    /// Not sure what this does. Used by German.
    pub check_sharps: bool,
    /// Any errors reported by the .aff file parser
    pub errors: Vec<String>,

    /// Cache. Maps affix flags to the suffix entries that have that flag
    /// as a continuation flag.
    rev_cont: HashMap<AffixFlag, Vec<usize>>,

    /// Cache. Maps suffixes to the suffix entries that add that suffix.
    rev_suffix: SuffixTrie<usize>,
    /// Cache. All-caps version of `rev_suffix`.
    rev_suffix_capsed: SuffixTrie<usize>,

    /// Cache. Maps prefixes to the prefix entries that add that prefix.
    rev_prefix: PrefixTrie<usize>,
    /// Cache. All-caps version of `rev_prefix`.
    rev_prefix_capsed: PrefixTrie<usize>,
    /// Cache. Titlecase version of `rev_prefix`.
    rev_prefix_titled: PrefixTrie<usize>,
}

impl AffixData {
    pub fn new() -> Self {
        AffixData {
            flag_mode: FlagMode::Char,
            compound_min: 3,
            ..Default::default()
        }
    }

    pub fn parse_flags(&self, flags: &str) -> Result<Vec<AffixFlag>> {
        match self.flag_mode {
            FlagMode::Char | FlagMode::Utf8 => {
                Ok(flags.chars().map(|c| c as u32).collect())
            }
            FlagMode::DoubleChar => flags
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
            FlagMode::Numeric => flags
                .split(',')
                .map(|d| d.parse::<u32>())
                .collect::<Result<Vec<AffixFlag>, ParseIntError>>()
                .map_err(anyhow::Error::from),
        }
    }

    fn recalc_rev_cont(&mut self) {
        self.rev_cont.clear();
        for (i, sfx) in self.suffixes.iter().enumerate() {
            for af in sfx.contflags.affix_flags.iter() {
                self.rev_cont.entry(*af).or_default().push(i);
            }
        }
    }

    fn recalc_rev_suffix(&mut self) {
        self.rev_suffix.clear();
        self.rev_suffix_capsed.clear();
        for (i, sfx) in self.suffixes.iter().enumerate() {
            self.rev_suffix.insert(&sfx.affix, i);
            self.rev_suffix_capsed.insert(&sfx.capsed_affix, i);
        }
    }

    fn recalc_rev_prefix(&mut self) {
        self.rev_prefix.clear();
        self.rev_prefix_capsed.clear();
        self.rev_prefix_titled.clear();
        for (i, pfx) in self.prefixes.iter().enumerate() {
            self.rev_prefix.insert(&pfx.affix, i);
            self.rev_prefix_capsed.insert(&pfx.capsed_affix, i);
            self.rev_prefix_titled.insert(&pfx.titled_affix, i);
        }
    }

    pub fn finalize(&mut self) {
        for pfx in self.prefixes.iter_mut() {
            pfx.finalize(&self.special_flags);
        }
        for sfx in self.suffixes.iter_mut() {
            sfx.finalize(&self.special_flags);
        }
        self.recalc_rev_cont();
        self.recalc_rev_suffix();
        self.recalc_rev_prefix();
    }

    pub fn check_prefix(
        &self,
        word: &str,
        caps: CapStyle,
        compound: Compound,
        dict: &SpellerHunspellDict,
    ) -> bool {
        if caps == CapStyle::AllCaps {
            self.rev_prefix_capsed.lookup(word, |i| {
                self.prefixes[i].check_prefix(word, caps, compound, dict)
            })
        } else if caps == CapStyle::Capitalized {
            self.rev_prefix_titled.lookup(word, |i| {
                self.prefixes[i].check_prefix(word, caps, compound, dict)
            })
        } else {
            self.rev_prefix.lookup(word, |i| {
                self.prefixes[i].check_prefix(word, caps, compound, dict)
            })
        }
    }

    pub fn check_suffix(
        &self,
        word: &str,
        caps: CapStyle,
        compound: Compound,
        dict: &SpellerHunspellDict,
        from_prefix: Option<&AffixEntry>,
    ) -> bool {
        if caps == CapStyle::AllCaps {
            self.rev_suffix_capsed.lookup(word, |i| {
                self.suffixes[i].check_suffix(
                    word,
                    caps,
                    compound,
                    dict,
                    from_prefix,
                    false,
                )
            })
        } else {
            self.rev_suffix.lookup(word, |i| {
                self.suffixes[i].check_suffix(
                    word,
                    caps,
                    compound,
                    dict,
                    from_prefix,
                    false,
                )
            })
        }
    }
}

#[derive(Clone, Debug)]
pub struct AffixEntry {
    allow_cross: bool,
    flag: AffixFlag,
    strip: String,
    affix: String,
    condition: String,
    cond_chars: usize,
    contflags: WordInfo,

    // All caps and titlecase versions of the affix. Saved here for speed.
    capsed_affix: String,
    titled_affix: String,
}

impl AffixEntry {
    pub fn new(
        cross: bool,
        flag: AffixFlag,
        strip: &str,
        affix: &str,
        cond: &str,
        cflags: Vec<AffixFlag>,
    ) -> Self {
        // Not all special flags may be known yet, so leave WordFlags empty.
        AffixEntry {
            allow_cross: cross,
            flag,
            strip: strip.to_string(),
            affix: affix.to_string(),
            condition: cond.to_string(),
            cond_chars: _count_cond_chars(cond),
            contflags: WordInfo::new(WordFlags::empty(), cflags),

            capsed_affix: affix.to_uppercase(),
            titled_affix: affix.to_titlecase(),
        }
    }

    pub fn finalize(&mut self, sf: &SpecialFlags) {
        self.contflags.word_flags = sf.word_flags(&self.contflags.affix_flags);
    }

    fn _deprefixed_word(
        &self,
        word: &str,
        prefix: &str,
        dict: &SpellerHunspellDict,
    ) -> Option<String> {
        if let Some(root) = word.strip_prefix(prefix) {
            if !root.is_empty() || dict.affix_data.fullstrip {
                let mut pword =
                    String::with_capacity(self.strip.len() + root.len());
                pword.push_str(&self.strip);
                pword.push_str(root);
                if self._prefix_condition(&pword) {
                    return Some(pword);
                }
            }
        }
        None
    }

    fn deprefixed_word(
        &self,
        word: &str,
        caps: CapStyle,
        dict: &SpellerHunspellDict,
    ) -> Option<String> {
        if let Some(root) = self._deprefixed_word(word, &self.affix, dict) {
            return Some(root);
        } else if caps == CapStyle::AllCaps {
            if let Some(root) =
                self._deprefixed_word(word, &self.capsed_affix, dict)
            {
                return Some(root);
            }
        } else if caps == CapStyle::Capitalized {
            if let Some(root) =
                self._deprefixed_word(word, &self.titled_affix, dict)
            {
                return Some(root);
            }
        }
        None
    }

    fn _desuffixed_word(
        &self,
        word: &str,
        suffix: &str,
        dict: &SpellerHunspellDict,
    ) -> Option<String> {
        if let Some(root) = word.strip_suffix(suffix) {
            if !root.is_empty() || dict.affix_data.fullstrip {
                let mut sword =
                    String::with_capacity(root.len() + self.strip.len());
                sword.push_str(root);
                sword.push_str(&self.strip);
                if self._suffix_condition(&sword) {
                    return Some(sword);
                }
            }
        }
        None
    }

    fn desuffixed_word(
        &self,
        word: &str,
        caps: CapStyle,
        dict: &SpellerHunspellDict,
    ) -> Option<String> {
        if let Some(root) = self._desuffixed_word(word, &self.affix, dict) {
            return Some(root);
        } else if caps == CapStyle::AllCaps {
            if let Some(root) =
                self._desuffixed_word(word, &self.capsed_affix, dict)
            {
                return Some(root);
            }
        }
        None
    }

    pub fn check_prefix(
        &self,
        word: &str,
        caps: CapStyle,
        compound: Compound,
        dict: &SpellerHunspellDict,
    ) -> bool {
        if !compound.prefix_ok(self.contflags.word_flags) {
            return false;
        }
        if let Some(pword) = self.deprefixed_word(word, caps, dict) {
            if !self.contflags.needs_affix() {
                if let Some(homonyms) = dict.words.get(&pword) {
                    for winfo in homonyms.iter() {
                        if winfo.has_affix_flag(self.flag)
                            && compound.word_ok(
                                winfo.word_flags | self.contflags.word_flags,
                            )
                            && !winfo
                                .word_flags
                                .intersects(WordFlags::Forbidden)
                        {
                            return true;
                        }
                    }
                }
            }
            if self.allow_cross
                && dict.affix_data.check_suffix(
                    &pword,
                    caps,
                    compound,
                    dict,
                    Some(self),
                )
            {
                return true;
            }
        }
        false
    }

    pub fn check_suffix(
        &self,
        word: &str,
        caps: CapStyle,
        compound: Compound,
        dict: &SpellerHunspellDict,
        from_prefix: Option<&AffixEntry>,
        from_suffix: bool,
    ) -> bool {
        if !compound.suffix_ok(self.contflags.word_flags) {
            return false;
        }
        // Does this suffix itself need a further affix?
        // Check if the word has a second suffix, or a prefix that
        // does not itself have the NeedAffix flag.
        let mut needs_affix = self.contflags.needs_affix() && !from_suffix;
        if let Some(pfx) = from_prefix {
            if !self.allow_cross {
                return false;
            }
            needs_affix = needs_affix && pfx.contflags.needs_affix();
        }

        if let Some(sword) = self.desuffixed_word(word, caps, dict) {
            if !needs_affix {
                if let Some(homonyms) = dict.words.get(&sword) {
                    for winfo in homonyms.iter() {
                        let mut flags =
                            winfo.word_flags | self.contflags.word_flags;
                        if let Some(pfx) = from_prefix {
                            if !winfo.has_affix_flag(pfx.flag) {
                                continue;
                            }
                            flags.insert(pfx.contflags.word_flags);
                        }
                        if winfo.has_affix_flag(self.flag)
                            && compound.word_ok(flags)
                            && !winfo
                                .word_flags
                                .intersects(WordFlags::Forbidden)
                        {
                            return true;
                        }
                    }
                }
            }
            // Check if this suffix may be a continuation of another.
            // Requires the rev_cont cache to be up to date.
            if !from_suffix {
                if let Some(v) = dict.affix_data.rev_cont.get(&self.flag) {
                    for &i in v.iter() {
                        let sfx2 = &dict.affix_data.suffixes[i];
                        debug_assert!(sfx2
                            .contflags
                            .affix_flags
                            .contains(&self.flag));
                        if sfx2.check_suffix(
                            &sword, caps, compound, dict, None, true,
                        ) {
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
        false
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
                } else if c != '.' && wc != Some(c) {
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
        let affix = AffixEntry::new(false, 0, "", "", cond, Vec::new());
        affix._prefix_condition(word)
    }

    fn help_suffix_condition(cond: &str, word: &str) -> bool {
        let affix = AffixEntry::new(false, 0, "", "", cond, Vec::new());
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
        // a dot is a wildcard:
        assert!(help_prefix_condition("any.letter", "anylletter"));
        // but not in a group:
        assert!(!help_prefix_condition("any[.]letter", "anylletter"));
        assert!(help_prefix_condition("any[.]letter", "any.letter"));

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
