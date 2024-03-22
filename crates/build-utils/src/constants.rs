use lazy_static::lazy_static;
use reth_primitives::alloy_primitives::{address, Address};
use std::collections::HashMap;

pub const ROOT: &str = "GeneralStateTests";
pub const FORK: &str = "Cancun";

lazy_static! {
    // A registry of the most common addresses and their associated secret keys.
    // Most secret keys can be read from filler files directly - however, for python-based
    // tests, the secret keys are not present in the filler files. This registry
    // is used to fill in the missing secret keys (only two used in pyspec tests).
    pub static ref ADDRESSES_KEYS: HashMap<Address, &'static str> = {
        let mut registry = HashMap::new();
        registry.insert(
            address!("a94f5374fce5edbc8e2a8697c15331677e6ebf0b"),
            "0x45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8",
        );
        registry.insert(
            address!("8a0a19589531694250d570040a0c4b74576919b8"),
            "0x9e7645d0cfd9c3a04eb7a9db59a4eb7d359f2e75c9164a9d6b9a7d54e1b6a36f",
        );
        registry.insert(
            address!("d02d72e067e77158444ef2020ff2d325f929b363"),
            "41f6e321b31e72173f8ff2e292359e1862f24fba42fe6f97efaf641980eff298",
        );
        registry.insert(
            address!("97a7cb1de3cc7d556d0aa32433b035067709e1fc"),
            "0x0b2986cc45bd8a8d028c3fcf6f7a11a52f1df61f3ea5d63f05ca109dd73a3fa0"
        );
        registry
    };
}
