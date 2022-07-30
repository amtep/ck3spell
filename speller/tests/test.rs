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
    assert!(speller.spellcheck("appear"));
    assert!(speller.spellcheck("apply"));

    assert!(!speller.spellcheck("alberta"));
    assert!(!speller.spellcheck("agnle"));
    assert!(!speller.spellcheck("anglisism"));
    assert!(!speller.spellcheck("apear"));
}

#[test]
fn match_prefixes() {
    let dictpath = Path::new("tests/en_US.dic");
    let affpath = Path::new("tests/en_US.aff");
    let speller = SpellerHunspellDict::new(&dictpath, &affpath).unwrap();

    assert!(speller.spellcheck("reappear")); // A
    assert!(speller.spellcheck("disappear")); // E
    assert!(speller.spellcheck("reapply")); // A

    assert!(!speller.spellcheck("unappear")); // U (flag not present)
}

#[test]
fn match_suffixes() {
    let dictpath = Path::new("tests/en_US.dic");
    let affpath = Path::new("tests/en_US.aff");
    let speller = SpellerHunspellDict::new(&dictpath, &affpath).unwrap();

    assert!(speller.spellcheck("Alberta's")); // M

    assert!(speller.spellcheck("angle's")); // M
    assert!(speller.spellcheck("anglers")); // Z
    assert!(speller.spellcheck("angling")); // G
    assert!(speller.spellcheck("angled")); // D
    assert!(speller.spellcheck("angler")); // R
    assert!(speller.spellcheck("angles")); // S

    assert!(speller.spellcheck("anglicisms")); // S

    assert!(speller.spellcheck("anointing")); // G
    assert!(speller.spellcheck("anointed")); // D
    assert!(speller.spellcheck("anointer")); // R
    assert!(speller.spellcheck("anointment")); // L
    assert!(speller.spellcheck("anoints")); // S

    assert!(speller.spellcheck("appears")); // S
    assert!(speller.spellcheck("appeared")); // D
    assert!(speller.spellcheck("appearing")); // G

    assert!(speller.spellcheck("application")); // N
    assert!(speller.spellcheck("applications")); // X
    assert!(speller.spellcheck("applying")); // G
    assert!(speller.spellcheck("applied")); // D
    assert!(speller.spellcheck("applies")); // S

    assert!(!speller.spellcheck("applyication")); // badly applied N
    assert!(!speller.spellcheck("applyications")); // badly applied X
    assert!(!speller.spellcheck("applyed")); // badly applied D
    assert!(!speller.spellcheck("applyes")); // badly applied S

    assert!(!speller.spellcheck("applion")); // wrong N
    assert!(!speller.spellcheck("appleion")); // badly applied N
    assert!(!speller.spellcheck("applyen")); // wrong N
}
