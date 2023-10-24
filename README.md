# About

This repository contains the execution of the EF standard execution layer tests.

The Ethereum Foundation provides a suite of
[official tests](https://github.com/ethereum/tests) to verify the compliance of
EVM clients. Passing all these tests qualifies allows a client to gain
confidence on his execution layer. For further information, please refer to the
[official documentation](https://ethereum-tests.readthedocs.io/en/latest/).

Kakarot is an EVM running within CairoVM, coupled with a
[RPC](https://github.com/kkrt-labs/kakarot-rpc/tree/main), which would make it
possible to run these tests using the Ethereum Foundation runner
([retesteth](https://github.com/ethereum/retesteth)). However, in order to limit
the possible number of interactions and avoid adding failing points, we develop
our own simplified test runner based on Reth's ef-tests runner.

## Requirements

- nextest: to install [nextest](https://nexte.st/index.html), run
  `cargo install cargo-nextest --locked`
- A GitHub token in your `.env` file:
  - Copy the `.env.example` file to a `.env` file
  - Create a
    [GitHub token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens)
    and add it inside the `.env` file (make sure you have selected the
    `public_repo` scope in the `repo` category).

## Setup

In order to set up the repo and start the testing, please follow the below
instructions:

- run `make setup`
- run `make setup-kakarot`

## Test execution

To run the whole test suite, execute `make ef-test` To run a specific test or
list of tests, execute `cargo test regular_expression` where regular_expression
allows you to filter on the specific tests you want to run.

## Acknowledgement

This repository is heavily inspired by
<https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests>, it uses some
code snippets from the Reth codebase and when possible, imports modules and
helpers from it.
