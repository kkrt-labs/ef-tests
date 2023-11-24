# Heavily inspired by Reth: https://github.com/paradigmxyz/reth/blob/main/Makefile

# Include .env file to get GITHUB_TOKEN
ifneq ("$(wildcard .env)","")
	include .env
endif

# The release tag of https://github.com/ethereum/tests to use for EF tests
EF_TESTS_TAG := v12.4
EF_TESTS_URL := https://github.com/ethereum/tests/archive/refs/tags/$(EF_TESTS_TAG).tar.gz
EF_TESTS_DIR := ./crates/ef-testing/ethereum-tests

# Kakarot artifacts
KKRT_ARTIFACTS_URL = $(shell curl -sL -H "Authorization: token $(GITHUB_TOKEN)" "https://api.github.com/repos/kkrt-labs/kakarot/actions/workflows/ci.yml/runs?per_page=1&branch=main&event=push&status=success" | jq -r '.workflow_runs[0].artifacts_url')
KKRT_BUILD_ARTIFACT_URL = $(shell curl -sL -H "Authorization: token $(GITHUB_TOKEN)" "$(KKRT_ARTIFACTS_URL)" | jq -r '.artifacts[] | select(.name=="kakarot-build").url')/zip

# Downloads and unpacks Ethereum Foundation tests in the `$(EF_TESTS_DIR)` directory.
# Requires `wget` and `tar`
$(EF_TESTS_DIR):
	mkdir -p $(EF_TESTS_DIR)
	wget $(EF_TESTS_URL) -O ethereum-tests.tar.gz
	tar -xzf ethereum-tests.tar.gz --strip-components=1 -C $(EF_TESTS_DIR)
	rm ethereum-tests.tar.gz

# Ensures the commands for $(EF_TESTS_DIR) always run on `make setup`, regardless if the directory exists
.PHONY: $(EF_TESTS_DIR) build
setup: $(EF_TESTS_DIR)

setup-kakarot: clean-kakarot
	@curl -sL -o kakarot-build.zip -H "Authorization: token $(GITHUB_TOKEN)" "$(KKRT_BUILD_ARTIFACT_URL)"
	unzip -o kakarot-build.zip -d build/v0
	mv build/v0/fixtures/ERC20.json build/common/
	rm -f kakarot-build.zip

clean-kakarot:
	rm -rf build/v0
	mkdir -p build/v0

# Runs all tests but integration tests
unit:
	cargo test --lib

# Runs the repo tests with the `v0` feature
tests-v0: build
	cargo test --test tests --lib --no-fail-fast --quiet --features "v0,ci"

# Runs ef tests only with the `v0` feature
ef-test-v0: build
	cargo test --test tests --no-fail-fast --quiet --features "v0,ci"

# Build the rust crates
build:
	cargo build --release
