.PHONY: build test run fmt fmt-check clippy check ci

build:
	cargo build --workspace

test:
	cargo test --workspace

run:
	cargo run -p deepmate-cli -- $(ARGS)

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

check:
	cargo check --workspace

ci: fmt-check clippy test
