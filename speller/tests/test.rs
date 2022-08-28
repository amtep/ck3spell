use std::path::Path;

use speller::{Speller, SpellerHunspellDict};

fn load_speller(name: &str) -> impl Speller {
    let dictpath = format!("tests/files/{}.dic", name);
    let affpath = format!("tests/files/{}.aff", name);
    let speller = SpellerHunspellDict::new(Path::new(&dictpath), Path::new(&affpath)).unwrap();
    for e in speller.get_errors() {
        eprintln!("{:#}", e);
    }
    speller
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
fn match_ordinals() {
    let speller = load_speller("en_US");

    assert!(speller.spellcheck("1st"));
    assert!(speller.spellcheck("2nd"));
    assert!(speller.spellcheck("3rd"));
    assert!(speller.spellcheck("5th"));
    assert!(speller.spellcheck("10th"));
    assert!(speller.spellcheck("11th"));
    assert!(speller.spellcheck("12th"));
    assert!(speller.spellcheck("13th"));
    assert!(speller.spellcheck("20th"));
    assert!(speller.spellcheck("21st"));
    assert!(speller.spellcheck("22nd"));
    assert!(speller.spellcheck("23rd"));
    assert!(speller.spellcheck("1000th"));
    assert!(speller.spellcheck("1001st"));
    assert!(speller.spellcheck("1002nd"));
    assert!(speller.spellcheck("1003rd"));
    assert!(speller.spellcheck("1004th"));

    assert!(!speller.spellcheck("1nd"));
    assert!(!speller.spellcheck("1rd"));
    assert!(!speller.spellcheck("1th"));
    assert!(!speller.spellcheck("2th"));
    assert!(!speller.spellcheck("3th"));
    assert!(!speller.spellcheck("5nd"));
    assert!(!speller.spellcheck("10st"));
    assert!(!speller.spellcheck("11st"));
    assert!(!speller.spellcheck("12nd"));
    assert!(!speller.spellcheck("13rd"));
    assert!(!speller.spellcheck("20st"));
    assert!(!speller.spellcheck("21th"));
    assert!(!speller.spellcheck("22th"));
    assert!(!speller.spellcheck("23th"));
    assert!(!speller.spellcheck("1000st"));
    assert!(!speller.spellcheck("1001th"));
    assert!(!speller.spellcheck("1002rd"));
    assert!(!speller.spellcheck("1003st"));
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

    // empereur has a suffix S. that activates a prefix L'. Make sure
    // that all works.
    assert!(speller.spellcheck("empereur")); // S. /L'D'Q'
    assert!(speller.spellcheck("l'empereur")); // S. L'
    assert!(speller.spellcheck("l'Empereur")); // S. L'
    assert!(speller.spellcheck("L'Empereur")); // S. L'
    assert!(speller.spellcheck("d'empereur")); // S. D'
    assert!(speller.spellcheck("d'Empereur")); // S. D'
    assert!(speller.spellcheck("D'Empereur")); // S. D'
    assert!(speller.spellcheck("qu'empereur")); // S. Q'
    assert!(speller.spellcheck("qu'Empereur")); // S. Q'
    assert!(speller.spellcheck("Qu'Empereur")); // S. Q'

    assert!(speller.spellcheck("empereurs")); // S. /D'Q'
    assert!(speller.spellcheck("d'empereurs")); // S. D'
    assert!(speller.spellcheck("d'Empereurs")); // S. D'
    assert!(speller.spellcheck("D'Empereurs")); // S. D'
    assert!(speller.spellcheck("qu'empereurs")); // S. Q'
    assert!(speller.spellcheck("qu'Empereurs")); // S. Q'
    assert!(speller.spellcheck("Qu'Empereurs")); // S. Q'

    // But not the second form of S. with L.
    assert!(!speller.spellcheck("l'empereurs")); // S. L'
    assert!(!speller.spellcheck("l'Empereurs")); // S. L'
    assert!(!speller.spellcheck("L'Empereurs")); // S. L'
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
fn language_portuguese() {
    let speller = load_speller("pt_BR");

    assert!(speller.spellcheck("aéreo"));
    assert!(speller.spellcheck("aéreos")); // D
    assert!(speller.spellcheck("aérea")); // D
    assert!(speller.spellcheck("aéreas")); // D
    assert!(speller.spellcheck("antiaéreo")); // Á
    assert!(speller.spellcheck("subaéreo")); // È
    assert!(speller.spellcheck("antiaéreos")); // Á + D

    assert!(speller.spellcheck("eugeossinclinal"));
    assert!(speller.spellcheck("eugeossinclinalista")); // 2
    assert!(speller.spellcheck("eugeossinclinalistas")); // 2
    assert!(speller.spellcheck("eugeossinclinais")); // B
    assert!(speller.spellcheck("eugeossinclinalíssima")); // H
    assert!(speller.spellcheck("eugeossinclinalissimamente")); // H
    assert!(speller.spellcheck("eugeossinclinalmente")); // J
    assert!(speller.spellcheck("eugeossinclinaizinhos")); // R
    assert!(speller.spellcheck("eugeossinclinalzão")); // V
    assert!(speller.spellcheck("eugeossinclinalzões")); // V
    assert!(speller.spellcheck("eugeossinclinalidade")); // X
    assert!(speller.spellcheck("eugeossinclinalidades")); // X
    assert!(speller.spellcheck("eugeossinclinalismo")); // Z
    assert!(speller.spellcheck("eugeossinclinalismos")); // Z
    assert!(speller.spellcheck("inremediável"));
    assert!(speller.spellcheck("inremediáveis")); // B
    assert!(speller.spellcheck("inremediabilíssima")); // I
    assert!(speller.spellcheck("inremediabilissimamente")); // I
    assert!(speller.spellcheck("inremediavelmente")); // K
    assert!(speller.spellcheck("inremediabilidade")); // X
    assert!(speller.spellcheck("inremediabilidades")); // X
    assert!(speller.spellcheck("tuberta"));
    assert!(speller.spellcheck("tubertas")); // B
}

#[test]
fn language_polish() {
    let speller = load_speller("pl_PL");

    assert!(speller.spellcheck("fotoelektron"));
    assert!(speller.spellcheck("fotoelektronom")); // N
    assert!(speller.spellcheck("fotoelektronu")); // Q
    assert!(speller.spellcheck("fotoelektronowi")); // Q
    assert!(speller.spellcheck("fotoelektronem")); // Q
    assert!(speller.spellcheck("fotoelektronie")); // Q
    assert!(speller.spellcheck("fotoelektrony")); // s
    assert!(speller.spellcheck("fotoelektronów")); // T
    assert!(speller.spellcheck("niedniujący"));
    assert!(speller.spellcheck("niedniującego")); // X
    assert!(speller.spellcheck("niedniującemu")); // X
    assert!(speller.spellcheck("niedniującym")); // X
    assert!(speller.spellcheck("niedniująca")); // x
    assert!(speller.spellcheck("niedniującej")); // x
    assert!(speller.spellcheck("Starogardzie"));

    assert!(!speller.spellcheck("starogardzie"));
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

    assert!(!speller.spellcheck("ipod"));
    assert!(!speller.spellcheck("iPodos"));

    assert!(sugg(&speller, "ipod", "iPod", 3));
    assert!(sugg(&speller, "iPodos", "ipodos", 3));
    assert!(!sugg(&speller, "ipod", "iPodos", 3));
    assert!(!sugg(&speller, "iPodos", "iPodos", 3));
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

#[test]
fn titlecase_break() {
    let speller = load_speller("en_US");

    assert!(speller.spellcheck("Blood"));
    assert!(speller.spellcheck("Brothers"));
    assert!(speller.spellcheck("Blood-Brothers"));
}

#[test]
fn numeric_break() {
    let speller = load_speller("en_US");

    assert!(speller.spellcheck("15"));
    assert!(speller.spellcheck("foot"));
    assert!(speller.spellcheck("15-foot"));
}

#[test]
fn needaffix_continuation() {
    // test "needaffix5" from hunspell
    let speller = load_speller("needaffix-continuation");

    assert!(speller.spellcheck("foo"));
    assert!(speller.spellcheck("prefoo"));
    assert!(speller.spellcheck("foosuf"));
    assert!(speller.spellcheck("prefoosuf"));
    assert!(speller.spellcheck("foosufbar"));
    assert!(speller.spellcheck("prefoosufbar"));
    assert!(speller.spellcheck("pseudoprefoosuf"));
    assert!(speller.spellcheck("pseudoprefoosufbar"));
    assert!(speller.spellcheck("pseudoprefoopseudosufbar"));
    assert!(speller.spellcheck("prefoopseudosuf"));
    assert!(speller.spellcheck("prefoopseudosufbar"));

    assert!(!speller.spellcheck("pseudoprefoo"));
    assert!(!speller.spellcheck("foopseudosuf"));
    assert!(!speller.spellcheck("pseudoprefoopseudosuf"));
}

fn sugg(speller: &impl Speller, word: &str, sugg: &str, max: usize) -> bool {
    speller.suggestions(word, max).contains(&sugg.to_string())
}

#[test]
fn suggestions() {
    let speller = load_speller("en_US");

    assert!(sugg(&speller, "portmanto", "portmanteau", 3));
}

#[test]
fn suggest_a_lot() {
    let speller = load_speller("en_US");

    // REP with spaces
    assert!(sugg(&speller, "alot", "a lot", 3));
}

#[test]
fn suggest_related_chars() {
    let speller = load_speller("fr_FR");

    assert!(sugg(&speller, "Nereide", "Néréide", 3));
}

#[test]
fn suggest_capsed() {
    let speller = load_speller("en_US");

    assert!(sugg(&speller, "alberta", "Alberta", 3));
}

#[test]
fn suggest_upcased() {
    let speller = load_speller("suggest");

    assert!(sugg(&speller, "nasa", "NASA", 3));
}

#[test]
fn suggest_long_move() {
    let speller = load_speller("suggest");

    assert!(sugg(&speller, "Ghandi", "Gandhi", 3));
    assert!(sugg(&speller, "greatful", "grateful", 3));
}

#[test]
fn suggest_replace_char() {
    let speller = load_speller("suggest-replace");

    // Make sure the suggestion from earlier in the TRY string comes first.
    assert_eq!(
        vec!["permanent", "permenent", "pxrmxnent"],
        speller.suggestions("permxnent", 3)
    );
}

#[test]
fn suggest_delete_char() {
    let speller = load_speller("en_US");

    assert!(sugg(&speller, "appearr", "appear", 3));
    assert!(sugg(&speller, "apppear", "appear", 3));
    assert!(sugg(&speller, "aappear", "appear", 3));
    assert!(sugg(&speller, "disapppear", "disappear", 3));
}

#[test]
fn suggest_delete_double_pair() {
    let speller = load_speller("suggest");

    assert!(sugg(&speller, "bananana", "banana", 3));
    assert!(sugg(&speller, "vacacation", "vacation", 3));
}

#[test]
fn suggest_add_char() {
    let speller = load_speller("en_US");

    assert!(sugg(&speller, "apear", "appear", 3));
    assert!(sugg(&speller, "ppear", "appear", 3));
    assert!(sugg(&speller, "appea", "appear", 3));
    assert!(sugg(&speller, "disappea", "disappear", 3));
    assert!(sugg(&speller, "isappear", "disappear", 3));
}

#[test]
fn suggest_swap_char() {
    let speller = load_speller("en_US");

    assert!(sugg(&speller, "appaer", "appear", 3));
    assert!(sugg(&speller, "papear", "appear", 3));
    assert!(sugg(&speller, "appera", "appear", 3));

    // Swaps at greater distance
    assert!(sugg(&speller, "apreap", "appear", 3));
    assert!(sugg(&speller, "eppaar", "appear", 3));

    // Multiple swaps in a word
    assert!(sugg(&speller, "ehav", "have", 3));
    assert!(sugg(&speller, "hwihc", "which", 3));
}

#[test]
fn suggest_needaffix() {
    let speller = load_speller("suggest-needaffix");

    assert!(speller.spellcheck("atypical"));
    assert!(!speller.spellcheck("typical"));

    assert!(sugg(&speller, "attypical", "atypical", 3));
    assert!(!sugg(&speller, "typicall", "typical", 3));
    assert!(!sugg(&speller, "attypical", "typical", 3));
}

#[test]
fn suggest_split_word() {
    // Based on hunspell sug2 test
    let speller = load_speller("suggest-split-word");

    assert!(!speller.spellcheck("alot"));
    assert!(!speller.spellcheck("inspite"));
    assert!(!speller.spellcheck("scotfree"));

    assert_eq!(vec!["a lot"], speller.suggestions("alot", 9));
    assert_eq!(vec!["in spite"], speller.suggestions("inspite", 9));
    assert_eq!(vec!["scot-free"], speller.suggestions("scotfree", 9));

    assert_eq!(
        vec!["alto. Inspire"],
        speller.suggestions("alto.Inspire", 9)
    );
}

#[test]
fn test_sharps() {
    // From hunspell checksharpsutf test
    let speller = load_speller("checksharps");

    assert!(speller.spellcheck("müßig"));
    assert!(speller.spellcheck("Müßig"));
    assert!(speller.spellcheck("MÜSSIG"));
    assert!(speller.spellcheck("Ausstoß"));
    assert!(speller.spellcheck("Abstoß."));
    assert!(speller.spellcheck("Außenabmessung"));
    assert!(speller.spellcheck("Prozessionsstraße"));
    assert!(speller.spellcheck("Außenmaße"));
    assert!(speller.spellcheck("AUSSTOSS"));
    assert!(speller.spellcheck("ABSTOSS."));
    assert!(speller.spellcheck("AUSSENABMESSUNG"));
    assert!(speller.spellcheck("PROZESSIONSSTRASSE"));
    assert!(speller.spellcheck("AUSSENMASSE"));

    assert!(!speller.spellcheck("MÜßIG"));
}

#[test]
fn test_compounding() {
    let speller = load_speller("de_DE");

    assert!(speller.spellcheck("Abdeckzirkular"));
    assert!(speller.spellcheck("Abdeck-Abdeckzirkular"));

    assert!(!speller.spellcheck("Abdeck")); // needs affix
    assert!(!speller.spellcheck("-Abdeck")); // only in compound
}

#[test]
fn test_iconv() {
    // From hunspell iconv test
    let speller = load_speller("iconv");

    // The ICONV of this speller should convert cedilla forms to comma forms
    assert!(speller.spellcheck("Chișinău")); // ș (S-cedilla)
    assert!(speller.spellcheck("Chişinău")); // ş (S-comma)
    assert!(speller.spellcheck("Ţepes")); // Ţ (T-cedilla)
    assert!(speller.spellcheck("Țepes")); // Ț (T-comma)
    assert!(speller.spellcheck("Ş")); // S-cedilla
    assert!(speller.spellcheck("ţ")); // t-cedilla
}

#[test]
fn test_iconv_longest() {
    // From hunspell iconv2 test
    let speller = load_speller("iconv-longest");

    // Check that ICONV is applied to the longest match in the table
    assert!(speller.spellcheck("GaNa"));
    assert!(speller.spellcheck("Gag"));
    assert!(speller.spellcheck("GaggNa"));
    assert!(speller.spellcheck("NanDa"));
}

#[test]
fn test_oconv() {
    let speller = load_speller("oconv");

    assert!(speller.spellcheck("bébé"));
    assert!(speller.spellcheck("dádá"));

    assert!(!speller.spellcheck("béb"));
    assert!(!speller.spellcheck("dád"));
    assert!(!speller.spellcheck("aábcde"));

    // Check that OCONV is applied to suggestions.
    assert_eq!(vec!["BÉBÉ"], speller.suggestions("béb", 9));
    assert_eq!(vec!["DÁDÁ"], speller.suggestions("dád", 9));
    assert_eq!(vec!["AÁBCDEÉ"], speller.suggestions("aábcde", 9));
}

#[test]
fn test_wordpair() {
    let speller = load_speller("wordpair");

    assert!(speller.spellcheck("foo"));
    assert!(speller.spellcheck("bar"));
    assert!(speller.spellcheck("barfoo"));

    assert!(!speller.spellcheck("foobar"));
}

#[test]
fn test_keepcase() {
    // Based on hunspell "opentaal_keepcase" test
    let speller = load_speller("keepcase");

    assert!(speller.spellcheck("tv-word"));
    assert!(speller.spellcheck("word-tv"));
    assert!(speller.spellcheck("NATO-word"));
    assert!(speller.spellcheck("word-NATO"));

    assert!(!speller.spellcheck("TV-word"));
    assert!(!speller.spellcheck("Tv-word"));
    assert!(!speller.spellcheck("word-TV"));
    assert!(!speller.spellcheck("word-Tv"));
    assert!(!speller.spellcheck("Nato-word"));
    assert!(!speller.spellcheck("word-nato"));
}
