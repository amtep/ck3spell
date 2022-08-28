use std::cmp::min;
use unicode_titlecase::StrTitleCase;

use crate::hunspell::{CapStyle, SpellerHunspellDict};

/// No more than this many suggestion attempts from any one source.
const MAX_SUGGESTS_PER_SOURCE: usize = 1000;

#[derive(Clone, Debug)]
pub struct SuggCollector<'a> {
    dict: &'a SpellerHunspellDict,
    word: &'a str,
    caps: CapStyle,
    max: usize,
    limit: usize,
    suggs: Vec<String>,

    current_source: &'a str,
    counter: usize,
    done: bool,
}

impl<'a> SuggCollector<'a> {
    pub fn new(dict: &'a SpellerHunspellDict, word: &'a str, max: usize) -> Self {
        SuggCollector {
            dict,
            word,
            caps: CapStyle::from_str(word),
            max,
            limit: max,
            suggs: Vec::new(),
            current_source: "unknown",
            counter: 0,
            done: false,
        }
    }

    #[allow(dead_code)] // This function is useful when debugging
    pub fn len(&self) -> usize {
        self.suggs.len()
    }

    pub fn set_limit(&mut self, reserve: usize) {
        self.limit = min(self.suggs.len() + reserve, self.max);
    }

    pub fn new_source(&mut self, name: &'a str) {
        self.current_source = name;
        self.counter = MAX_SUGGESTS_PER_SOURCE;
    }

    pub fn has_suggestions(&self) -> bool {
        !self.suggs.is_empty()
    }

    /// Return true iff no more suggestions should be submitted
    pub fn limit(&self) -> bool {
        self.done || self.suggs.len() >= self.limit || self.counter == 0
    }

    pub fn suggest_priority(&mut self, sugg: &str) {
        // If the suggestion is in the dictionary as a single entry
        // (so no space or break checking), then it overrides all other
        // suggestions.
        if sugg != self.word && self.dict.check_suggestion_priority(sugg, self.caps) {
            self.suggs.clear();
            self.suggs.push(sugg.to_string());
            self.done = true;
        } else {
            self.suggest(sugg);
        }
    }

    pub fn suggest(&mut self, sugg: &str) {
        if self.limit() || sugg == self.word || self.suggs.iter().any(|s| s == sugg) {
            return;
        }
        self.counter -= 1;

        // If the original word was capitalized or uppercase, then prefer
        // capitalized or uppercased suggestions. Do fall back on the
        // unmodified suggestion in case the capitalized forms are rejected.
        if matches!(self.caps, CapStyle::Capitalized) {
            let cap = sugg.to_titlecase();
            if self.dict.check_suggestion(&cap, self.caps) {
                self.suggs.push(cap);
                return;
            }
        } else if matches!(self.caps, CapStyle::AllCaps) {
            let cap = sugg.to_uppercase();
            if self.dict.check_suggestion(&cap, self.caps) {
                self.suggs.push(cap);
                return;
            }
        }

        if self.dict.check_suggestion(sugg, self.caps) {
            self.suggs.push(sugg.to_string());
        }
    }
}

impl<'a> IntoIterator for SuggCollector<'a> {
    type Item = String;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.suggs.into_iter()
    }
}
