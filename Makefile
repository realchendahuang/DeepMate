.PHONY: build test run dev dev-release fmt fmt-check clippy check ci desktop-check purity bindings-check i18n-check verify cache-clean

# --locked everywhere: the released artifact must be built from the lockfile
# the repository pins, and a stale lockfile should fail the gate rather than
# silently resolve new dependency versions.
build:
	cargo build --workspace --locked

test:
	cargo test --workspace --locked

run:
	cargo run -p deepmate-cli --locked -- $(ARGS)

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
	cargo clippy --workspace --all-targets --locked -- -D warnings

check:
	cargo check --workspace --locked

# Architectural gate: the control core stays a pure data layer.
# Harness-specific names live in crates/deepseek-harness, never in the core.
# Case-insensitive so `DeepSeek`/`DSH` cannot slip past the pattern.
purity:
	@if grep -rEIi 'deepseek|dsh' crates/deepmate-core/src crates/deepmate-core/Cargo.toml; then \
		echo "error: deepmate-core must not reference harness-specific names (deepseek/dsh)"; \
		exit 1; \
	fi

# The generated bindings must match the Rust command surface. Compares the
# file's hash before and after a regeneration instead of diffing against git,
# so the gate works on a dirty working tree too (which is where it matters).
bindings-check:
	@hash() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$$1"; else shasum -a 256 "$$1"; fi | cut -d' ' -f1; }; \
	before=$$(hash apps/desktop/src/shared/api/bindings.ts); \
	cargo test -p deepmate-desktop --lib export_bindings_headless --locked >/dev/null 2>&1; \
	after=$$(hash apps/desktop/src/shared/api/bindings.ts); \
	if [ "$$before" != "$$after" ]; then \
		echo "error: bindings.ts did not match the command surface; it has been regenerated — commit the result"; \
		exit 1; \
	fi; \
	echo "bindings are current"

ci: fmt-check clippy test purity bindings-check

# Frontend gate: eslint + tsc + vitest + prettier formatting check.
desktop-check:
	cd apps/desktop && npm run check

# Localization gate: both catalogs must carry the same key set.
i18n-check:
	cd apps/desktop && npm run check:i18n

# Everything a release must pass: the Rust workspace gate plus the frontend.
verify: ci desktop-check i18n-check

# Cargo never prunes stale incremental-cache fingerprints on its own; run this
# when target/debug grows into the multiple-GB range.
cache-clean:
	rm -rf target/debug/incremental
