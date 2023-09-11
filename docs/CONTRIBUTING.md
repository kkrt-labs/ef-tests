# Contributing

When contributing to this repository, please first discuss the change you wish to make via issue, email, or any other method with the owners of this repository before making a change.

## Roles

Three roles are available for contributors and will lead to different tasks in the repository:

- Integrator: adds a test suite, runs the suite, makes an issue for each failing test, marks failing tests as skipped.
- Debuggor: picks an issue raised by a Integrator, and dives in the EVM spec and the current
Cairo 0 implementation in order to pin point the exact issue. Ones the issue is correctly understood, checks if a similar issue hasn't been raised yet by another Debuggor yet. If it has, link the Integrator issue to the Debuggor issue for tracking and pick up another Integrator issue. If it hasn't, raises a detailed issue about the Cairo 0 implementation bug and links it to the Integrator issue.
- Implementor: picks up an detailed issue raised by a Debuggor and reimplements the test logic in Cairo 0 python tests. Once the error is replicated, fixes the test in Cairo 0.

## How to

### Integrator

In order to match the current EVM implementation from Reth, we want to run the same ethereum/tests as them. A list of all applicable tests can be found [here](https://github.com/paradigmxyz/reth/blob/main/testing/ef-tests/tests/tests.rs#L17).

Check for an unassigned `epic` issue for a test. Add it to  `crates/ef-testing/tests.rs` by using the `blockchain_tests` macro. The first argument should be the name of your test, the second is the folder to find the test. Please use the snake case name of the folder for the name of the test (e.g. `blockchain_tests!(st_bad_opcode, stBadOpcode)` for adding stBadOpcode).

Now comment all the other lines except for the test you added and run `make ef-tests`.
After a while (count 10-15 minutes) the test will end and you should have in your terminal's output a list of all the failing tests. You can now start making issues. When raising issues, please follow the general outline that has been used up to now. Don't forget to add the `Integrator` label to your issue.

### Debuggor

Pick an issue from the available `Integrator` issues. Verify that the test fails (you can run a specific test by using `make target=your_test_name ef-test`). If it does, you can start debbugging it. The following documentation can be used:

- [Test fillers](https://github.com/ethereum/tests/tree/develop/src/GeneralStateTestsFiller) used to generated the actual test.
- [Yellow paper](https://ethereum.github.io/yellowpaper/paper.pdf)
- [Execution specs](https://github.com/ethereum/execution-specs/tree/master)

#### Log opcodes

The following section describes how to log the opcodes when running an ethereum/test:

- Clone `https://github.com/kkrt-labs/kakarot-rpc.git`
- Update the `Cargo.toml` file at the root of the repository, by replacing the blockifier import by `blockifier = { git = "https://github.com/jobez/blockifier.git", rev = "7f00407" }`.
- In the file `./kakarot-rpc/lib/kakarot/src/kakarot/instructions.cairo`, add the following hint `%{print(ids.opcode, ids.ctx) %}` right after line 65. See the following [gist](https://gist.github.com/jobez/42941db9361d81778abd36309dfb60dc#file-instructions-cairo-L68-L70) for more details.
- From the root of the `kakarot-rpc` repository, run the following: `cd lib/kakarot && STARKNET_NETWORK=starknet-devnet make build && cd ../.. && make RUST_BACKTRACE=1 dump-katana`
- Now copy the `.katana` folder and copy it inside the ef-tests repository.
- Finally, update the `Cargo.toml` file at the root of the ef-tests repository, by replacing the blockifier import by `blockifier = { git = "https://github.com/jobez/blockifier.git", rev = "7f00407" }`.
- Run `make target=your_test_name ef-test`. You should see the executed opcodes being printed out.

#### Reproducing in Python (to be updated once Python test flow is improved)

In case the above doesn't help you with debugging, you can always reproduce the test case you are fixing in Python. This will give you unlimited hint usage, allowing for better tracing of the error. In order to set this up:

- Clone `https://github.com/kkrt-labs/kakarot.git`
- Run `make setup && make build`
- Create a new file in `tests/integration/solidity_contracts/EFTests` if there is no file corresponding to the scope you want to test. Otherwise you can add your test to an existing file.
- The following fixtures can be used in order to set the test environment to the exact replica of the ethereum/test: `create_account_with_bytecode_and_storage`, `set_storage_at_evm_address`, `deploy_eoa`,... You can have a look at the current tests in the `EfTests` folder for help.
- Call `eth_send_transaction` with the exact same arguments as the transaction from your test. If the error you get is the same as the error on the ef-tests repository, you can now start debugging using hints in the Cairo 0 code.
- In order to run only your test, you can add `@pytest.mark.MY_TEST` and run it with `make mark=MY_TEST run-test-log`.
- Debug the Cairo 0 code by adding `%{print(ids.x)%}` to print variables (where x is the variable you want to print). Be sure to recompile your Cairo 0 code EVERY TIME you make changes, by running `make build`.

### Implementor

Coming soon
