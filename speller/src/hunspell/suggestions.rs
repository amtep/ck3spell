use std::mem::swap;

pub fn related_char_suggestions(
    related: &[String],
    word: &str,
    mut suggest: impl FnMut(String) -> bool,
) {
    // Try all possible combinations of replacements of related characters.
    // This can result in a huge number of candidates for long words.
    // Rely on the `suggest` callback to limit the time spent here.
    // When suggest() returns false, we abort.
    let wvec: Vec<char> = word.chars().collect();
    let mut candidates: Vec<Vec<char>> = vec![wvec.clone()];

    // Process the related classes in order, because the affix file ordered
    // them starting with the most likely.
    for rc in related.iter() {
        for i in 0..wvec.len() {
            if rc.contains(wvec[i]) {
                let mut new_candidates: Vec<Vec<char>> = Vec::new();
                for cnd in candidates.drain(..) {
                    for newc in rc.chars() {
                        if newc == wvec[i] {
                            continue;
                        }
                        let mut newcnd: Vec<char> = cnd.clone();
                        newcnd[i] = newc;
                        let newword = newcnd.iter().collect::<String>();
                        if !suggest(newword) {
                            return;
                        }
                        new_candidates.push(newcnd);
                    }
                    new_candidates.push(cnd);
                }
                swap(&mut candidates, &mut new_candidates);
            }
        }
    }
}

pub fn delete_char_suggestions(
    word: &str,
    mut suggest: impl FnMut(String) -> bool,
) {
    for (i, c) in word.char_indices() {
        let mut sugg = String::with_capacity(word.len());
        sugg.push_str(&word[..i]);
        sugg.push_str(&word[i + c.len_utf8()..]);
        if !suggest(sugg) {
            return;
        }
    }
}
