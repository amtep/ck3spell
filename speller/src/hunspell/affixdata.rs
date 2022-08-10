use anyhow::{bail, Result};
use fnv::FnvHashMap;
use itertools::Itertools;
use std::num::ParseIntError;
use unicode_titlecase::StrTitleCase;

use crate::affix_trie::{PrefixTrie, SuffixTrie};
use crate::hunspell::compoundrule::CompoundRule;
use crate::hunspell::condition::AffixCondition;
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
    inner: FnvHashMap<WordFlags, AffixFlag>,
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
        self.all
            .intersects(WordFlags::CompoundBegin | WordFlags::CompoundFlag)
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

    /// Is this guessed to be a language where words are combined with dashes?
    pub dash_word_heuristic: bool,

    /// Cache. Maps affix flags to the suffix entries that have that flag
    /// as a continuation flag.
    rev_cont: FnvHashMap<AffixFlag, Vec<usize>>,

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
            max_ngram_suggestions: 4,
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
        self.dash_word_heuristic = if let Some(try_string) = &self.try_string {
            try_string.contains('_')
                || try_string.contains(|c: char| c.is_ascii_alphabetic())
        } else {
            false
        };
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

    pub fn generate_words_from_root(
        &self,
        root: &str,
        dict: &SpellerHunspellDict,
        mut suggest: impl FnMut(&str),
    ) {
        let caps = CapStyle::from_str(root);
        for winfo in dict.word_iter(root) {
            // First try the root itself.
            if !winfo.word_flags.intersects(
                WordFlags::Forbidden
                    | WordFlags::NoSuggest
                    | WordFlags::OnlyInCompound
                    | WordFlags::NeedAffix,
            ) {
                suggest(root);
                break;
            }

            for pfx in self.prefixes.iter() {
                if winfo.has_affix_flag(pfx.flag) {
                    pfx.try_prefix(root, winfo, caps, dict, &mut suggest);
                }
            }

            for sfx in self.suffixes.iter() {
                if winfo.has_affix_flag(sfx.flag) {
                    sfx.try_suffix(
                        root,
                        winfo,
                        caps,
                        dict,
                        &mut suggest,
                        false,
                    );
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct AffixEntry {
    allow_cross: bool,
    flag: AffixFlag,
    strip: String,
    affix: String,
    condition: AffixCondition,
    contflags: WordInfo,

    // All caps and titlecase versions of the affix. Saved here for speed.
    capsed_affix: String,
    titled_affix: String,

    // see discussion in AffixEntry::new
    pruned_condition: AffixCondition,
}

impl AffixEntry {
    pub fn new(
        is_pfx: bool,
        cross: bool,
        flag: AffixFlag,
        strip: &str,
        affix: &str,
        cond: &str,
        cflags: Vec<AffixFlag>,
    ) -> Self {
        let condition = AffixCondition::new(cond);
        // Since we apply the affix backward (removing `affix` and adding
        // `strip`), there's no need to keep checking the affix's condition
        // against its own strip characters. So prune those from the condition,
        // so that we can check directly against the root of the word for speed.
        let mut pruned = condition.clone();
        if is_pfx {
            pruned.prune_prefix(strip);
        } else {
            pruned.prune_suffix(strip);
        }
        // Not all special flags may be known yet, so leave WordFlags empty.
        AffixEntry {
            allow_cross: cross,
            flag,
            strip: strip.to_string(),
            affix: affix.to_string(),
            condition,
            contflags: WordInfo::new(WordFlags::empty(), cflags),

            capsed_affix: affix.to_uppercase(),
            titled_affix: affix.to_titlecase(),

            pruned_condition: pruned,
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
            if (!root.is_empty() || dict.affix_data.fullstrip)
                && self.pruned_condition.prefix_match(root)
            {
                let mut pword =
                    String::with_capacity(self.strip.len() + root.len());
                pword.push_str(&self.strip);
                pword.push_str(root);
                return Some(pword);
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
            if (!root.is_empty() || dict.affix_data.fullstrip)
                && self.pruned_condition.suffix_match(root)
            {
                let mut sword =
                    String::with_capacity(root.len() + self.strip.len());
                sword.push_str(root);
                sword.push_str(&self.strip);
                return Some(sword);
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
                for winfo in dict.word_iter_fold(&pword, caps) {
                    if winfo.has_affix_flag(self.flag)
                        && compound.word_ok(
                            winfo.word_flags | self.contflags.word_flags,
                        )
                        && !winfo
                            .word_flags
                            .intersects(WordFlags::Forbidden | caps.keepcase())
                    {
                        return true;
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
                for winfo in dict.word_iter_fold(&sword, caps) {
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
                            .intersects(WordFlags::Forbidden | caps.keepcase())
                    {
                        return true;
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

    fn try_prefix(
        &self,
        root: &str,
        winfo: &WordInfo,
        caps: CapStyle,
        dict: &SpellerHunspellDict,
        suggest: &mut impl FnMut(&str),
    ) {
        if !self.condition.prefix_match(root) {
            return;
        }
        if let Some(stripped) = root.strip_prefix(&self.strip) {
            let mut word =
                String::with_capacity(self.affix.len() + stripped.len());
            word.push_str(&self.affix);
            word.push_str(stripped);
            suggest(&word);

            if self.allow_cross {
                for sfx in dict.affix_data.suffixes.iter() {
                    if winfo.has_affix_flag(sfx.flag) {
                        sfx.try_suffix(
                            &word, winfo, caps, dict, suggest, false,
                        );
                    }
                }
            }
        }
    }

    fn try_suffix(
        &self,
        root: &str,
        winfo: &WordInfo,
        caps: CapStyle,
        dict: &SpellerHunspellDict,
        suggest: &mut impl FnMut(&str),
        from_suffix: bool,
    ) {
        if !self.condition.suffix_match(root) {
            return;
        }
        if let Some(stripped) = root.strip_suffix(&self.strip) {
            let mut word =
                String::with_capacity(self.affix.len() + stripped.len());
            word.push_str(stripped);
            word.push_str(&self.affix);
            suggest(&word);

            if !from_suffix && !self.contflags.affix_flags.is_empty() {
                for sfx2 in dict.affix_data.suffixes.iter() {
                    if self.contflags.affix_flags.contains(&sfx2.flag) {
                        sfx2.try_suffix(
                            &word, winfo, caps, dict, suggest, false,
                        );
                    }
                }
            }
        }
    }
}
