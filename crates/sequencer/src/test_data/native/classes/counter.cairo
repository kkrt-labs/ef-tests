#[starknet::contract]
mod TestContract {
    #[storage]
    struct Storage {
        counter: felt252, 
    }

    #[external(v0)]
    fn inc(ref self: ContractState) {
        let counter = self.counter.read();
        self.counter.write(counter + 1);
    }
}
