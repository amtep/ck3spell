/// Affix conditions are rudimantery regexps (supporting [] groups and
/// [^] negated groups and '.' as wildcard). They are matched against
/// the start or end of words to determine eligibility for suffix and
/// prefix rules.

#[derive(Clone, Debug)]
enum AffixCondChar {
    Any,
    Match(char),
    Group(String),
    NegatedGroup(String),
}

impl AffixCondChar {
    fn matches(&self, wc: char) -> bool {
        match self {
            AffixCondChar::Match(c) => *c == wc,
            AffixCondChar::Group(s) => s.contains(wc),
            AffixCondChar::NegatedGroup(s) => !s.contains(wc),
            AffixCondChar::Any => true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AffixCondition {
    /// A processed version of the condition string, suitable for fast matching.
    cond: Vec<AffixCondChar>,
}

impl AffixCondition {
    pub fn new(condition: &str) -> Self {
        #[derive(PartialEq)]
        enum CondState {
            Matching,
            GroupStart,
            InGroup,
            InNegatedGroup,
        }
        let mut state = CondState::Matching;
        let mut v = Vec::new();
        let mut group_start = 0;
        for (i, c) in condition.char_indices() {
            match state {
                CondState::Matching => {
                    if c == '[' {
                        state = CondState::GroupStart;
                    } else if c == '.' {
                        v.push(AffixCondChar::Any);
                    } else {
                        v.push(AffixCondChar::Match(c));
                    }
                }
                CondState::GroupStart => {
                    if c == '^' {
                        state = CondState::InNegatedGroup;
                        group_start = i + 1;
                    } else {
                        state = CondState::InGroup;
                        group_start = i;
                    }
                }
                CondState::InGroup => {
                    if c == ']' {
                        v.push(AffixCondChar::Group(condition[group_start..i].to_string()));
                        state = CondState::Matching;
                    }
                }
                CondState::InNegatedGroup => {
                    if c == ']' {
                        v.push(AffixCondChar::NegatedGroup(
                            condition[group_start..i].to_string(),
                        ));
                        state = CondState::Matching;
                    }
                }
            }
        }
        if state != CondState::Matching {
            // Bad syntax in condition. Disable it.
            // TODO: warn?
            v.push(AffixCondChar::Group(String::new()));
        }
        AffixCondition { cond: v }
    }

    pub fn prune_prefix(&mut self, prefix: &str) {
        for c in prefix.chars() {
            if self.cond.is_empty() {
                return;
            }
            if !self.cond[0].matches(c) {
                self.cond = vec![AffixCondChar::Group(String::new())];
                return;
            }
            self.cond.remove(0);
        }
    }

    pub fn prune_suffix(&mut self, suffix: &str) {
        for c in suffix.chars().rev() {
            if self.cond.is_empty() {
                return;
            }
            if !self.cond[self.cond.len() - 1].matches(c) {
                self.cond = vec![AffixCondChar::Group(String::new())];
                return;
            }
            self.cond.pop();
        }
    }

    pub fn prefix_match(&self, word: &str) -> bool {
        if self.cond.is_empty() {
            return true;
        }

        let mut pos = 0;
        for c in word.chars() {
            if !self.cond[pos].matches(c) {
                return false;
            }
            pos += 1;
            if pos >= self.cond.len() {
                return true;
            }
        }
        false
    }

    pub fn suffix_match(&self, word: &str) -> bool {
        if self.cond.is_empty() {
            return true;
        }

        let mut pos = self.cond.len() - 1;
        for c in word.chars().rev() {
            if !self.cond[pos].matches(c) {
                return false;
            }
            if pos == 0 {
                return true;
            }
            pos -= 1;
        }
        false
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn help_prefix_condition(cond: &str, word: &str) -> bool {
        let condition = AffixCondition::new(cond);
        condition.prefix_match(word)
    }

    fn help_suffix_condition(cond: &str, word: &str) -> bool {
        let condition = AffixCondition::new(cond);
        condition.suffix_match(word)
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
