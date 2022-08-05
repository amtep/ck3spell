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

    pub fn lookup(&self, word: &str, found: impl Fn(T) -> bool) -> bool {
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
