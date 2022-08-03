use criterion::{black_box, criterion_group, criterion_main, Criterion};

use std::path::{Path, PathBuf};

use speller::{Speller, SpellerHunspellDict};

fn load_speller(name: &str) -> impl Speller {
    // Relative path of the files depends on whether we are called by
    // cargo bench or cargo flamegraph
    println!("Current directory: {}", Path::new(".").canonicalize().unwrap().display());
    for dir in ["benches/files", "speller/benches/files"].iter() {
        let dictpath = PathBuf::from(&format!("{}/{}.dic", dir, name));
        let affpath = PathBuf::from(&format!("{}/{}.aff", dir, name));
        if !dictpath.exists() {
            eprintln!("Not found: {}", dictpath.display());
            continue;
        }
        if !affpath.exists() {
            eprintln!("Not found: {}", affpath.display());
            continue;
        }
        match SpellerHunspellDict::new(&dictpath, &affpath) {
            Ok(dict) => { return dict; }
            Err(e) => eprintln!("{:#}", e.to_string()),
        }
    }
    panic!("Could not find dictionary for {}", name);
}

fn suggest_fr(c: &mut Criterion) {
    let speller = load_speller("fr_FR");

    // Tickle both add_char_suggestions and related_char_suggestions
    c.bench_function("suggest_fr_nereide", |b| {
        b.iter(|| speller.suggestions(black_box("Nereide"), 3))
    });
}

fn suggest_en(c: &mut Criterion) {
    let speller = load_speller("en_US");

    c.bench_function("suggest_en_disapearance", |b| {
        b.iter(|| speller.suggestions(black_box("disapearance"), 3))
    });
}

criterion_group!(benches_fr, suggest_fr);
criterion_group!(benches_en, suggest_en);
criterion_main!(benches_fr, benches_en);
