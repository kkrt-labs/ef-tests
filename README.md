# About

This repository contains the execution of the EF standard execution layer tests.

The Ethereum Foundation provides a suite of [official tests](https://github.com/ethereum/tests) to verify the compliance of EVM clients.
Passing all these tests qualifies a client as EVM-compliant.

For further information, please refer to the [official documentation](https://ethereum-tests.readthedocs.io/en/latest/)

As Kakarot is an EVM running within CairoVM, we can't run these tests using the Ethereum Foundation runner ([retesteth](https://github.com/ethereum/retesteth)).
We therefore need to develop our own runner to be able to run these tests on Kakarot and thus certify our compatibility with the EVM.

## Requirements

- nextest: to install [nextest](https://nexte.st/index.html), run `cargo install cargo-nextest --locked`
- A GitHub token in your `.env` file:
  - Copy the `.env.example` file to a `.env` file
  - Create a [GitHub token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens) and add it inside the `.env` file.

## Setup

In order to set up the repo and start the testing, please follow the below
instructions:

- run `make setup`
- run `make fetch-dump`

## Test execution

To run the whole test suite, execute `make ef-tests` To run a specific test or
list of tests, execute `make target=regular_expression ef-test` where
regular_expression allows you to filter on the specific tests you want to run.

## Acknowledgement

This repository is heavily inspired by <https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests>, it uses some code snippets from the Reth codebase and when possible, imports modules and helpers from it.
