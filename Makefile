CARGO ?= cargo
ARGS ?=

.PHONY: check test run

check:
	$(CARGO) clippy --manifest-path rust/Cargo.toml --workspace --all-targets --all-features -- -D warnings

test:
	$(CARGO) nextest run --manifest-path rust/Cargo.toml --workspace

run:
	$(CARGO) run --manifest-path rust/Cargo.toml -p trustify-client --example info -- $(ARGS)
