use criterion::{black_box, criterion_group, criterion_main, Criterion};

use std::path::Path;

use speller::{Speller, SpellerHunspellDict};

fn load_speller(name: &str) -> impl Speller {
    let dictpath = format!("tests/files/{}.dic", name);
    let affpath = format!("tests/files/{}.aff", name);
    SpellerHunspellDict::new(Path::new(&dictpath), Path::new(&affpath)).unwrap()
}

fn criterion_benchmark(c: &mut Criterion) {
    let speller = load_speller("fr_FR");

    c.bench_function("related fr", |b| {
        b.iter(|| speller.suggestions(black_box("Nereide"), 3))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
