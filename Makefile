flamegraph: flamegraph.svg

flamegraph.svg: always
	cargo flamegraph -v --bench suggestions --skip-after suggestions::main -- --bench --profile-time 1

.PHONY: always
