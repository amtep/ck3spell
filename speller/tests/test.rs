use std::path::Path;

use speller::{Speller, SpellerHunspellDict};

#[test]
fn match_root_words() {
    let dictpath = Path::new("tests/en_US.dic");
    let affpath = Path::new("tests/en_US.aff");
    let speller = SpellerHunspellDict::new(&dictpath, &affpath).unwrap();

    assert!(speller.spellcheck("Alberta"));
    assert!(speller.spellcheck("angle"));
    assert!(speller.spellcheck("anglicism"));
    assert!(speller.spellcheck("anoint"));

    assert!(!speller.spellcheck("alberta"));
    assert!(!speller.spellcheck("agnle"));
    assert!(!speller.spellcheck("anglisism"));
    assert!(!speller.spellcheck("anont"));
}
