use std::path::Path;

use speller::{Speller, SpellerHunspellDict};

fn load_speller(name: &str) -> impl Speller {
    let dictpath = format!("tests/files/{}.dic", name);
    let affpath = format!("tests/files/{}.aff", name);
    SpellerHunspellDict::new(Path::new(&dictpath), Path::new(&affpath)).unwrap()
}

#[test]
fn match_root_words() {
    let speller = load_speller("en_US");

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
    let speller = load_speller("en_US");

    assert!(speller.spellcheck("reappear")); // A
    assert!(speller.spellcheck("disappear")); // E
    assert!(speller.spellcheck("reapply")); // A

    // Capitalized prefixes and all caps prefixes should work too
    assert!(speller.spellcheck("Reappear"));
    assert!(speller.spellcheck("REAPPEAR"));

    assert!(!speller.spellcheck("unappear")); // U (flag not present)
}

#[test]
fn match_suffixes() {
    let speller = load_speller("en_US");

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
    let speller = load_speller("en_US");

    assert!(speller.spellcheck("ALBERTA"));
    assert!(speller.spellcheck("Angle"));
    assert!(speller.spellcheck("ANOINT"));

    assert!(!speller.spellcheck("alberta")); // As capitalized in the dict
    assert!(!speller.spellcheck("apPear")); // random middle caps are errors
}

#[test]
fn match_cross_words() {
    let speller = load_speller("en_US");

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
    let speller = load_speller("en_US");

    assert!(speller.spellcheck("Alberta-angle"));
    assert!(speller.spellcheck("----angle---"));
}

#[test]
fn language_french() {
    let speller = load_speller("fr_FR");

    assert!(speller.spellcheck("visser")); // a0
    assert!(speller.spellcheck("vissant")); // a0
    assert!(speller.spellcheck("visse")); // a0
    assert!(speller.spellcheck("vissé")); // p+
    assert!(speller.spellcheck("vissés")); // p+

    // Néréide appears twice in the dictionary, with different affix flags.
    // Check that all the flags from both words work.
    assert!(speller.spellcheck("Néréide"));
    assert!(speller.spellcheck("L'Néréide")); // L'
    assert!(speller.spellcheck("D'Néréide")); // D'
    assert!(speller.spellcheck("d'Néréide")); // D'
    assert!(speller.spellcheck("qu'Néréide")); // Q'
    assert!(speller.spellcheck("Qu'Néréide")); // Q'
    assert!(speller.spellcheck("Néréides")); // S.

    // But mixing suffix and prefix from different homonyms shouldn't work.
    assert!(!speller.spellcheck("L'Néréides")); // L'
}

#[test]
fn language_german() {
    let speller = load_speller("de_DE");

    assert!(speller.spellcheck("ziemlich"));
    assert!(speller.spellcheck("ziemliche")); // A
    assert!(speller.spellcheck("ziemlicher")); // A
    assert!(speller.spellcheck("unziemlich")); // U
    assert!(speller.spellcheck("unziemliche")); // U + A

    // None of these are allowed because zirkular is OnlyInCompound
    assert!(!speller.spellcheck("zirkular"));
    assert!(!speller.spellcheck("zirkulare")); // E
    assert!(!speller.spellcheck("zirkularen")); // P
    assert!(!speller.spellcheck("zirkulars")); // S
}

#[test]
fn language_spanish() {
    let speller = load_speller("es_ES");

    assert!(speller.spellcheck("gres"));
    assert!(speller.spellcheck("grietás")); // R
    assert!(speller.spellcheck("grieto")); // E
    assert!(speller.spellcheck("grietado")); // D
    assert!(speller.spellcheck("úteros")); // S
}

#[test]
fn language_russian() {
    let speller = load_speller("ru_RU");

    assert!(speller.spellcheck("стащивший"));
    assert!(speller.spellcheck("стащившими")); // A

    assert!(!speller.spellcheck("стаившими")); // A
}

#[test]
fn match_continuation_suffix() {
    let speller = load_speller("2sfx"); // test "flag" from hunspell

    assert!(speller.spellcheck("foo"));
    assert!(speller.spellcheck("foos"));
    assert!(speller.spellcheck("foosbar"));
    assert!(speller.spellcheck("foosbaz"));
    assert!(speller.spellcheck("unfoo"));
    assert!(speller.spellcheck("unfoos"));
    assert!(speller.spellcheck("unfoosbar"));
    assert!(speller.spellcheck("unfoosbaz"));
}

#[test]
fn mixed_case_all_caps() {
    let speller = load_speller("allcaps"); // test "allcaps_utf" from hunspell

    assert!(speller.spellcheck("OpenOffice.org"));
    assert!(speller.spellcheck("OPENOFFICE.ORG"));
    // All caps words should be able to have all caps affixes
    assert!(speller.spellcheck("UNICEF's"));
    assert!(speller.spellcheck("UNICEF'S"));

    // Wrong forms
    assert!(!speller.spellcheck("Openoffice.org"));
    assert!(!speller.spellcheck("Unicef"));
    assert!(!speller.spellcheck("Unicef's"));
}

#[test]
fn mixed_case_all_caps2() {
    let speller = load_speller("allcaps2"); // test from hunspell

    assert!(speller.spellcheck("iPod"));
    assert!(speller.spellcheck("IPOD"));
    assert!(speller.spellcheck("ipodos"));
    assert!(speller.spellcheck("IPODOS"));
}

#[test]
fn forbidden_break() {
    let speller = load_speller("forbidden-break");

    assert!(speller.spellcheck("foo"));
    assert!(speller.spellcheck("bar"));
    assert!(speller.spellcheck("gnu"));
    assert!(speller.spellcheck("foo-gnu"));
    assert!(speller.spellcheck("bar-gnu"));

    assert!(!speller.spellcheck("foo-bar")); // This one is marked forbidden
}

fn sugg(speller: impl Speller, word: &str, sugg: &str, max: usize) -> bool {
    speller.suggestions(word, max).contains(&sugg.to_string())
}

#[test]
fn suggestions() {
    let speller = load_speller("en_US");

    assert!(sugg(speller, "portmanto", "portmanteau", 3));
}

#[test]
fn suggest_a_lot() {
    let speller = load_speller("en_US");

    // REP with spaces
    assert!(sugg(speller, "alot", "a lot", 3));
}

#[test]
fn suggest_related_chars() {
    let speller = load_speller("fr_FR");

    assert!(sugg(speller, "Nereide", "Néréide", 3));
}

#[test]
fn suggest_capsed() {
    let speller = load_speller("en_US");

    assert!(sugg(speller, "alberta", "Alberta", 3));
}
