use starknet::core::types::FieldElement;

pub struct ClassHashes {
    pub proxy_class_hash: FieldElement,
    pub eoa_class_hash: FieldElement,
    pub contract_account_class_hash: FieldElement,
}

impl ClassHashes {
    pub fn new(
        proxy_class_hash: FieldElement,
        eoa_class_hash: FieldElement,
        contract_account_class_hash: FieldElement,
    ) -> Self {
        Self {
            proxy_class_hash,
            eoa_class_hash,
            contract_account_class_hash,
        }
    }
}
