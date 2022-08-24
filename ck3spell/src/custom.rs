use fnv::FnvHashMap;

#[derive(Debug)]
pub struct CustomEndings {
    table: FnvHashMap<&'static str, Vec<&'static str>>,
}

const CUSTOM_DE: &str = include_str!("../assets/custom_DE.txt");
const CUSTOM_ES: &str = include_str!("../assets/custom_ES.txt");
const CUSTOM_FR: &str = include_str!("../assets/custom_FR.txt");

impl CustomEndings {
    pub fn new(locale: &str) -> Self {
        let mut new = CustomEndings {
            table: FnvHashMap::default(),
        };
        match locale {
            "de_DE" => new.load_strings(CUSTOM_DE),
            "es_ES" => new.load_strings(CUSTOM_ES),
            "fr_FR" => new.load_strings(CUSTOM_FR),
            _ => (),
        }
        new
    }

    pub fn check(&self, custom: &str) -> Option<&Vec<&'static str>> {
        self.table.get(custom)
    }

    fn load_strings(&mut self, text: &'static str) {
        for line in text.lines() {
            let mut iter = line.split(';');
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

    #[test]
    fn test_loaded_es() {
        let custom = CustomEndings::new("es_ES");
        assert_eq!(Some(&vec!["a", ""]), custom.table.get("ES_XA"));
    }

    #[test]
    fn test_loaded_fr() {
        let custom = CustomEndings::new("fr_FR");
        assert_eq!(Some(&vec!["ette", "et"]), custom.table.get("FR_EtEtte"));
    }
}
