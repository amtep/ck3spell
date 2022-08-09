flamegraph: flamegraph-suggest flamegraph-load

flamegraph-suggest: flamegraph-suggest.svg
flamegraph-load: flamegraph-load.svg
flamegraph-ngram: flamegraph-ngram.svg

flamegraph-suggest.svg: always
	cargo flamegraph -v --bench criterion --skip-after criterion::main -- --bench --profile-time 5 suggest
	mv flamegraph.svg flamegraph-suggest.svg

flamegraph-load.svg: always
	cargo flamegraph -v --bench criterion --skip-after criterion::main -- --bench --profile-time 5 load
	mv flamegraph.svg flamegraph-load.svg

flamegraph-ngram.svg: always
	cargo flamegraph -v --bench criterion --skip-after criterion::main --skip-after speller::ngram::ngram -- --bench --profile-time 5 ngram
	mv flamegraph.svg flamegraph-ngram.svg

.PHONY: always
