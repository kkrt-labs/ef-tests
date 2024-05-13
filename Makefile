# Heavily inspired by Reth: https://github.com/paradigmxyz/reth/blob/main/Makefile

# Include .env file to get GITHUB_TOKEN
ifneq ("$(wildcard .env)","")
	include .env
endif

# The release tag of https://github.com/ethereum/tests to use for EF tests
EF_TESTS_TAG := v13.3-kkrt
EF_TESTS_URL := https://github.com/kkrt-labs/tests/archive/refs/tags/$(EF_TESTS_TAG).tar.gz
EF_TESTS_DIR := ./crates/ef-testing/ethereum-tests

# Kakarot artifacts V0
KKRT_V0_BUILD_ARTIFACT_URL = $(shell curl -L https://api.github.com/repos/kkrt-labs/kakarot/releases/latest | jq -r '.assets[0].browser_download_url')

# Kakarot SSJ artifacts for precompiles
KKRT_SSJ_BUILD_ARTIFACT_URL = $(shell curl -L https://api.github.com/repos/kkrt-labs/kakarot-ssj/releases/latest | jq -r '.assets[0].browser_download_url')

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

setup-kakarot-v0: clean-kakarot-v0
	@curl -sL -o kakarot-build.zip "$(KKRT_V0_BUILD_ARTIFACT_URL)"
	unzip -o kakarot-build.zip -d build/v0
	mv build/v0/build/* build/v0
	mv build/v0/fixtures/ERC20.json build/common/
	rm -f kakarot-build.zip

setup-kakarot-v1: clean-kakarot-v1
	@curl -sL -o dev-artifacts.zip "$(KKRT_SSJ_BUILD_ARTIFACT_URL)"
	unzip -o dev-artifacts.zip -d build/temp
	mv build/temp/contracts_AccountContract.compiled_contract_class.json build/v1
	mv build/temp/contracts_KakarotCore.compiled_contract_class.json build/v1
	mv build/temp/contracts_UninitializedAccount.compiled_contract_class.json build/v1
	mv build/temp/contracts_Cairo1Helpers.compiled_contract_class.json build/common/cairo1_helpers.json
	rm -fr build/temp
	rm -f dev-artifacts.zip

setup-kakarot: clean-common setup-kakarot-v0 setup-kakarot-v1

clean-common:
	rm -rf build/common
	mkdir -p build/common

clean-kakarot-v0:
	rm -rf build/v0
	mkdir -p build/v0

clean-kakarot-v1:
	rm -rf build/v1
	mkdir -p build/v1

# Runs all tests but integration tests
unit:
	cargo test --lib

vm-tests-v0-ci: build
	cargo test --test VmTests --lib --no-fail-fast --quiet --features "v0,ci"

vm-tests-v1-ci: build
	cargo test --test VMTests --lib --no-fail-fast --quiet --features "v1,ci"

# Runs the repo tests with the `v0` feature
tests-v0-ci: build
	cargo test --test tests --lib --no-fail-fast --quiet --features "v0,ci"

# Runs the repo tests with the `v1` feature
tests-v1-ci: build
	cargo test --test tests --lib --no-fail-fast --quiet --features "v1,ci"

# Runs ef tests only with the `v0` feature
ef-test-v0: build
	cargo test --test tests --no-fail-fast --quiet --features "v0,ci"

# Runs ef tests only with the `v1` feature
ef-test-v1: build
	cargo test --test tests --no-fail-fast --quiet --features "v1,ci"

# Build the rust crates
build:
	cargo build --release

# Generates a `blockchain-tests-skip.yml` at the project root, by consuming a `data.txt` file containing logs of the ran tests
generate-skip-file:
	python ./scripts/generate_skip_file.py
