/// Calculate a score for the similarity between `str1` and `str2`.
/// `len1` must be the length of `str1` in chars.
/// `len2` must be the length of `str2` in chars.
/// `nmax` is a bound on how large chunks should be considered for similarity.
pub fn ngram(
    nmax: usize,
    str1: &str,
    len1: usize,
    str2: &str,
    len2: usize,
) -> usize {
    let mut score = 0;

    // handle n = 1 as a special case because it is so much simpler
    for c1 in str1.chars() {
        for c2 in str2.chars() {
            score += (c1 == c2) as usize
        }
    }
    if nmax == 1 || score <= 1 {
        return score;
    }

    for n in 2..=nmax {
        let mut nscore = 0;
        if n > len1 || n > len2 {
            break;
        }
        for (i1, _) in str1.char_indices().take(len1 + 1 - n) {
            for (i2, _) in str2.char_indices().take(len2 + 1 - n) {
                let eq = str1[i1..]
                    .chars()
                    .take(n)
                    .zip(str2[i2..].chars().take(n))
                    .filter(|(c1, c2)| c1 == c2)
                    .count();
                if eq == n {
                    nscore += 1;
                }
            }
        }
        score += nscore * n;
        if nscore <= 1 {
            // If there's only 1 hit of this size, there are no longer hits
            break;
        }
    }

    score
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_ngram_scores() {
        assert_eq!(0, ngram(1, "foo", 3, "bar", 3));
        assert_eq!(6, ngram(1, "awooo", 5, "foo", 3));
        assert_eq!(6, ngram(1, "awooo", 5, "foo", 3));
        assert_eq!(10, ngram(2, "awooo", 5, "foo", 3));
        assert_eq!(10, ngram(3, "awooo", 5, "foo", 3));
        assert_eq!(9, ngram(1, "awooo", 5, "awooga", 6));
        assert_eq!(17, ngram(2, "awooo", 5, "awooga", 6));
        assert_eq!(23, ngram(3, "awooo", 5, "awooga", 6));
        assert_eq!(27, ngram(4, "awooo", 5, "awooga", 6));
        assert_eq!(27, ngram(5, "awooo", 5, "awooga", 6));
    }
}
