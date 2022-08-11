use std::cmp::min;

/// How many transformations does it take to go from `str1` to `str2`?
/// Valid transformations are:
/// - delete a char
/// - insert any char
/// In the worst case, all of `str1` has to be deleted and all of `str2`
/// has to be inserted, so the score is the sum of their lengths.
/// Best case they are identical already, so 0.
///
/// `maxscore` is an optimization technique. The caller isn't interested
/// in any matches scoring higher than this, so don't bother to calculate
/// them exactly.
pub fn delins(str1: &[char], str2: &[char], maxscore: usize) -> usize {
    delins_inner(str1, str2, maxscore as isize)
}

fn delins_inner(str1: &[char], str2: &[char], maxscore: isize) -> usize {
    if maxscore < 0 {
        return 0;
    }
    let mut i1 = 0;
    let mut i2 = 0;
    while i1 < str1.len() && i2 < str2.len() && str1[i1] == str2[i2] {
        // If the chars are equal then advance for free
        i1 += 1;
        i2 += 1;
    }

    // Once at least one string is exhausted, the leftover score is
    // whatever remains of the other string.
    if i1 >= str1.len() {
        str2.len() - i2
    } else if i2 >= str2.len() {
        str1.len() - i1
    } else {
        // Inserting the needed char to str1 is equivalent to deleting
        // a char from str2, so our choice is which string to 'delete'
        // from by advancing its pointer.
        let del_score =
            1 + delins_inner(&str1[i1 + 1..], &str2[i2..], maxscore - 1);

        let mut ins_score = 1;
        i2 += 1;
        // If we choose not to delete the char, there's no point in
        // changing our minds, we should 'insert' chars until we can
        // use the char we didn't delete.
        while i2 < str2.len() && str2[i2] != str1[i1] {
            ins_score += 1;
            i2 += 1;
        }
        if ins_score < del_score {
            // this check is an optimization
            ins_score += delins_inner(
                &str1[i1..],
                &str2[i2..],
                maxscore - ins_score as isize,
            );
        }

        min(del_score, ins_score)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn delins_helper(str1: &str, str2: &str) -> usize {
        let v1 = str1.chars().collect::<Vec<char>>();
        let v2 = str2.chars().collect::<Vec<char>>();
        delins(&v1, &v2, 100)
    }

    #[test]
    fn test_delins_scores() {
        assert_eq!(1, delins_helper("ba", "a"));
        assert_eq!(1, delins_helper("a", "ba"));
        assert_eq!(2, delins_helper("aa", "ba"));
        assert_eq!(2, delins_helper("foo", "boo"));
        assert_eq!(2, delins_helper("Valhalla", "Walhalla"));
        assert_eq!(7, delins_helper("AÁBCDEÉ", "DÁDÁ"));
    }
}
