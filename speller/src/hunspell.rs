use anyhow::{Context, Result};
use caseless::default_case_fold_str;
use fnv::FnvHashMap;
use smallvec::SmallVec;
use std::fs::{read_to_string, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::CharIndices;
use unicode_casing::CharExt;
use unicode_titlecase::StrTitleCase;

mod affixdata;
mod compoundrule;
mod condition;
mod parse_aff;
mod replacements;
mod suggcollector;
mod suggestions;
mod wordflags;

use crate::hunspell::affixdata::{AffixData, AffixFlag};
use crate::hunspell::parse_aff::parse_affix_data;
use crate::hunspell::suggcollector::SuggCollector;
use crate::hunspell::suggestions::{
    add_char_suggestions, capitalize_char_suggestions, delete_char_suggestions,
    delete_doubled_pair_suggestions, delins_suggestions, move_char_suggestions,
    ngram_suggestions, related_char_suggestions, split_word_suggestions,
    split_word_with_dash_suggestions, swap_char_suggestions,
    wrong_key_suggestions,
};
use crate::hunspell::wordflags::WordFlags;
use crate::Speller;

/// A limit on the recursive attempts to break a word at breakpoints such as -
const MAX_WORD_BREAK_ATTEMPTS: u16 = 1000;

/// A speller that loads Hunspell dictionaries
#[derive(Clone, Debug)]
pub struct SpellerHunspellDict {
    affix_data: AffixData,
    user_dict: Option<PathBuf>,
    words: FnvHashMap<String, SmallVec<[WordInfo; 1]>>,
    // An index of case-folded words, to help with spell checking of
    // all-caps words and phrases. It combines all the WordInfo of the
    // original words, so that for example both "ROSE'S" (name) and
    // "ROSES" (flower) are valid in all caps.
    folded_words: FnvHashMap<String, SmallVec<[WordInfo; 1]>>,
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

    fn needs_affix(&self) -> bool {
        self.word_flags.intersects(WordFlags::NeedAffix)
    }
}

/// A word's place in the word compounding sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Compound {
    None,
    Begin,
    Middle,
    End,
}

impl Compound {
    /// This function gets the union of the root word's flags and the
    /// continuation flags of any prefix or suffix applied.
    fn word_ok(self, wf: WordFlags) -> bool {
        match self {
            Compound::None => !wf.intersects(WordFlags::OnlyInCompound),
            Compound::Begin => wf
                .intersects(WordFlags::CompoundBegin | WordFlags::CompoundFlag),
            Compound::Middle => wf.intersects(
                WordFlags::CompoundMiddle | WordFlags::CompoundFlag,
            ),
            Compound::End => {
                wf.intersects(WordFlags::CompoundEnd | WordFlags::CompoundFlag)
            }
        }
    }

    fn prefix_ok(self, wf: WordFlags) -> bool {
        match self {
            Compound::None => !wf.intersects(WordFlags::OnlyInCompound),
            Compound::Begin => true,
            Compound::Middle => wf.intersects(WordFlags::CompoundPermit),
            Compound::End => wf.intersects(WordFlags::CompoundPermit),
        }
    }

    fn suffix_ok(self, wf: WordFlags) -> bool {
        match self {
            Compound::None => !wf.intersects(WordFlags::OnlyInCompound),
            Compound::Begin => wf.intersects(WordFlags::CompoundPermit),
            Compound::Middle => wf.intersects(WordFlags::CompoundPermit),
            Compound::End => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapStyle {
    Lowercase,
    Capitalized,
    AllCaps,
    Mixed,
    Neutral,
    // Folded is a special category for words that have been run
    // through caseless::default_case_fold_str.
    Folded,
    // Decapitalized is a special category for Capitalized words that
    // have been lowercased.
    Decapitalized,
}

impl CapStyle {
    fn from_str(word: &str) -> Self {
        let mut iter = word.chars();
        let c1 = match iter.next() {
            Some(c1) => c1,
            None => {
                return CapStyle::Neutral;
            }
        };
        if c1.is_lowercase() {
            for c in iter {
                if c.is_uppercase() || c.is_titlecase() {
                    return CapStyle::Mixed;
                }
            }
            CapStyle::Lowercase
        } else if c1.is_uppercase() {
            let mut seen_ucase = false;
            let mut seen_lcase = false;
            for c in iter {
                if c.is_lowercase() {
                    seen_lcase = true;
                } else if c.is_uppercase() {
                    seen_ucase = true;
                } else if c.is_titlecase() {
                    return CapStyle::Mixed;
                }
            }
            if seen_ucase && seen_lcase {
                CapStyle::Mixed
            } else if seen_lcase {
                CapStyle::Capitalized
            } else {
                CapStyle::AllCaps
            }
        } else if c1.is_titlecase() {
            for c in iter {
                if c.is_uppercase() || c.is_titlecase() {
                    return CapStyle::Mixed;
                }
            }
            CapStyle::Capitalized
        } else {
            CapStyle::from_str(&word[c1.len_utf8()..])
        }
    }

    // Return the keepcase flag if KeepCase entries should be excluded
    // for this word.
    fn keepcase(&self) -> WordFlags {
        if *self == CapStyle::Decapitalized {
            WordFlags::KeepCase
        } else {
            WordFlags::empty()
        }
    }

    // Return whether suggestions should be checked with strict capitalization
    // or whether decapitalized and casefolded forms are ok.
    // If the original word was already capitalized, then permissive checking
    // is ok. Otherwise, capitalized suggestions should only be made if they
    // are in the dictionary that way.
    fn strict(self) -> StrictMode {
        match self {
            CapStyle::Lowercase | CapStyle::Neutral => StrictMode::Strict,
            CapStyle::Capitalized => StrictMode::AllowDecap,
            _ => StrictMode::AllowAll,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum StrictMode {
    Strict,
    AllowDecap,
    AllowAll,
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
            user_dict: None,
            words: FnvHashMap::default(),
            folded_words: FnvHashMap::default(),
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
                dict.words
                    .entry(word.to_string())
                    .or_default()
                    .push(winfo.clone());

                // Forbidden words are case sensitive, so don't add them
                // to the case-folded dictionary.
                if !winfo
                    .word_flags
                    .intersects(WordFlags::Forbidden | WordFlags::KeepCase)
                {
                    let folded = default_case_fold_str(word);
                    dict.folded_words.entry(folded).or_default().push(winfo);
                }
            }
        }

        Ok(dict)
    }

    pub fn get_errors(&self) -> Vec<String> {
        let mut v = Vec::new();
        for e in self.affix_data.errors.iter() {
            v.push(e.clone());
        }
        v
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
                } else if i - spos == 3 {
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
        let mut seen_digit = false;
        for c in word.chars() {
            // TODO check for unicode number separators here
            if c == '.' || c == ',' {
                if !seen_digit {
                    return false;
                }
                seen_digit = false;
            } else if c.is_ascii_digit() {
                seen_digit = true;
            } else {
                return false;
            }
        }
        true
    }

    fn is_forbidden(&self, word: &str) -> bool {
        let mut forbidden = false;
        for winfo in self.word_iter(word) {
            if winfo.word_flags.contains(WordFlags::Forbidden) {
                forbidden = true;
            } else {
                return false;
            }
        }
        forbidden
    }

    fn is_forbidden_suggestion(&self, word: &str) -> bool {
        let mut nosug = false;
        for winfo in self.word_iter(word) {
            if winfo
                .word_flags
                .intersects(WordFlags::Forbidden | WordFlags::NoSuggest)
            {
                nosug = true;
            } else {
                return false;
            }
        }
        nosug
    }

    fn has_word_pair_fold(
        &self,
        word1: &str,
        word2: &str,
        caps: CapStyle,
    ) -> bool {
        let mut word = String::with_capacity(word1.len() + 1 + word2.len());
        word.push_str(word1);
        word.push(' ');
        word.push_str(word2);
        for winfo in self.word_iter_fold(&word, caps) {
            if !winfo.word_flags.contains(WordFlags::Forbidden) {
                return true;
            }
        }
        false
    }

    fn word_iter(&self, word: &str) -> std::slice::Iter<'_, WordInfo> {
        if let Some(homonyms) = self.words.get(word) {
            homonyms.iter()
        } else {
            [].iter()
        }
    }

    fn word_iter_fold(
        &self,
        word: &str,
        caps: CapStyle,
    ) -> std::slice::Iter<'_, WordInfo> {
        if caps == CapStyle::Folded {
            if let Some(homonyms) = self.folded_words.get(word) {
                homonyms.iter()
            } else {
                [].iter()
            }
        } else {
            self.word_iter(word)
        }
    }

    fn has_affix_flag_fold(
        &self,
        word: &str,
        caps: CapStyle,
        flag: AffixFlag,
    ) -> bool {
        for winfo in self.word_iter_fold(word, caps) {
            if winfo.has_affix_flag(flag) {
                return true;
            }
        }
        false
    }

    /// Check a word against the dictionary and try affix combinations
    fn _spellcheck_affixes(
        &self,
        word: &str,
        caps: CapStyle,
        compound: Compound,
    ) -> bool {
        for winfo in self.word_iter_fold(word, caps) {
            if compound.word_ok(winfo.word_flags)
                && !winfo.word_flags.intersects(
                    WordFlags::Forbidden
                        | WordFlags::NeedAffix
                        | caps.keepcase(),
                )
            {
                return true;
            }
        }

        self.affix_data.check_prefix(word, caps, compound, self)
            || self
                .affix_data
                .check_suffix(word, caps, compound, self, None)
    }

    fn _spellcheck_compoundrule<'a>(
        &self,
        word: &'a str,
        caps: CapStyle,
        v: &mut Vec<&'a str>,
        mut iter: CharIndices,
    ) -> bool {
        let mut wlen = 0;
        let mut wstart = None;
        while let Some((i, c)) = iter.next() {
            if wstart.is_none() {
                wstart = Some(i);
            }
            wlen += 1;
            if wlen < self.affix_data.compound_min {
                continue;
            }
            let iafter = i + c.len_utf8();
            if wstart.unwrap() == 0 && iafter == word.len() {
                // If the "piece" is the whole word, then it's not compound.
                continue;
            }
            let piece = &word[wstart.unwrap()..iafter];
            if !self.words.contains_key(piece) {
                continue;
            }
            // Found a possible word piece.
            // Recurse to try the piece.
            v.push(piece);
            // Only try the piece if at least one rule would match these pieces.
            // This avoids a lot of backtracking for words that would never
            // work anyway.
            for rule in self.affix_data.compound_rules.iter() {
                if rule.partial_match(v, |word, flag| {
                    self.has_affix_flag_fold(word, caps, flag)
                }) {
                    if self._spellcheck_compoundrule(
                        word,
                        caps,
                        v,
                        iter.clone(),
                    ) {
                        return true;
                    }
                    break;
                }
            }
            // Then loop to try not using the piece.
            v.pop();
        }
        if wlen > 0 {
            // too-small leftover piece at the end
            return false;
        }
        for rule in self.affix_data.compound_rules.iter() {
            if rule.matches(v, |word, flag| {
                self.has_affix_flag_fold(word, caps, flag)
            }) {
                return true;
            }
        }
        false
    }

    /// This is similar to _spellcheck_compoundrule, but we don't check for
    /// specific word flags and we allow affixes.
    fn _spellcheck_compounding<'a>(
        &self,
        word: &'a str,
        caps: CapStyle,
        v: &mut Vec<&'a str>,
        mut iter: CharIndices,
    ) -> bool {
        let mut wlen = 0;
        let mut wstart = None;
        while let Some((i, c)) = iter.next() {
            if wstart.is_none() {
                wstart = Some(i);
            }
            wlen += 1;
            if wlen < self.affix_data.compound_min {
                continue;
            }
            let iafter = i + c.len_utf8();
            if wstart.unwrap() == 0 && iafter == word.len() {
                // If the "piece" is the whole word, then it's not compound.
                continue;
            }
            let piece = &word[wstart.unwrap()..iafter];
            let compound = if v.is_empty() {
                Compound::Begin
            } else if iafter == word.len() {
                Compound::End
            } else {
                Compound::Middle
            };
            let piece_caps = if (caps == CapStyle::Capitalized
                || caps == CapStyle::Decapitalized)
                && compound != Compound::Begin
            {
                CapStyle::Lowercase
            } else if caps == CapStyle::Mixed {
                CapStyle::from_str(piece)
            } else {
                caps
            };
            if !self._spellcheck_affixes(piece, piece_caps, compound) {
                continue;
            }
            // Found a possible word piece.
            // Recurse to try the piece.
            v.push(piece);
            if self._spellcheck_compounding(word, caps, v, iter.clone()) {
                return true;
            }
            // Then loop to try not using the piece.
            v.pop();
        }
        // Success if we exactly consumed `word`.
        // Also check a special case: if a word pair is in the dictionary
        // separated by a space, then don't accept it as a compound.
        wlen == 0
            && !(v.len() == 2 && self.has_word_pair_fold(v[0], v[1], caps))
    }

    /// Check a word against the dictionary and try compound words
    fn _spellcheck_compound(&self, word: &str, caps: CapStyle) -> bool {
        if self._spellcheck_affixes(word, caps, Compound::None) {
            return true;
        }

        // For COMPOUNDRULE, divide the word into pieces that are all
        // directly in the dictionary (no prefix/suffix processing).
        if !self.affix_data.compound_rules.is_empty()
            && self._spellcheck_compoundrule(
                word,
                caps,
                &mut Vec::new(),
                word.char_indices(),
            )
        {
            return true;
        }

        // Early return for dictionaries that don't support compounding.
        self.affix_data.special_flags.has_compounds()
            && self._spellcheck_compounding(
                word,
                caps,
                &mut Vec::new(),
                word.char_indices(),
            )
    }

    // Check a word against the dictionary and try word breaks and affixes
    fn _spellcheck(
        &self,
        word: &str,
        strict: StrictMode,
        count: &mut u16,
    ) -> bool {
        if Self::is_numeric(word) {
            return true;
        }

        let caps = CapStyle::from_str(word);
        if *count > MAX_WORD_BREAK_ATTEMPTS {
            return false;
        }
        *count += 1;

        if self._spellcheck_caps(word, caps, strict) {
            return true;
        }

        // break patterns may be anchored with ^ or $
        // Try those first.
        for brk in self.affix_data.word_breaks.iter() {
            if let Some(brk) = brk.strip_prefix('^') {
                if let Some(bword) = word.strip_prefix(brk) {
                    if self._spellcheck(bword, strict, count) {
                        return true;
                    }
                }
            } else if let Some(brk) = brk.strip_suffix('$') {
                if let Some(bword) = word.strip_suffix(brk) {
                    if self._spellcheck(bword, strict, count) {
                        return true;
                    }
                }
            }
        }

        // If the word ends on a '.', try removing it.
        if let Some(bword) = word.strip_suffix('.') {
            if self._spellcheck(bword, strict, count) {
                return true;
            }
        }

        // Try breaking words into pieces.
        for brk in self.affix_data.word_breaks.iter() {
            if brk.starts_with('^') || brk.ends_with('$') {
                continue;
            }
            if let Some((worda, wordb)) = word.split_once(brk) {
                if self._spellcheck(worda, strict, count)
                    && self._spellcheck(wordb, strict, count)
                {
                    return true;
                }
            }
        }
        false
    }

    // Check a word against the dictionary and try different capitalization
    fn _spellcheck_caps(
        &self,
        word: &str,
        caps: CapStyle,
        strict: StrictMode,
    ) -> bool {
        if self._spellcheck_compound(word, caps) {
            return true;
        }

        // Any word might be capitalized at the beginning of a sentence,
        // and any phrase might be written in all caps for emphasis,
        // so those should all be detected as correctly spelled.

        if matches!(strict, StrictMode::AllowAll)
            && caps == CapStyle::AllCaps
            && self._spellcheck_compound(
                &default_case_fold_str(word),
                CapStyle::Folded,
            )
        {
            return true;
        }

        if matches!(strict, StrictMode::AllowDecap | StrictMode::AllowAll)
            && caps == CapStyle::Capitalized
            && self._spellcheck_compound(
                &word.to_lowercase(),
                CapStyle::Decapitalized,
            )
        {
            return true;
        }

        false
    }

    fn check_suggestion(&self, word: &str, origcaps: CapStyle) -> bool {
        if self.is_forbidden_suggestion(word) {
            return false;
        }

        let mut count = 0u16;
        if self._spellcheck(word, origcaps.strict(), &mut count) {
            return true;
        }

        // If the suggestion is two words, check both
        if let Some((sugga, suggb)) = word.split_once(' ') {
            self.check_suggestion(sugga, origcaps)
                && self.check_suggestion(suggb, origcaps)
        } else {
            false
        }
    }

    fn check_suggestion_priority(
        &self,
        word: &str,
        origcaps: CapStyle,
    ) -> bool {
        if self.is_forbidden_suggestion(word) {
            return false;
        }

        let caps = CapStyle::from_str(word);
        self._spellcheck_caps(word, caps, origcaps.strict())
    }

    fn _suggestions(&self, word: String, max: usize) -> Vec<String> {
        let mut collector = SuggCollector::new(self, &word, max);

        // Try lowercased, capitalized, or all caps
        // TODO: also match mixed case words, such as "ipod" -> "iPod"
        collector.new_source("different_case");
        collector.suggest(&word.to_lowercase());
        collector.suggest(&word.to_titlecase_lower_rest());
        collector.suggest(&word.to_uppercase());

        self.affix_data.replacements.suggest(&word, &mut collector);

        related_char_suggestions(
            &self.affix_data.related_chars,
            &word,
            &mut collector,
        );

        delete_char_suggestions(&word, &mut collector);

        // TODO: maybe a straight up "delete any two chars" suggestion would
        // be better?
        delete_doubled_pair_suggestions(&word, &mut collector);

        swap_char_suggestions(&word, &mut collector);

        if let Some(try_chars) = &self.affix_data.try_string {
            add_char_suggestions(&word, try_chars, &mut collector);
        }

        move_char_suggestions(&word, &mut collector);

        if let Some(keys) = &self.affix_data.keyboard_string {
            wrong_key_suggestions(&word, keys, &mut collector);
        }

        capitalize_char_suggestions(&word, &mut collector);

        let has_good = collector.has_suggestions();

        // Try splitting the word into two words.
        // These should be suggested even if `has_good` is true, but don't
        // count as good suggestions themselves.
        split_word_suggestions(&word, &mut collector);
        if self.affix_data.dash_word_heuristic {
            split_word_with_dash_suggestions(&word, &mut collector);
        }

        // Only try the ngram and delins algorithms if the straightforward
        // corrections didn't produce any usable suggestions.
        if !has_good {
            // Re-use MAXNGRAMSUGGS to limit delins suggestions too.
            collector.set_limit(self.affix_data.max_ngram_suggestions as usize);
            delins_suggestions(&word, self, &mut collector);

            collector.set_limit(self.affix_data.max_ngram_suggestions as usize);
            ngram_suggestions(&word, self, &mut collector);

            collector.set_limit(max);
        }

        collector.into_iter().collect()
    }
}

impl Speller for SpellerHunspellDict {
    fn spellcheck(&self, word: &str) -> bool {
        let word = self.affix_data.iconv.conv(word.trim());
        if word.is_empty() {
            return true;
        }
        if self.is_forbidden(&word) {
            return false;
        }
        let mut count = 0u16;
        self._spellcheck(&word, StrictMode::AllowAll, &mut count)
    }

    fn suggestions(&self, word: &str, max: usize) -> Vec<String> {
        let word = self.affix_data.iconv.conv(word.trim());
        if word.is_empty() || max == 0 {
            return Vec::new();
        }

        self._suggestions(word, max)
            .into_iter()
            .map(|sugg| self.affix_data.oconv.conv(&sugg))
            .collect()
    }

    fn add_word(&mut self, word: &str) -> bool {
        let word = self.affix_data.iconv.conv(word.trim());
        if word.is_empty() {
            return false;
        }
        self.folded_words
            .entry(default_case_fold_str(&word))
            .or_default()
            .push(WordInfo::default());
        self.words
            .entry(word)
            .or_default()
            .push(WordInfo::default());
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
        assert_eq!(false, SpellerHunspellDict::is_numeric(".50"));
    }

    #[test]
    fn test_split_morph() {
        assert_eq!(
            ("a lot", None),
            SpellerHunspellDict::split_morphological_fields("a lot")
        );
        assert_eq!(
            ("Alyssa/L'D'Q'", Some("po:prn is:fem is:inv")),
            SpellerHunspellDict::split_morphological_fields(
                "Alyssa/L'D'Q' po:prn is:fem is:inv"
            )
        );
    }
}
