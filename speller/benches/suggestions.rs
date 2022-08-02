use anyhow::Result;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

use std::path::Path;

use speller::{Speller, SpellerHunspellDict};

fn load_speller(dir: &str, name: &str) -> Result<impl Speller> {
    let dictpath = format!("{}/{}.dic", dir, name);
    let affpath = format!("{}/{}.aff", dir, name);
    SpellerHunspellDict::new(Path::new(&dictpath), Path::new(&affpath))
}

fn criterion_benchmark(c: &mut Criterion) {
    // Relative path of the files depends on whether we are called by
    // cargo bench or cargo flamegraph
    let speller = load_speller("tests/files", "fr_FR")
        .or_else(|_| load_speller("speller/tests/files", "fr_FR")).unwrap();

    c.bench_function("related fr", |b| {
        b.iter(|| speller.suggestions(black_box("Nereide"), 3))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
