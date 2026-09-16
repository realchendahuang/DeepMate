.PHONY: build test run dev dev-release fmt fmt-check clippy check ci cache-clean

build:
	cargo build --workspace

test:
	cargo test --workspace

run:
	cargo run -p deepmate-cli -- $(ARGS)

# Desktop app (Tauri): install frontend deps once, then run the dev loop.
# Requires Node/npm.
dev:
	cd apps/desktop && npm install && npm run tauri dev

dev-release:
	cd apps/desktop && npm install && npm run tauri build

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

check:
	cargo check --workspace

ci: fmt-check clippy test

# Cargo never prunes stale incremental-cache fingerprints on its own; run this
# when target/debug grows into the multiple-GB range.
cache-clean:
	rm -rf target/debug/incremental
