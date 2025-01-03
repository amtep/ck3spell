use fnv::FnvHashSet;
use itertools::Itertools;
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::mem::swap;

use crate::delins::delins;
use crate::hunspell::suggcollector::SuggCollector;
use crate::hunspell::wordflags::WordFlags;
use crate::ngram::ngram;
use crate::SpellerHunspellDict;

const MAX_NGRAM_ROOTS: usize = 100;
const MAX_NGRAM_SUGG: usize = 20;
const MAX_DELINS_ROOTS: usize = 100;
const MAX_DELINS_SUGG: usize = 20;
/// This is a heuristic. Suggestions scoring worse than this are not offered.
const MAX_DELINS_SCORE: usize = 5;
/// Don't accept too short delins suggestions; they rarely have anything
/// to do with the original word.
const MAX_DELINS_SHORTER: usize = 3;

pub fn related_char_suggestions(related: &[String], word: &str, collector: &mut SuggCollector) {
    collector.new_source("related_char");
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
                        collector.suggest(&newword);
                        if collector.limit() {
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

pub fn delete_char_suggestions(word: &str, collector: &mut SuggCollector) {
    collector.new_source("delete_char");
    let mut sugg = String::with_capacity(word.len());
    for (i, c) in word.char_indices() {
        sugg.clear();
        sugg.push_str(&word[..i]);
        sugg.push_str(&word[i + c.len_utf8()..]);
        collector.suggest(&sugg);
        if collector.limit() {
            return;
        }
    }
}

/// bananana -> banana
pub fn delete_doubled_pair_suggestions(word: &str, collector: &mut SuggCollector) {
    collector.new_source("delete_doubled_pair");
    let mut sugg = String::with_capacity(word.len());
    for ((i1, c1), (_, c2), (i3, c3), (_, c4)) in word.char_indices().tuple_windows() {
        if c1 == c3 && c2 == c4 {
            sugg.clear();
            // delete c1 and c2
            sugg.push_str(&word[..i1]);
            sugg.push_str(&word[i3..]);
            collector.suggest(&sugg);
            if collector.limit() {
                return;
            }
        }
    }
}

pub fn swap_char_suggestions(word: &str, collector: &mut SuggCollector) {
    collector.new_source("swap_char");
    let mut sugg = String::with_capacity(word.len());
    // First try swapping adjacent chars (most likely case)
    for ((i1, c1), (i2, c2)) in word.char_indices().tuple_windows() {
        sugg.clear();
        sugg.push_str(&word[..i1]);
        sugg.push(c2);
        sugg.push(c1);
        sugg.push_str(&word[i2 + c2.len_utf8()..]);
        collector.suggest(&sugg);
        if collector.limit() {
            return;
        }
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
            collector.suggest(&sugg);
            if collector.limit() {
                return;
            }
        }
    }

    // Then try multiple swaps of adjacent chars
    for ((i1, c1), (_, c2)) in word.char_indices().tuple_windows() {
        sugg.clear();
        sugg.push_str(&word[..i1]);
        sugg.push(c2);
        sugg.push(c1);
        let len = sugg.len();
        for ((i3, c3), (i4, c4)) in word[len..].char_indices().tuple_windows() {
            sugg.truncate(len);
            sugg.push_str(&word[len..len + i3]);
            sugg.push(c4);
            sugg.push(c3);
            sugg.push_str(&word[len + i4 + c4.len_utf8()..]);
            collector.suggest(&sugg);
            if collector.limit() {
                return;
            }
        }
    }
}

pub fn move_char_suggestions(word: &str, collector: &mut SuggCollector) {
    collector.new_source("move_char");
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
            collector.suggest(&sugg);
            sugg.truncate(i1);
            sugg.push(c2);
            sugg.push_str(&word[i1..real_i2]);
            sugg.push_str(&word[after_i2..]);
            collector.suggest(&sugg);
            if collector.limit() {
                return;
            }
        }
    }
}

pub fn add_char_suggestions(word: &str, try_chars: &str, collector: &mut SuggCollector) {
    collector.new_source("add_char");
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
            collector.suggest(&sugg);
            if collector.limit() {
                return;
            }
        }
        // Also try it at the end
        sugg.clear();
        sugg.push_str(word);
        sugg.push(tc);
        collector.suggest(&sugg);
        if collector.limit() {
            return;
        }
    }
}

/// Try replacing each char with any of the chars from the TRY string.
/// (This overlaps a lot with wrong_key_suggestions; do we really need both?)
pub fn replace_char_suggestions(word: &str, try_chars: &str, collector: &mut SuggCollector) {
    collector.new_source("replace_char");
    let mut sugg = String::with_capacity(word.len() + 3);

    for tc in try_chars.chars() {
        if tc == '-' {
            // Dashes are tried separately with special logic
            continue;
        }
        for (i, c) in word.char_indices() {
            sugg.clear();
            sugg.push_str(&word[..i]);
            sugg.push(tc);
            sugg.push_str(&word[i + c.len_utf8()..]);
            collector.suggest(&sugg);
            if collector.limit() {
                return;
            }
        }
    }
}

/// `keyboard` contains a |-separated list of horizontally adjacent keys.
/// This information will be used to generate suggestions, on the basis
/// that the user may have hit an adjacent key instead of the right one.
///
/// A character may occur more than once in the keyboard string, thus having
/// more than two neighbors. This can be used to indicate vertical adjacency
/// as well.
pub fn wrong_key_suggestions(word: &str, keyboard: &str, collector: &mut SuggCollector) {
    collector.new_source("wrong_key");
    let mut sugg = String::with_capacity(word.len());

    for (i, c) in word.char_indices() {
        for (kc1, kc2) in keyboard.chars().tuple_windows() {
            // check neighbor to the right
            if kc1 == c && kc2 != '|' {
                sugg.clear();
                sugg.push_str(&word[..i]);
                sugg.push(kc2);
                sugg.push_str(&word[i + c.len_utf8()..]);
                collector.suggest(&sugg);
            }
            // check neighbor to the left
            if kc2 == c && kc1 != '|' {
                sugg.clear();
                sugg.push_str(&word[..i]);
                sugg.push(kc1);
                sugg.push_str(&word[i + c.len_utf8()..]);
                collector.suggest(&sugg);
            }
            if collector.limit() {
                return;
            }
        }
    }
}

pub fn split_word_suggestions(word: &str, collector: &mut SuggCollector) {
    collector.new_source("split_word");
    // Try adding a space between each pair of letters
    // Try before each letter except the first.
    for ((_, c1), (i2, c2)) in word.char_indices().tuple_windows() {
        if c1 == '-' || c2 == '-' {
            continue;
        }
        let sugg = format!("{} {}", &word[..i2], &word[i2..]);
        collector.suggest_priority(&sugg);
        if collector.limit() {
            return;
        }
    }
}

pub fn split_word_with_dash_suggestions(word: &str, collector: &mut SuggCollector) {
    collector.new_source("split_word_with_dash");
    let mut sugg = String::with_capacity(word.len() + 1);
    // Try adding a dash between each pair of letters
    // Try before each letter except the first.
    for ((_, c1), (i2, c2)) in word.char_indices().tuple_windows() {
        if c1 == '.' || c1 == '-' || c2 == '.' || c2 == '-' {
            continue;
        }
        sugg.clear();
        sugg.push_str(&word[..i2]);
        sugg.push('-');
        sugg.push_str(&word[i2..]);
        collector.suggest_priority(&sugg);
        if collector.limit() {
            return;
        }
    }
}

/// Did the user forget to hit shift on one letter?
pub fn capitalize_char_suggestions(word: &str, collector: &mut SuggCollector) {
    collector.new_source("capitalize_char");
    let mut sugg = String::with_capacity(word.len());
    for (i, c) in word.char_indices() {
        if c.is_uppercase() {
            continue;
        }
        sugg.clear();
        sugg.push_str(&word[..i]);
        // Uppercasing a char may produce multiple chars
        for c_up in c.to_uppercase() {
            sugg.push(c_up);
        }
        sugg.push_str(&word[i + c.len_utf8()..]);
        collector.suggest(&sugg);
        if collector.limit() {
            return;
        }
    }
}

pub fn ngram_suggestions(word: &str, dict: &SpellerHunspellDict, collector: &mut SuggCollector) {
    collector.new_source("ngram");
    if collector.limit() {
        return;
    }

    #[derive(Eq, PartialEq)]
    struct HeapItem<T> {
        word: T,
        score: usize,
    }

    impl<T: Eq> PartialOrd for HeapItem<T> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            // Put other first, to make the heap a min-heap.
            Some(other.score.cmp(&self.score))
        }
    }

    impl<T: Eq> Ord for HeapItem<T> {
        fn cmp(&self, other: &Self) -> Ordering {
            // Put other first, to make the heap a min-heap.
            other.score.cmp(&self.score)
        }
    }

    let wvec = word.chars().collect::<Vec<char>>();

    let mut rootscores: Vec<HeapItem<&String>> = dict
        .words
        .par_iter()
        .filter_map(|(root, homonyms)| {
            for winfo in homonyms.iter() {
                if winfo.word_flags.intersects(
                    WordFlags::Forbidden | WordFlags::NoSuggest | WordFlags::OnlyInCompound,
                ) {
                    return None;
                }
            }

            let rvec = root.chars().collect::<Vec<char>>();
            if rvec.len() > wvec.len() + 2 {
                return None;
            }
            let score = ngram(3, &wvec, &rvec);
            Some(HeapItem { word: root, score })
        })
        .collect();
    rootscores.par_sort_unstable();
    rootscores.truncate(MAX_NGRAM_ROOTS);

    // Heuristic minimum score, to discard bad suggestions
    let heuristic = ngram(1, &wvec, &wvec);
    let mut suggheap: BinaryHeap<HeapItem<String>> = BinaryHeap::with_capacity(MAX_NGRAM_SUGG);
    let mut uniq: FnvHashSet<String> = FnvHashSet::default();
    for HeapItem { word: root, .. } in rootscores {
        dict.affix_data
            .generate_words_from_root(root, dict, |sugg| {
                if uniq.contains(sugg) {
                    return;
                }
                uniq.insert(sugg.to_string());
                let svec = sugg.chars().collect::<Vec<char>>();
                let score = ngram(3, &wvec, &svec);
                if score <= heuristic {
                    return;
                }
                if suggheap.len() == MAX_NGRAM_SUGG && score > suggheap.peek().unwrap().score {
                    suggheap.pop();
                }
                if suggheap.len() < MAX_NGRAM_SUGG {
                    suggheap.push(HeapItem {
                        word: sugg.to_string(),
                        score,
                    });
                }
            });
    }
    for HeapItem { word: sugg, .. } in suggheap.into_sorted_vec() {
        collector.suggest(&sugg);
        if collector.limit() {
            return;
        }
    }
}

/// Same method as ngram suggestions, but using the delins scoring algorithm.
pub fn delins_suggestions(word: &str, dict: &SpellerHunspellDict, collector: &mut SuggCollector) {
    collector.new_source("ngram");
    if collector.limit() {
        return;
    }

    // The logic is reversed compared to ngram because delins scores are
    // lower = better.
    #[derive(Eq, PartialEq)]
    struct HeapItem<T> {
        word: T,
        score: usize,
    }

    impl<T: Eq> PartialOrd for HeapItem<T> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.score.cmp(&other.score))
        }
    }

    impl<T: Eq> Ord for HeapItem<T> {
        fn cmp(&self, other: &Self) -> Ordering {
            self.score.cmp(&other.score)
        }
    }

    let wvec = word.chars().collect::<Vec<char>>();

    let mut rootscores: Vec<HeapItem<&String>> = dict
        .words
        .par_iter()
        .filter_map(|(root, homonyms)| {
            for winfo in homonyms.iter() {
                if winfo.word_flags.intersects(
                    WordFlags::Forbidden | WordFlags::NoSuggest | WordFlags::OnlyInCompound,
                ) {
                    return None;
                }
            }

            let rvec = root.chars().collect::<Vec<char>>();
            if rvec.len() > wvec.len() + 2 {
                return None;
            }
            let score = delins(&wvec, &rvec, wvec.len());
            Some(HeapItem { word: root, score })
        })
        .collect();
    rootscores.par_sort_unstable();
    rootscores.truncate(MAX_DELINS_ROOTS);

    let mut suggheap: BinaryHeap<HeapItem<String>> = BinaryHeap::with_capacity(MAX_DELINS_SUGG);
    let mut uniq: FnvHashSet<String> = FnvHashSet::default();
    for HeapItem { word: root, .. } in rootscores {
        dict.affix_data
            .generate_words_from_root(root, dict, |sugg| {
                if uniq.contains(sugg) {
                    return;
                }
                uniq.insert(sugg.to_string());
                let svec = sugg.chars().collect::<Vec<char>>();
                if svec.len() + MAX_DELINS_SHORTER < wvec.len() {
                    return;
                }
                let score = delins(&wvec, &svec, MAX_DELINS_SCORE);
                if score > MAX_DELINS_SCORE {
                    return;
                }
                if suggheap.len() == MAX_DELINS_SUGG && score < suggheap.peek().unwrap().score {
                    suggheap.pop();
                }
                if suggheap.len() < MAX_DELINS_SUGG {
                    suggheap.push(HeapItem {
                        word: sugg.to_string(),
                        score,
                    });
                }
            });
    }
    for HeapItem { word: sugg, .. } in suggheap.into_sorted_vec() {
        collector.suggest(&sugg);
        if collector.limit() {
            return;
        }
    }
}
