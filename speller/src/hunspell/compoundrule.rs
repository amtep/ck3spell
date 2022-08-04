use anyhow::{bail, Result};

use crate::hunspell::affixdata::{AffixData, AffixFlag};

pub struct CompoundRule {
    v: Vec<CompoundElement>,
}

pub enum CompoundElement {
    Multi(AffixFlag),
    Optional(AffixFlag),
    Once(AffixFlag),
}

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
                    rule.v.push(CompoundElement::Once(flag[0]));
                    paren_start = None;
                }
            } else if c == '(' {
                paren_start = Some(i + 1);
            } else if c == '*' {
                let node = match rule.v.last() {
                    None
                    | Some(CompoundElement::Multi(_))
                    | Some(CompoundElement::Optional(_)) => {
                        bail!("COMPOUNDRULE: * must follow flag");
                    }
                    Some(CompoundElement::Once(f)) => {
                        CompoundElement::Multi(*f)
                    }
                };
                *rule.v.last_mut().unwrap() = node;
            } else if c == '?' {
                let node = match rule.v.last() {
                    None
                    | Some(CompoundElement::Multi(_))
                    | Some(CompoundElement::Optional(_)) => {
                        bail!("COMPOUNDRULE: ? must follow flag");
                    }
                    Some(CompoundElement::Once(f)) => {
                        CompoundElement::Optional(*f)
                    }
                };
                *rule.v.last_mut().unwrap() = node;
            } else {
                let flag = ad.parse_flags(&s[i..i + c.len_utf8()])?;
                rule.v.push(CompoundElement::Once(flag[0]));
            }
        }
        Ok(rule)
    }
}
