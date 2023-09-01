# About

This repository contains the execution of the EF standard execution layer tests.

# Setup

In order to set up the repo and start the testing, please follow the below
instructions:

-   run `make setup`
-   run `make fetch-dump`

# Test execution

To run the whole test suite, execute `make ef-tests` To run a specific test or
list of tests, execute `make target=regular_expression ef-test` where
regular_expression allows you to filter on the specific tests you want to run.
