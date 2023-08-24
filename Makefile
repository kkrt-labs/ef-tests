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

setup: $(EF_TESTS_DIR)

# Runs the Ethereum Foundation tests
ef-tests:
	cargo nextest run -p ef-testing --features ef-tests 

# Runs specific test
ef-test:
	TARGET=$(target) cargo test -p ef-testing --features ef-tests -- --nocapture
