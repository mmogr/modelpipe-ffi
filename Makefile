.DEFAULT_GOAL := help
.PHONY: help build check clean fmt fmt-check lint test doc doc-check lock-check \
        enforce slices swift xcframework xcframework-fast checksum dev pre-commit ci

# Resolve cargo through rustup's shim explicitly, so a standalone toolchain
# installed by Homebrew cannot silently win over rust-toolchain.toml. Carried
# over from modelpipe, where the same trap was hit.
CARGO ?= $(shell command -v rustup >/dev/null 2>&1 && echo "rustup run --install $$(grep -m1 channel rust-toolchain.toml | cut -d'"' -f2) cargo" || echo cargo)


help: ## Show this help
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Build the library for this host
	$(CARGO) build --lib

check: ## Type-check everything, tests included
	$(CARGO) check --all-targets

clean: ## Remove build output, generated Swift and the framework
	$(CARGO) clean
	rm -rf build generated

fmt: ## Format
	$(CARGO) fmt --all

fmt-check: ## Check formatting
	$(CARGO) fmt --all -- --check

lint: ## Clippy, warnings denied
	$(CARGO) clippy --all-targets --all-features -- -D warnings

test: ## Run the test suite
	$(CARGO) test --no-fail-fast

doc: ## Build and open the docs
	$(CARGO) doc --no-deps --document-private-items --open

doc-check: ## Build the docs with warnings denied
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps --document-private-items

lock-check: ## Fail if Cargo.lock is out of date
	$(CARGO) metadata --locked --format-version 1 >/dev/null

enforce: ## The architecture gates (no toolchain needed)
	@./scripts/check_file_size.sh
	@./scripts/check_no_credentials.sh
	@./scripts/check_workflow_yaml.sh

swift: ## Generate the Swift binding from the built library
	$(CARGO) build --lib
	$(CARGO) run --bin uniffi-bindgen -- generate \
		--library target/debug/libmodelpipe_ffi$(shell uname -s | grep -q Darwin && echo .dylib || echo .so) \
		--language swift --out-dir generated

xcframework: ## Build the XCFramework, optimised (macOS only; needs Xcode)
	@PROFILE=release ./scripts/build-xcframework.sh

xcframework-fast: ## Build the XCFramework unoptimised, for a quick check
	@PROFILE=dev ./scripts/build-xcframework.sh

slices: ## Re-check an already-built XCFramework's slices
	@./scripts/check-slices.sh build/ModelpipeFFI.xcframework

checksum: ## Zip the XCFramework and print its SwiftPM checksum
	@cd build && zip -qry ModelpipeFFI.xcframework.zip ModelpipeFFI.xcframework
	@swift package compute-checksum build/ModelpipeFFI.xcframework.zip

dev: fmt lint test ## Format, lint, test

# What CI runs, minus the Apple half, so a contributor can predict a red build
# before pushing. `xcframework` is deliberately not in here: it needs Xcode,
# and a target that fails on Linux for a reason that is not the contributor's
# fault teaches people to ignore the target.
pre-commit: fmt-check lint check test doc-check lock-check enforce ## Everything CI checks on Linux

ci: pre-commit ## Alias for pre-commit
