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
    mut suggest: impl FnMut(&str) -> bool,
) {
    let mut sugg = String::with_capacity(word.len());
    for (i, c) in word.char_indices() {
        sugg.clear();
        sugg.push_str(&word[..i]);
        sugg.push_str(&word[i + c.len_utf8()..]);
        if !suggest(&sugg) {
            return;
        }
    }
}

/// bananana -> banana
pub fn delete_doubled_pair_suggestions(
    word: &str,
    mut suggest: impl FnMut(&str) -> bool,
) {
    let mut sugg = String::with_capacity(word.len());
    let mut prev1 = None;
    let mut prev2 = None;
    let mut prev3 = None;
    for (i, c) in word.char_indices() {
        if prev1.is_none() {
            prev1 = Some((i, c));
            continue;
        } else if prev2.is_none() {
            prev2 = Some((i, c));
            continue;
        } else if prev3.is_none() {
            prev3 = Some((i, c));
            continue;
        }
        if prev1.unwrap().1 == prev3.unwrap().1 && prev2.unwrap().1 == c {
            sugg.clear();
            // delete prev1 and prev2
            sugg.push_str(&word[..prev1.unwrap().0]);
            sugg.push_str(&word[prev3.unwrap().0..]);
            if !suggest(&sugg) {
                return;
            }
        }
        prev1 = prev2;
        prev2 = prev3;
        prev3 = Some((i, c));
    }
}

pub fn swap_char_suggestions(
    word: &str,
    mut suggest: impl FnMut(&str) -> bool,
) {
    let mut sugg = String::with_capacity(word.len());
    // First try swapping adjacent chars (most likely case)
    let mut prev = None;
    for (i, c) in word.char_indices() {
        if let Some((prev_i, prev_c)) = prev {
            sugg.clear();
            sugg.push_str(&word[..prev_i]);
            sugg.push(c);
            sugg.push(prev_c);
            sugg.push_str(&word[i + c.len_utf8()..]);
            if !suggest(&sugg) {
                return;
            }
        }
        prev = Some((i, c));
    }

    // Then try swapping all chars regardless of distance
    for (i1, c1) in word.char_indices() {
        let after_i1 = i1 + c1.len_utf8();
        for (i2, c2) in word[after_i1..].char_indices() {
            // The the char directly after c1 is handled in the loop above
            if i2 == 0 {
                continue;
            }
            let real_i2 = after_i1 + i2;
            let after_i2 = real_i2 + c2.len_utf8();
            sugg.clear();
            sugg.push_str(&word[..i1]);
            sugg.push(c2);
            sugg.push_str(&word[after_i1..real_i2]);
            sugg.push(c1);
            sugg.push_str(&word[after_i2..]);
            if !suggest(&sugg) {
                return;
            }
        }
    }

    // Then try multiple swaps of adjacent chars
    let mut prev = None;
    for (i, c) in word.char_indices() {
        if let Some((prev_i, prev_c)) = prev {
            sugg.clear();
            sugg.push_str(&word[..prev_i]);
            sugg.push(c);
            sugg.push(prev_c);
            let len = sugg.len();
            let mut prev2 = None;
            for (i2, c2) in word[len..].char_indices() {
                if let Some((prev_i2, prev_c2)) = prev2 {
                    sugg.truncate(len);
                    sugg.push_str(&word[len..len + prev_i2]);
                    sugg.push(c2);
                    sugg.push(prev_c2);
                    sugg.push_str(&word[len + i2 + c2.len_utf8()..]);
                    if !suggest(&sugg) {
                        return;
                    }
                }
                prev2 = Some((i2, c2));
            }
        }
        prev = Some((i, c));
    }
}

pub fn move_char_suggestions(
    word: &str,
    mut suggest: impl FnMut(&str) -> bool,
) {
    let mut sugg = String::with_capacity(word.len());
    // Try moving a char to another place in the word.
    // The new location has to be at least 2 chars away, otherwise
    // it's just a swap which we already try in swap_char_suggestions.
    for (i1, c1) in word.char_indices() {
        let after_i1 = i1 + c1.len_utf8();
        for (i2, c2) in word[after_i1..].char_indices() {
            if i2 == 0 {
                continue;
            }
            let real_i2 = after_i1 + i2;
            let after_i2 = real_i2 + c2.len_utf8();
            sugg.clear();
            sugg.push_str(&word[..i1]);
            sugg.push_str(&word[after_i1..after_i2]);
            sugg.push(c1);
            sugg.push_str(&word[after_i2..]);
            if !suggest(&sugg) {
                return;
            }
            sugg.truncate(i1);
            sugg.push(c2);
            sugg.push_str(&word[i1..real_i2]);
            sugg.push_str(&word[after_i2..]);
            if !suggest(&sugg) {
                return;
            }
        }
    }
}

pub fn add_char_suggestions(
    word: &str,
    try_chars: &str,
    mut suggest: impl FnMut(&str) -> bool,
) {
    // Try them in order; the affix file put them in order of likelihood
    for tc in try_chars.chars() {
        if tc == '-' {
            // Dashes are tried separately with special logic
            continue;
        }
        // Try the char in front of each char
        let sugg_len = word.len() + tc.len_utf8();
        let mut sugg = String::with_capacity(sugg_len);
        for (i, _) in word.char_indices() {
            sugg.clear();
            sugg.push_str(&word[..i]);
            sugg.push(tc);
            sugg.push_str(&word[i..]);
            if !suggest(&sugg) {
                return;
            }
        }
        // Also try it at the end
        sugg.clear();
        sugg.push_str(word);
        sugg.push(tc);
        if !suggest(&sugg) {
            return;
        }
    }
}

pub fn split_word_suggestions(
    word: &str,
    mut suggest: impl FnMut(&str) -> bool,
) {
    let mut sugg = String::with_capacity(word.len() + 1);
    // Try adding a space between each pair of letters
    // Try before each letter except the first.
    let mut prev = None;
    for (i, c) in word.char_indices() {
        if let Some(prev_c) = prev {
            if prev_c == '-' || c == '-' {
                continue;
            }
            sugg.clear();
            sugg.push_str(&word[..i]);
            sugg.push(' ');
            sugg.push_str(&word[i..]);
            if !suggest(&sugg) {
                return;
            }
        }
        prev = Some(c);
    }
}

pub fn split_word_with_dash_suggestions(
    word: &str,
    mut suggest: impl FnMut(&str) -> bool,
) {
    let mut sugg = String::with_capacity(word.len() + 1);
    // Try adding a dash between each pair of letters
    // Try before each letter except the first.
    let mut prev = None;
    for (i, c) in word.char_indices() {
        if let Some(prev_c) = prev {
            if prev_c == '.' || prev_c == '-' || c == '-' {
                continue;
            }
            sugg.clear();
            sugg.push_str(&word[..i]);
            sugg.push('-');
            sugg.push_str(&word[i..]);
            if !suggest(&sugg) {
                return;
            }
        }
        prev = Some(c);
    }
}
