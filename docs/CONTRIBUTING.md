# Contributing

Before contributing to this repository, always submit and discuss your proposed
changes through an issue. Should you need further clarification, you can reach
out to us on Telegram, or Discord. Please wait to be assigned to an issue before
opening a pull request.

## Roles

Three roles are available for contributors and will lead to different tasks in
the repository:

- Integrator: adds a test suite, runs the suite, makes an issue for each failing
  test, marks failing tests as skipped.
- Debugger: picks an issue raised by a Integrator, and dives in the EVM spec and
  the current Cairo 0 implementation in order to pin point the exact issue. Ones
  the issue is correctly understood, checks if a similar issue hasn't been
  raised by another Debugger yet. If it has, link the Integrator issue to the
  Debugger issue for tracking and pick up another Integrator issue. If it
  hasn't, raises a detailed issue about the Cairo 0 implementation bug and links
  it to the Integrator issue.
- Implementor: picks up an detailed issue raised by a Debugger and reimplements
  the test logic in Cairo 0 python tests. Once the error is replicated, fixes
  the test in Cairo 0.

## How to

### Integrator

In order to match the current EVM implementation from Reth, we want to run the
same Ethereum/tests as them. A list of all applicable tests can be found
[here](https://github.com/paradigmxyz/reth/blob/main/testing/ef-tests/tests/tests.rs#L17).
To start:

- Check for an unassigned `epic` issue for a test.
- Add it to `crates/ef-testing/tests.rs` by using the `blockchain_tests` macro.
  The first argument should be the name of your test, the second is the folder
  to find the test. Please use the snake case name of the folder for the name of
  the test (e.g. `blockchain_tests!(st_bad_opcode, stBadOpcode)` for adding
  stBadOpcode).
- Comment all the other lines except for the test you added and run
  `make ef-tests`. After a while (count 10-15 minutes) the test will end and you
  should have in your terminal's output a list of all the failing tests.
- Start making issues. When raising issues, please try to match other
  `Integrator` issued on the style, and don't forget to add any raised issue to
  the `epic` issue's task list.

Please find [here](https://github.com/kkrt-labs/ef-tests/issues/52) an example
of an issue raised by an Integrator.

### Debugger

Pick an issue from the available `Integrator` issues. Verify that the test fails
(you can run a specific test by using `make target=your_test_name ef-test`). If
it does, you can start debugging it. The following documentation can be used:

- [Test fillers](https://github.com/ethereum/tests/tree/develop/src/GeneralStateTestsFiller):
  used to generated the actual test. This can be used to understand what code
  the transaction executes.
- [Execution specs](https://github.com/ethereum/execution-specs/tree/master):
  Ethereum execution specifications written in Python. This can be used to
  compare the expected behavior to the
  [Cairo 0 implementation](https://github.com/kkrt-labs/kakarot/tree/main/src).
- [Yellow paper](https://ethereum.github.io/yellowpaper/paper.pdf): Ethereum
  formal specification. This can be used to compare the expected behavior to the
  [Cairo 0 implementation](https://github.com/kkrt-labs/kakarot/tree/main/src).

Please find [here](https://github.com/kkrt-labs/ef-tests/issues/57) an example
of an issue raised by a Debugger.

#### Log opcodes

The following section describes how to log the opcodes when running an
Ethereum/test:

- Clone `https://github.com/kkrt-labs/kakarot-rpc.git`
- Update the `Cargo.toml` file at the root of the repository, by replacing the
  blockifier import by

  ```text
  <!-- trunk-ignore(markdownlint/MD013) -->
  blockifier = { git = "https://github.com/jobez/blockifier.git", rev = "7f00407" }
  ```

- In the file `./kakarot-rpc/lib/kakarot/src/kakarot/instructions.cairo`, add
  the following hint `%{print(ids.opcode, ids.ctx) %}` right after line 65. See
  the following
  [gist](https://gist.github.com/jobez/42941db9361d81778abd36309dfb60dc#file-instructions-cairo-L68-L70)
  for more details.
- From the root of the `kakarot-rpc` repository, run the following:

  ```bash
  cd lib/kakarot && STARKNET_NETWORK=starknet-devnet make build && cd ../..
  make RUST_BACKTRACE=1 dump-katana
  ```

- Now copy the `.katana` folder and paste it inside the ef-tests repository.
- Finally, update the `Cargo.toml` file at the root of the ef-tests repository,
  by replacing the blockifier import by

  ```text
  <!-- trunk-ignore(markdownlint/MD013) -->
  blockifier = { git = "https://github.com/jobez/blockifier.git", rev = "7f00407" }
  ```

- Run `make target=your_test_name ef-test`. You should see the executed opcodes
  being printed out.

#### Reproducing in Python (to be updated once Python test flow is improved)

In case the above doesn't help you with debugging, you can always reproduce the
test case you are fixing in Python. This will give you unlimited hint usage,
allowing for better tracing of the error. In order to set this up:

- Clone `https://github.com/kkrt-labs/kakarot.git`
- Run `make setup && make build`
- Create a new file in `tests/integration/solidity_contracts/EFTests` if there
  is no file corresponding to the scope you want to test. Otherwise you can add
  your test to an existing file.
- The following fixtures can be used in order to set the test environment to the
  exact replica of the Ethereum/test:
  `create_account_with_bytecode_and_storage`, `set_storage_at_evm_address`,
  `deploy_eoa`,... You can have a look at the current tests in the `EfTests`
  folder for help.
- Call `eth_send_transaction` with the exact same arguments as the transaction
  from your test. If the error you get is the same as the error on the ef-tests
  repository, you can now start debugging using hints in the Cairo 0 code.
- In order to run only your test, use `poetry run pytest -k your_test_name`.
- Debug the Cairo 0 code by adding `%{print(ids.x)%}` to print variables (where
  x is the variable you want to print). Be sure to recompile your Cairo 0 code
  EVERY TIME you make changes, by running `make build`.

### Implementor

Coming soon
