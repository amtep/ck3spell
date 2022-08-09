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
    let vec1: Vec<char> = str1.chars().collect();
    let vec2: Vec<char> = str2.chars().collect();

    let mut score = 0;

    // handle n = 1 as a special case because it is so much simpler
    for c1 in vec1.iter() {
        for c2 in vec2.iter() {
            score += (c1 == c2) as usize
        }
    }
    if nmax == 1 || score <= 1 || len1 <= 1 || len2 <= 1 {
        return score;
    }

    let mut nscore = 0;
    let mut iter1 = vec1.iter().peekable();
    while let Some(c1) = iter1.next() {
        let mut iter2 = vec2.iter().peekable();
        while let Some(c2) = iter2.next() {
            let p1 = iter1.peek();
            let p2 = iter2.peek();
            nscore += (c1 == c2 && p1.is_some() && p1 == p2) as usize
        }
    }

    score += nscore * 2;
    if nmax == 2 || score <= 1 || len1 <= 2 || len2 <= 2 {
        return score;
    }

    for n in 3..=nmax {
        let mut nscore = 0;
        if n > len1 || n > len2 {
            break;
        }
        for i1 in 0..len1 - n {
            'next: for i2 in 0..len2 - n {
                for j in 0..n {
                    if vec1[i1 + j] != vec2[i2 + j] {
                        continue 'next;
                    }
                }
                nscore += 1;
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
