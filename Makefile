# Heavily inspired by Reth: https://github.com/paradigmxyz/reth/blob/main/Makefile

# The release tag of https://github.com/ethereum/tests to use for EF tests
EF_TESTS_TAG := v12.3
EF_TESTS_URL := https://github.com/ethereum/tests/archive/refs/tags/$(EF_TESTS_TAG).tar.gz
EF_TESTS_DIR := ./crates/ef-testing/ethereum-tests

# Downloads and unpacks Ethereum Foundation tests in the `$(EF_TESTS_DIR)` directory.
# Requires `wget` and `tar`
$(EF_TESTS_DIR):
	mkdir -p $(EF_TESTS_DIR)
	wget $(EF_TESTS_URL) -O ethereum-tests.tar.gz
	tar -xzf ethereum-tests.tar.gz --strip-components=1 -C $(EF_TESTS_DIR)
	rm ethereum-tests.tar.gz

# Ensures the commands for $(EF_TESTS_DIR) always run on `make setup`, regardless if the directory exists
.PHONY: $(EF_TESTS_DIR)
setup: $(EF_TESTS_DIR)

setup-kakarot: pull-kakarot
	cd lib/kakarot && make setup && make build

pull-kakarot:
	git submodule update --init --recursive

# Runs the repo tests
tests:
	cargo nextest run --all-features

# Runs ef tests only
ef-test:
	cargo nextest run --package ef-testing --test tests --features ef-tests
