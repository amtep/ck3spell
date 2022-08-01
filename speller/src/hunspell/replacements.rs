#[derive(Default)]
struct Rep {
    anchor_begin: bool,
    anchor_end: bool,
    from: String,
    to: String,
}

impl Rep {
    fn matches(&self, word: &str, at_start: bool) -> bool {
        if self.anchor_begin && !at_start {
            false
        } else if self.anchor_end {
            word == self.from
        } else {
            word.starts_with(&self.from)
        }
    }
}

#[derive(Default)]
pub struct Replacements {
    reps: Vec<Rep>,
}

impl Replacements {
    pub fn push(&mut self, from: &str, to: &str) {
        let mut rep = Rep::default();
        let mut from = from;
        if from.starts_with('^') {
            from = &from[1..];
            rep.anchor_begin = true;
        }
        if from.ends_with('$') {
            from = &from[..from.len() - 1];
            rep.anchor_end = true;
        }
        rep.from = from.to_string();
        rep.to = to.to_string();
        self.reps.push(rep);
    }

    // TODO make this logarithmic instead of linear
    fn longest_match(&self, word: &str, at_start: bool) -> Option<&Rep> {
        let mut longest_len = 0;
        let mut longest_rep: Option<&Rep> = None;
        for rep in self.reps.iter() {
            if rep.from.len() > longest_len && rep.matches(word, at_start) {
                longest_len = rep.from.len();
                longest_rep = Some(rep);
            }
        }
        longest_rep
    }

    pub fn conv(&self, word: &str) -> String {
        let mut output = String::new();
        let mut skip_to = 0;
        for (i, c) in word.char_indices() {
            if i < skip_to {
                continue;
            }
            if let Some(rep) = self.longest_match(&word[i..], i == 0) {
                output += &rep.to;
                skip_to = i + rep.from.len();
            } else {
                output.push(c);
            }
        }
        output
    }

    pub fn suggest(&self, word: &str, mut suggest: impl FnMut(String) -> bool) {
        for (i, _) in word.char_indices() {
            // TODO: optimize by putting start-anchored reps in a separate list
            for rep in self.reps.iter() {
                if rep.matches(&word[i..], i == 0) {
                    let mut sugg = word[..i].to_string();
                    sugg += &rep.to;
                    sugg += &word[i + rep.from.len()..];
                    if !suggest(sugg) {
                        break;
                    }
                }
            }
        }
    }
}
