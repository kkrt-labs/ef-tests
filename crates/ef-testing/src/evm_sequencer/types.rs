use std::collections::HashMap;
use std::sync::Arc;

use blockifier::execution::contract_class::{ContractClassV0, ContractClassV0Inner};
use cairo_vm::felt::Felt252;
use cairo_vm::serde::deserialize_program::{
    ApTracking, Attribute, FlowTrackingData, HintLocation, HintParams, Identifier, InputFile,
    InstructionLocation, Location, Member, Reference, ReferenceManager,
};
use cairo_vm::serde::deserialize_utils::parse_value;
use cairo_vm::types::relocatable::MaybeRelocatable;
use cairo_vm::{felt::PRIME_STR, serde::deserialize_program::BuiltinName, types::program::Program};
use num_traits::float::FloatCore;
use num_traits::Pow;
use reth_primitives::Address;
use serde::{de, Deserialize, Deserializer};
use serde_json::Number;
use starknet::core::types::contract::legacy::{
    LegacyEntrypointOffset, LegacyParentLocation, RawLegacyEntryPoint, RawLegacyEntryPoints,
};
use starknet::core::types::{
    contract::{
        legacy::{LegacyContractClass, LegacyProgram},
        ComputeClassHashError,
    },
    FieldElement,
};
use starknet_api::core::EntryPointSelector;
use starknet_api::deprecated_contract_class::{EntryPoint, EntryPointOffset, EntryPointType};
use starknet_api::{
    core::{ContractAddress, PatriciaKey},
    hash::StarkFelt,
    StarknetApiError,
};

#[derive(Debug, Clone, Copy)]
pub struct FeltSequencer(FieldElement);

impl From<FieldElement> for FeltSequencer {
    fn from(felt: FieldElement) -> Self {
        Self(felt)
    }
}

impl From<FeltSequencer> for FieldElement {
    fn from(felt: FeltSequencer) -> Self {
        felt.0
    }
}

impl From<Address> for FeltSequencer {
    fn from(address: Address) -> Self {
        let address = FieldElement::from_byte_slice_be(&address.0[..]).unwrap(); // safe unwrap since Address is 20 bytes
        Self(address)
    }
}

impl From<FeltSequencer> for StarkFelt {
    fn from(felt: FeltSequencer) -> Self {
        StarkFelt::from(felt.0)
    }
}

impl TryFrom<FeltSequencer> for ContractAddress {
    type Error = StarknetApiError;

    fn try_from(felt: FeltSequencer) -> Result<Self, Self::Error> {
        let felt: StarkFelt = felt.into();
        let contract_address = ContractAddress(TryInto::<PatriciaKey>::try_into(felt)?);
        Ok(contract_address)
    }
}

pub struct StarknetProgramV0(LegacyProgram);

impl From<LegacyProgram> for StarknetProgramV0 {
    fn from(program: LegacyProgram) -> Self {
        Self(program)
    }
}

impl TryFrom<StarknetProgramV0> for Program {
    type Error = eyre::Error;

    fn try_from(program: StarknetProgramV0) -> Result<Self, Self::Error> {
        let program = program.0;
        if PRIME_STR != program.prime {
            return Err(eyre::eyre!(
                "Program prime {} does not match expected prime {}",
                program.prime,
                PRIME_STR
            ));
        }

        let builtins = program
            .builtins
            .iter()
            .map(|b| serde_json::from_str(&format!("\"{}\"", b)))
            .collect::<Result<Vec<BuiltinName>, serde_json::Error>>()?;

        let data = program
            .data
            .iter()
            .map(|d| MaybeRelocatable::Int(Felt252::from_bytes_be(&d.to_bytes_be())))
            .collect::<Vec<_>>();

        let mut hints: HashMap<usize, Vec<HintParams>> = HashMap::new();
        program.hints.iter().for_each(|(k, v)| {
            hints.insert(
                *k as usize,
                v.iter()
                    .map(|v| HintParams {
                        code: v.code.clone(),
                        accessible_scopes: v.accessible_scopes.clone(),
                        flow_tracking_data: FlowTrackingData {
                            ap_tracking: ApTracking {
                                group: v.flow_tracking_data.ap_tracking.group as usize,
                                offset: v.flow_tracking_data.ap_tracking.offset as usize,
                            },
                            reference_ids: v
                                .flow_tracking_data
                                .reference_ids
                                .iter()
                                .map(|ids| (ids.0.clone(), *ids.1 as usize))
                                .collect(),
                        },
                    })
                    .collect(),
            );
        });

        let reference_manager = ReferenceManager {
            references: program
                .reference_manager
                .references
                .iter()
                .map(|re| {
                    Ok(Reference {
                        ap_tracking_data: ApTracking {
                            group: re.ap_tracking_data.group as usize,
                            offset: re.ap_tracking_data.offset as usize,
                        },
                        pc: Some(re.pc as usize),
                        value_address: parse_value(&re.value)
                            .map_err(|_| {
                                eyre::eyre!("Failed to parse {} to ValueAddress", re.value)
                            })?
                            .1,
                    })
                })
                .collect::<Result<Vec<Reference>, eyre::Error>>()?,
        };

        let identifiers = program
            .identifiers
            .iter()
            .map(|id| {
                Ok((
                    id.0.clone(),
                    Identifier {
                        pc: id.1.pc.map(|pc| pc as usize),
                        type_: Some(id.1.r#type.clone()),
                        value: id
                            .1
                            .value
                            .to_owned()
                            .map(|v| {
                                #[derive(Deserialize)]
                                struct Temp(
                                    #[serde(deserialize_with = "felt_from_number")] pub Felt252,
                                );
                                let t = serde_json::from_str::<Temp>(v.get()).map_err(|err| {
                                    eyre::eyre!(
                                        "Failed to deserialize {} to felt: {}",
                                        v.get(),
                                        err.to_string()
                                    )
                                })?;
                                Result::<Felt252, eyre::Error>::Ok(t.0)
                            })
                            .transpose()?,
                        cairo_type: id.1.cairo_type.clone(),
                        full_name: id.1.full_name.clone(),
                        members: id.1.members.to_owned().map(|x| {
                            x.iter()
                                .map(|(k, v)| {
                                    (
                                        k.clone(),
                                        Member {
                                            cairo_type: v.cairo_type.clone(),
                                            offset: v.offset as usize,
                                        },
                                    )
                                })
                                .collect::<HashMap<String, Member>>()
                        }),
                    },
                ))
            })
            .collect::<Result<HashMap<String, Identifier>, eyre::Error>>()?;

        let error_message_attributes = program
            .attributes
            .map(|x| {
                x.iter()
                    .map(|x| Attribute {
                        name: x.name.clone(),
                        start_pc: x.start_pc as usize,
                        end_pc: x.end_pc as usize,
                        value: x.value.clone(),
                        flow_tracking_data: x.flow_tracking_data.to_owned().map(|x| {
                            FlowTrackingData {
                                ap_tracking: ApTracking {
                                    group: x.ap_tracking.group as usize,
                                    offset: x.ap_tracking.offset as usize,
                                },
                                reference_ids: x
                                    .reference_ids
                                    .iter()
                                    .map(|(k, v)| (k.clone(), *v as usize))
                                    .collect(),
                            }
                        }),
                    })
                    .collect::<Vec<Attribute>>()
            })
            .unwrap_or_default();

        let instruction_locations = program.debug_info.map(|x| {
            x.instruction_locations
                .iter()
                .map(|(k, v)| {
                    (
                        *k as usize,
                        InstructionLocation {
                            inst: Location {
                                end_line: v.inst.end_line as u32,
                                end_col: v.inst.end_col as u32,
                                input_file: InputFile {
                                    filename: v
                                        .inst
                                        .input_file
                                        .filename
                                        .to_owned()
                                        .unwrap_or_default(),
                                },
                                parent_location: convert_legacy_parent_location_to_parent_location(
                                    v.inst.parent_location.to_owned(),
                                ),
                                start_line: v.inst.start_line as u32,
                                start_col: v.inst.start_col as u32,
                            },
                            hints: v
                                .hints
                                .iter()
                                .map(|x| HintLocation {
                                    location: Location {
                                        end_line: x.location.end_line as u32,
                                        end_col: x.location.end_col as u32,
                                        input_file: InputFile {
                                            filename: x
                                                .location
                                                .input_file
                                                .filename
                                                .to_owned()
                                                .unwrap_or_default(),
                                        },
                                        parent_location:
                                            convert_legacy_parent_location_to_parent_location(
                                                x.location.parent_location.to_owned(),
                                            ),
                                        start_line: x.location.start_line as u32,
                                        start_col: x.location.start_col as u32,
                                    },
                                    n_prefix_newlines: x.n_prefix_newlines as u32,
                                })
                                .collect(),
                        },
                    )
                })
                .collect()
        });

        Ok(Program::new(
            builtins,
            data,
            None,
            hints,
            reference_manager,
            identifiers,
            error_message_attributes,
            instruction_locations,
        )?)
    }
}

fn convert_legacy_parent_location_to_parent_location(
    legacy: Option<LegacyParentLocation>,
) -> Option<(Box<Location>, String)> {
    match legacy {
        None => None,
        Some(legacy) => Some((
            Box::new(Location {
                end_line: legacy.location.end_line as u32,
                end_col: legacy.location.end_col as u32,
                input_file: InputFile {
                    filename: legacy.location.input_file.filename.unwrap_or_default(),
                },
                parent_location: convert_legacy_parent_location_to_parent_location(
                    legacy.location.parent_location,
                ),
                start_line: legacy.location.start_line as u32,
                start_col: legacy.location.start_col as u32,
            }),
            legacy.remark,
        )),
    }
}

pub struct StarknetContractClassV0(LegacyContractClass);

impl From<LegacyContractClass> for StarknetContractClassV0 {
    fn from(contract_class: LegacyContractClass) -> Self {
        Self(contract_class)
    }
}

impl StarknetContractClassV0 {
    pub fn class_hash(&self) -> Result<FieldElement, ComputeClassHashError> {
        self.0.class_hash()
    }
}

impl TryFrom<StarknetContractClassV0> for ContractClassV0 {
    type Error = eyre::Error;

    fn try_from(contract_class: StarknetContractClassV0) -> Result<Self, Self::Error> {
        let contract_class = contract_class.0;
        let program = StarknetProgramV0::from(contract_class.program);
        Ok(Self(Arc::new(ContractClassV0Inner {
            program: program.try_into()?,
            entry_points_by_type: convert_legacy_entrypoint_to_entrypoint(
                contract_class.entry_points_by_type,
            ),
        })))
    }
}

fn convert_legacy_entrypoint_to_entrypoint(
    entrypoints: RawLegacyEntryPoints,
) -> HashMap<EntryPointType, Vec<EntryPoint>> {
    fn convert(
        r#type: EntryPointType,
        entrypoints: Vec<RawLegacyEntryPoint>,
    ) -> HashMap<EntryPointType, Vec<EntryPoint>> {
        let entrypoints = entrypoints
            .into_iter()
            .map(|ep| {
                let offset = match ep.offset {
                    LegacyEntrypointOffset::U64AsHex(i) => i,
                    LegacyEntrypointOffset::U64AsInt(i) => i,
                };
                EntryPoint {
                    selector: EntryPointSelector(StarkFelt::from(ep.selector)),
                    offset: EntryPointOffset(offset as usize),
                }
            })
            .collect();
        let mut map = HashMap::new();
        map.insert(r#type, entrypoints);
        map
    }

    let constructor = convert(EntryPointType::Constructor, entrypoints.constructor);
    let external = convert(EntryPointType::External, entrypoints.external);
    let l1_handler = convert(EntryPointType::L1Handler, entrypoints.l1_handler);

    constructor
        .into_iter()
        .chain(external)
        .chain(l1_handler)
        .collect()
}

/// Taken from the cairo-rs crate with minimal changes.
fn felt_from_number<'de, D>(deserializer: D) -> Result<Felt252, D::Error>
where
    D: Deserializer<'de>,
{
    let n = Number::deserialize(deserializer)?;
    match Felt252::parse_bytes(n.to_string().as_bytes(), 10) {
        Some(x) => Ok(x),
        None => {
            // Handle de Number with scientific notation cases
            // e.g.: n = Number(1e27)
            let felt = deserialize_scientific_notation(n);
            if let Some(felt) = felt {
                return Ok(felt);
            }

            Err(de::Error::custom(String::from(
                "felt_from_number parse error",
            )))
        }
    }
}

/// Taken from the cairo-rs crate with minimal changes.
fn deserialize_scientific_notation(n: Number) -> Option<Felt252> {
    match n.as_f64() {
        None => {
            let str = n.to_string();
            let list: [&str; 2] = str.split('e').collect::<Vec<&str>>().try_into().ok()?;

            let exponent = list[1].parse::<u32>().ok()?;
            let base = Felt252::parse_bytes(list[0].to_string().as_bytes(), 10)?;
            Some(base * Felt252::from(10).pow(exponent))
        }
        Some(float) => Felt252::parse_bytes(FloatCore::round(float).to_string().as_bytes(), 10),
    }
}
