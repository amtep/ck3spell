use criterion::{black_box, criterion_group, criterion_main, Criterion};

use std::path::{Path, PathBuf};

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

fn load_speller(name: &str) -> impl Speller {
    // Relative path of the files depends on whether we are called by
    // cargo bench or cargo flamegraph
    println!(
        "Current directory: {}",
        Path::new(".").canonicalize().unwrap().display()
    );
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

criterion_group!(load, load_fr, load_en, load_de);
criterion_group!(suggest, suggest_fr, suggest_en, suggest_de);
criterion_main!(suggest, load);
