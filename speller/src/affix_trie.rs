/// SuffixTrie and PrefixTrie are very similar, but they differ in their
/// internal logic and performance is important, so it was easier to make
/// two separate structs than to make one that can do both.

#[derive(Clone, Debug, Default)]
pub struct SuffixTrie<T> {
    end_here: Vec<T>,
    more: Vec<SuffixTrie<T>>,
}

impl<T: Copy + Default> SuffixTrie<T> {
    pub fn clear(&mut self) {
        self.end_here.clear();
        self.more.clear();
    }

    pub fn insert(&mut self, suffix: &str, t: T) {
        let mut ptr = self;
        let sufb = suffix.as_bytes();
        let mut pos = sufb.len();
        loop {
            if pos == 0 {
                ptr.end_here.push(t);
                break;
            }
            pos -= 1;
            if ptr.more.is_empty() {
                ptr.more.resize_with(u8::MAX as usize, SuffixTrie::default);
            }
            ptr = &mut ptr.more[sufb[pos] as usize];
        }
    }

    pub fn lookup(&self, word: &str, mut found: impl FnMut(T) -> bool) -> bool {
        let mut ptr = self;
        let wordb = word.as_bytes();
        let mut pos = wordb.len();
        loop {
            for t in ptr.end_here.iter() {
                if found(*t) {
                    return true;
                }
            }
            if pos == 0 || ptr.more.is_empty() {
                break;
            }
            pos -= 1;
            ptr = &ptr.more[wordb[pos] as usize];
        }
        false
    }
}

#[derive(Clone, Debug, Default)]
pub struct PrefixTrie<T> {
    end_here: Vec<T>,
    more: Vec<PrefixTrie<T>>,
}

impl<T: Copy + Default> PrefixTrie<T> {
    pub fn clear(&mut self) {
        self.end_here.clear();
        self.more.clear();
    }

    pub fn insert(&mut self, prefix: &str, t: T) {
        let mut ptr = self;
        let preb = prefix.as_bytes();
        let mut pos = 0;
        loop {
            if pos == preb.len() {
                ptr.end_here.push(t);
                break;
            }
            if ptr.more.is_empty() {
                ptr.more.resize_with(u8::MAX as usize, PrefixTrie::default);
            }
            ptr = &mut ptr.more[preb[pos] as usize];
            pos += 1;
        }
    }

    pub fn lookup(&self, word: &str, mut found: impl FnMut(T) -> bool) -> bool {
        let mut ptr = self;
        let wordb = word.as_bytes();
        let mut pos = 0;
        loop {
            for t in ptr.end_here.iter() {
                if found(*t) {
                    return true;
                }
            }
            if pos == wordb.len() || ptr.more.is_empty() {
                break;
            }
            ptr = &ptr.more[wordb[pos] as usize];
            pos += 1;
        }
        false
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_prefix_trie() {
        let mut st: PrefixTrie<i8> = PrefixTrie::default();

        st.insert("foo", 1);
        st.insert("bar", 2);
        st.insert("foobar", 3);
        st.insert("", 0);

        let mut v = Vec::new();
        st.lookup("foo", |i| {
            v.push(i);
            false
        });
        assert_eq!(vec![0, 1], v);

        let mut v = Vec::new();
        st.lookup("foobar", |i| {
            v.push(i);
            false
        });
        assert_eq!(vec![0, 1, 3], v);
    }

    #[test]
    fn test_suffix_trie() {
        let mut st: SuffixTrie<i8> = SuffixTrie::default();

        st.insert("foo", 1);
        st.insert("bar", 2);
        st.insert("foobar", 3);
        st.insert("", 0);

        let mut v = Vec::new();
        st.lookup("foo", |i| {
            v.push(i);
            false
        });
        assert_eq!(vec![0, 1], v);

        let mut v = Vec::new();
        st.lookup("foobar", |i| {
            v.push(i);
            false
        });
        assert_eq!(vec![0, 2, 3], v);

        let mut v = Vec::new();
        st.lookup("foobar", |i| {
            v.push(i);
            true
        });
        assert_eq!(vec![0], v);
    }
}
