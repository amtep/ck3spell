use criterion::{black_box, criterion_group, criterion_main, Criterion};

use caseless::default_case_fold_str;
use std::fs::read_to_string;
use std::path::PathBuf;

use speller::ngram::ngram as ngram_fn;
use speller::{Speller, SpellerHunspellDict};

fn find_dict(name: &str) -> (PathBuf, PathBuf) {
    // Relative path of the files depends on whether we are called by
    // cargo bench or cargo flamegraph
    for dir in ["benches/files", "speller/benches/files"].iter() {
        let dictpath = PathBuf::from(&format!("{}/{}.dic", dir, name));
        let affpath = PathBuf::from(&format!("{}/{}.aff", dir, name));
        if !dictpath.exists() {
            // eprintln!("Not found: {}", dictpath.display());
            continue;
        }
        if !affpath.exists() {
            // eprintln!("Not found: {}", affpath.display());
            continue;
        }
        match SpellerHunspellDict::new(&dictpath, &affpath) {
            Ok(_) => {
                return (dictpath, affpath);
            }
            Err(e) => eprintln!("{:#}", e.to_string()),
        }
    }
    panic!("Could not find dictionary for {}", name);
}

fn load_fr(c: &mut Criterion) {
    let (dictpath, affpath) = find_dict("fr_FR");

    c.bench_function("load_fr", |b| {
        b.iter(|| SpellerHunspellDict::new(&dictpath, &affpath))
    });
}

fn load_en(c: &mut Criterion) {
    let (dictpath, affpath) = find_dict("en_US");

    c.bench_function("load_en", |b| {
        b.iter(|| SpellerHunspellDict::new(&dictpath, &affpath))
    });
}

fn load_de(c: &mut Criterion) {
    let (dictpath, affpath) = find_dict("de_DE");

    c.bench_function("load_de", |b| {
        b.iter(|| SpellerHunspellDict::new(&dictpath, &affpath))
    });
}

fn load_pt(c: &mut Criterion) {
    let (dictpath, affpath) = find_dict("pt_BR");

    c.bench_function("load_pt", |b| {
        b.iter(|| SpellerHunspellDict::new(&dictpath, &affpath))
    });
}

fn load_speller(name: &str) -> impl Speller {
    // Relative path of the files depends on whether we are called by
    // cargo bench or cargo flamegraph
    for dir in ["benches/files", "speller/benches/files"].iter() {
        let dictpath = PathBuf::from(&format!("{}/{}.dic", dir, name));
        let affpath = PathBuf::from(&format!("{}/{}.aff", dir, name));
        if !dictpath.exists() {
            // eprintln!("Not found: {}", dictpath.display());
            continue;
        }
        if !affpath.exists() {
            // eprintln!("Not found: {}", affpath.display());
            continue;
        }
        match SpellerHunspellDict::new(&dictpath, &affpath) {
            Ok(dict) => {
                return dict;
            }
            Err(e) => eprintln!("{:#}", e.to_string()),
        }
    }
    panic!("Could not find dictionary for {}", name);
}

fn suggest_fr(c: &mut Criterion) {
    let speller = load_speller("fr_FR");

    dbg!(speller.suggestions("Nereide", 9));

    // Tickle both add_char_suggestions and related_char_suggestions
    c.bench_function("suggest_fr_nereide", |b| {
        b.iter(|| speller.suggestions(black_box("Nereide"), 9))
    });
}

fn suggest_en(c: &mut Criterion) {
    let speller = load_speller("en_US");

    dbg!(speller.suggestions("disapearance", 9));

    c.bench_function("suggest_en_disapearance", |b| {
        b.iter(|| speller.suggestions(black_box("disapearance"), 9))
    });
}

fn suggest_de(c: &mut Criterion) {
    let speller = load_speller("de_DE");

    dbg!(speller.suggestions("Arbeitscompter", 9));

    c.bench_function("suggest_de_compound", |b| {
        b.iter(|| speller.suggestions(black_box("Arbeitscompter"), 9))
    });
}

fn suggest_pt(c: &mut Criterion) {
    let speller = load_speller("pt_BR");

    // Pick a word with enough errors to activate the NGRAM and DELINS
    // suggestion methods.
    dbg!(speller.suggestions("inenediávesi", 9));

    c.bench_function("suggest_pt", |b| {
        b.iter(|| speller.suggestions(black_box("inenediávesi"), 9))
    });
}

fn load_words(name: &str) -> String {
    for dir in ["benches/files", "speller/benches/files"].iter() {
        let dictpath = PathBuf::from(&format!("{}/{}.dic", dir, name));
        if !dictpath.exists() {
            continue;
        }
        match read_to_string(dictpath) {
            Ok(words) => {
                return words;
            }
            Err(e) => eprintln!("{:#}", e.to_string()),
        }
    }
    panic!("Could not find word list for {}", name);
}

fn ngram_loop(c: &mut Criterion) {
    let dict = load_words("de_DE");

    let mut words = Vec::new();
    let mut count = 0;
    for line in dict.lines() {
        count += 1;
        if count != 1000 {
            continue;
        }
        let word = line.split_once('/').map(|(w, _)| w).unwrap_or(line);
        words.push(word.chars().collect::<Vec<char>>());
        count = 0;
    }

    c.bench_function("ngram_loop", |b| {
        b.iter(|| {
            for w1 in &words {
                for w2 in &words {
                    black_box(ngram_fn(3, w1, w2));
                }
            }
        });
    });
}

fn casefold_loop(c: &mut Criterion) {
    c.bench_function("default_case_fold_str", |b| {
        b.iter(|| {
            black_box(default_case_fold_str(black_box("Daniel")));
            black_box(default_case_fold_str(black_box("LICHTENSTEIN")));
            black_box(default_case_fold_str(black_box("marmalade")));
            black_box(default_case_fold_str(black_box("Île-de-France")));
            black_box(default_case_fold_str(black_box("PTOLÉMÉE")));
            black_box(default_case_fold_str(black_box("palissandre")));
            black_box(default_case_fold_str(black_box("Dosen")));
            black_box(default_case_fold_str(black_box("DAGEGEN")));
            black_box(default_case_fold_str(black_box("reihung")));
        });
    });
    c.bench_function("default_case_fold_str_short", |b| {
        b.iter(|| {
            black_box(default_case_fold_str(black_box("I")));
            black_box(default_case_fold_str(black_box("Hz")));
            black_box(default_case_fold_str(black_box("Öl")));
        });
    });
    // Compare with tolower as a reference point
    c.bench_function("tolower", |b| {
        b.iter(|| {
            black_box(black_box("Daniel").to_lowercase());
            black_box(black_box("LICHTENSTEIN").to_lowercase());
            black_box(black_box("marmalade").to_lowercase());
            black_box(black_box("Île-de-France").to_lowercase());
            black_box(black_box("PTOLÉMÉE").to_lowercase());
            black_box(black_box("palissandre").to_lowercase());
            black_box(black_box("Dosen").to_lowercase());
            black_box(black_box("DAGEGEN").to_lowercase());
            black_box(black_box("reihung").to_lowercase());
        });
    });
    c.bench_function("tolower_short", |b| {
        b.iter(|| {
            black_box(black_box("I").to_lowercase());
            black_box(black_box("Hz").to_lowercase());
            black_box(black_box("Öl").to_lowercase());
        });
    });
}

fn load_sample_words(name: &str) -> String {
    for dir in ["benches/files", "speller/benches/files"].iter() {
        let path = PathBuf::from(&format!("{}/{}", dir, name));
        if !path.exists() {
            continue;
        }
        match read_to_string(path) {
            Ok(words) => return words,
            Err(e) => eprintln!("{:#}", e.to_string()),
        }
    }
    panic!("Could not find word list {}", name);
}

fn spellcheck_en(c: &mut Criterion) {
    let speller = load_speller("en_US");

    // A small sample of words from https://www.english-corpora.org/now/
    // with 10% of them deliberately misspelled.
    let words = load_sample_words("words_en.txt");
    let words = words.lines().collect::<Vec<&str>>();

    eprintln!("Benchmarking {} words", words.len());

    c.bench_function("spellcheck_en", |b| {
        b.iter(|| {
            for word in &words {
                black_box(speller.spellcheck(word));
            }
        })
    });
}

fn spellcheck_es(c: &mut Criterion) {
    let speller = load_speller("es_ES");

    // A small sample of words from https://www.corpusdata.org/spanish.asp
    // with 10% of them deliberately misspelled.
    let words = load_sample_words("words_es.txt");
    let words = words.lines().collect::<Vec<&str>>();

    eprintln!("Benchmarking {} words", words.len());

    c.bench_function("spellcheck_es", |b| {
        b.iter(|| {
            for word in &words {
                black_box(speller.spellcheck(word));
            }
        })
    });
}

criterion_group!(spellcheck, spellcheck_en, spellcheck_es);
criterion_group!(casefold, casefold_loop);
criterion_group!(ngram, ngram_loop);
criterion_group!(load, load_fr, load_en, load_de, load_pt);
criterion_group!(suggest, suggest_fr, suggest_en, suggest_de, suggest_pt);
criterion_main!(suggest, load, ngram, casefold, spellcheck);
