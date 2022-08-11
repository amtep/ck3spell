/// Calculate a score for the similarity between `str1` and `str2`.
/// `len1` must be the length of `str1` in chars.
/// `len2` must be the length of `str2` in chars.
/// `nmax` is a bound on how large chunks should be considered for similarity.
pub fn ngram(nmax: usize, vec1: &[char], vec2: &[char]) -> usize {
    let mut score = 0;

    // handle n = 1 as a special case because it is so much simpler
    for c1 in vec1.iter() {
        for c2 in vec2.iter() {
            score += (c1 == c2) as usize
        }
    }
    if nmax == 1 || score <= 1 {
        return score;
    }

    for n in 2..=nmax {
        let mut nscore = 0;
        if n > vec1.len() || n > vec2.len() {
            break;
        }
        for i1 in 0..vec1.len() + 1 - n {
            'next: for i2 in 0..vec2.len() + 1 - n {
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
        let foo = "foo".chars().collect::<Vec<char>>();
        let bar = "bar".chars().collect::<Vec<char>>();
        let awooo = "awooo".chars().collect::<Vec<char>>();
        let awooga = "awooga".chars().collect::<Vec<char>>();

        assert_eq!(0, ngram(1, &foo, &bar));
        assert_eq!(6, ngram(1, &awooo, &foo));
        assert_eq!(6, ngram(1, &awooo, &foo));
        assert_eq!(10, ngram(2, &awooo, &foo));
        assert_eq!(10, ngram(3, &awooo, &foo));
        assert_eq!(9, ngram(1, &awooo, &awooga));
        assert_eq!(17, ngram(2, &awooo, &awooga));
        assert_eq!(23, ngram(3, &awooo, &awooga));
        assert_eq!(27, ngram(4, &awooo, &awooga));
        assert_eq!(27, ngram(5, &awooo, &awooga));
    }
}
