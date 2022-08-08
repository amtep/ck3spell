flamegraph: flamegraph-suggest flamegraph-load

flamegraph-suggest: flamegraph-suggest.svg
flamegraph-load: flamegraph-load.svg

flamegraph-suggest.svg: always
	cargo flamegraph -v --bench criterion --skip-after criterion::main -- --bench --profile-time 1 suggest
	mv flamegraph.svg flamegraph-suggest.svg

flamegraph-load.svg: always
	cargo flamegraph -v --bench criterion --skip-after criterion::main -- --bench --profile-time 1 load
	mv flamegraph.svg flamegraph-load.svg

.PHONY: always
