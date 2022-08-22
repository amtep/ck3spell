use fnv::FnvHashMap;

#[derive(Debug)]
pub struct CustomEndings {
    table: FnvHashMap<&'static str, Vec<&'static str>>,
}

const CUSTOM_DE: &str = include_str!("../assets/custom_DE.txt");

impl CustomEndings {
    pub fn new(locale: &str) -> Self {
        let mut new = CustomEndings {
            table: FnvHashMap::default(),
        };
        match locale {
            "de_DE" => new.load_strings(CUSTOM_DE),
            _ => (),
        }
        new
    }

    fn load_strings(&mut self, text: &'static str) {
        for line in text.lines() {
            let mut iter = line.trim_end().split(';');
            let key = iter.next().unwrap();
            for value in iter {
                self.table.entry(key).or_default().push(value);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_loaded_de() {
        let custom = CustomEndings::new("de_DE");
        assert_eq!(
            Some(&vec!["des", "der"]),
            custom.table.get("DE_ART_DEF_S_G")
        );
    }
}
