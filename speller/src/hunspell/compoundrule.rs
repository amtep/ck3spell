use anyhow::{bail, Result};

use crate::hunspell::affixdata::{AffixData, AffixFlag};

#[derive(Clone, Debug)]
pub struct CompoundRule {
    v: Vec<CompoundElement>,
}

#[derive(Clone, Debug)]
pub enum CompoundElement {
    Multi(AffixFlag),
    Optional(AffixFlag),
    Once(AffixFlag),
}
use CompoundElement::*;

impl CompoundRule {
    pub fn from_str(s: &str, ad: &AffixData) -> Result<Self> {
        let mut rule = CompoundRule { v: Vec::default() };
        let mut paren_start = None;
        for (i, c) in s.char_indices() {
            if let Some(ppos) = paren_start {
                if c == ')' {
                    let flag = ad.parse_flags(&s[ppos..i])?;
                    if flag.len() != 1 {
                        bail!("COMPOUNDRULE: expected 1 flag");
                    }
                    rule.v.push(Once(flag[0]));
                    paren_start = None;
                }
            } else if c == '(' {
                paren_start = Some(i + 1);
            } else if c == '*' {
                let node = match rule.v.last() {
                    None | Some(Multi(_)) | Some(Optional(_)) => {
                        bail!("COMPOUNDRULE: * must follow flag");
                    }
                    Some(Once(f)) => Multi(*f),
                };
                *rule.v.last_mut().unwrap() = node;
            } else if c == '?' {
                let node = match rule.v.last() {
                    None | Some(Multi(_)) | Some(Optional(_)) => {
                        bail!("COMPOUNDRULE: ? must follow flag");
                    }
                    Some(Once(f)) => Optional(*f),
                };
                *rule.v.last_mut().unwrap() = node;
            } else {
                let flag = ad.parse_flags(&s[i..i + c.len_utf8()])?;
                rule.v.push(Once(flag[0]));
            }
        }
        Ok(rule)
    }

    pub fn _matches(
        &self,
        words: &[&str],
        pos: usize,
        check: &impl Fn(&str, AffixFlag) -> bool,
    ) -> bool {
        if let Some(word) = words.get(0) {
            match self.v.get(pos) {
                None => false,
                Some(Once(f)) => {
                    if check(word, *f) {
                        self._matches(&words[1..], pos + 1, check)
                    } else {
                        false
                    }
                }
                Some(Optional(f)) => {
                    if check(word, *f) {
                        self._matches(&words[1..], pos + 1, check)
                            || self._matches(words, pos + 1, check)
                    } else {
                        self._matches(words, pos + 1, check)
                    }
                }
                Some(Multi(f)) => {
                    if check(word, *f) {
                        self._matches(&words[1..], pos, check)
                            || self._matches(words, pos + 1, check)
                    } else {
                        self._matches(words, pos + 1, check)
                    }
                }
            }
        } else {
            match self.v.get(pos) {
                None => true,
                Some(Once(_)) => false,
                Some(Optional(_)) => self._matches(words, pos + 1, check),
                Some(Multi(_)) => self._matches(words, pos + 1, check),
            }
        }
    }

    pub fn matches(
        &self,
        words: &[&str],
        check: impl Fn(&str, AffixFlag) -> bool,
    ) -> bool {
        self._matches(words, 0, &check)
    }
}
