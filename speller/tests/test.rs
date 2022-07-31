use std::path::Path;

use speller::{Speller, SpellerHunspellDict};

#[test]
fn match_root_words() {
    let dictpath = Path::new("tests/files/en_US.dic");
    let affpath = Path::new("tests/files/en_US.aff");
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
    let dictpath = Path::new("tests/files/en_US.dic");
    let affpath = Path::new("tests/files/en_US.aff");
    let speller = SpellerHunspellDict::new(&dictpath, &affpath).unwrap();

    assert!(speller.spellcheck("reappear")); // A
    assert!(speller.spellcheck("disappear")); // E
    assert!(speller.spellcheck("reapply")); // A

    assert!(!speller.spellcheck("unappear")); // U (flag not present)
}

#[test]
fn match_suffixes() {
    let dictpath = Path::new("tests/files/en_US.dic");
    let affpath = Path::new("tests/files/en_US.aff");
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

#[test]
fn match_case_words() {
    let dictpath = Path::new("tests/files/en_US.dic");
    let affpath = Path::new("tests/files/en_US.aff");
    let speller = SpellerHunspellDict::new(&dictpath, &affpath).unwrap();

    assert!(speller.spellcheck("ALBERTA"));
    assert!(speller.spellcheck("Angle"));
    assert!(speller.spellcheck("ANOINT"));

    assert!(!speller.spellcheck("alberta")); // As capitalized in the dict
    assert!(!speller.spellcheck("apPear")); // random middle caps are errors
}

#[test]
fn match_cross_words() {
    let dictpath = Path::new("tests/files/en_US.dic");
    let affpath = Path::new("tests/files/en_US.aff");
    let speller = SpellerHunspellDict::new(&dictpath, &affpath).unwrap();

    assert!(speller.spellcheck("reappears")); // A + S
    assert!(speller.spellcheck("reappeared")); // A + D
    assert!(speller.spellcheck("reappearing")); // A + G
    assert!(speller.spellcheck("reapplication")); // A + N
    assert!(speller.spellcheck("reapplications")); // A + X
    assert!(speller.spellcheck("reapplying")); // A + G
    assert!(speller.spellcheck("reapplied")); // A + D
    assert!(speller.spellcheck("reapplies")); // A + S
}

#[test]
fn match_broken_words() {
    let dictpath = Path::new("tests/files/en_US.dic");
    let affpath = Path::new("tests/files/en_US.aff");
    let speller = SpellerHunspellDict::new(&dictpath, &affpath).unwrap();

    assert!(speller.spellcheck("Alberta-angle"));
    assert!(speller.spellcheck("----angle---"));
    // The next one should fail because the speller refuses to recurse
    // as many times as would be needed to resolve it.
    assert!(!speller.spellcheck("-a-a-a-a-a-a-a-a-a-a-"));
}

#[test]
fn language_french() {
    let dictpath = Path::new("tests/files/fr_FR.dic");
    let affpath = Path::new("tests/files/fr_FR.aff");
    let speller = SpellerHunspellDict::new(&dictpath, &affpath).unwrap();

    assert!(speller.spellcheck("visser")); // a0
    assert!(speller.spellcheck("vissant")); // a0
    assert!(speller.spellcheck("visse")); // a0
    assert!(speller.spellcheck("vissé")); // p+
    assert!(speller.spellcheck("vissés")); // p+
}
