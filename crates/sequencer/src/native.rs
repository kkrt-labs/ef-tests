use blockifier::execution::contract_class::NativeContractClassV1;
use blockifier::{
    execution::contract_class::{ContractClass, ContractClassV0, ContractClassV1},
    state::state_api::StateResult,
};
use blockifier::{
    execution::contract_class::{
        ClassInfo as BlockifierClassInfo},
};
use cairo_lang_starknet_classes::abi::Contract;
use cairo_native::{
    context::NativeContext, error::Error as NativeError, executor::AotNativeExecutor,
    metadata::gas::GasMetadata, module::NativeModule,
};
use cairo_lang_starknet_classes::contract_class::ContractClass as SierraContractClass;
use libloading::Library;
use starknet_api::core::ClassHash;
use serde::{Deserialize, Serialize};

use std::{
    ffi::{c_char, c_uchar, c_void, CStr},
    fs,
    path::PathBuf,
    slice,
    sync::Mutex,
};
use cached::{Cached, SizedCache};
use once_cell::sync::Lazy;
use lazy_static::lazy_static;

use cairo_lang_sierra::{program::Program, program_registry::ProgramRegistry};
use hashbrown::HashMap;

lazy_static! {
    static ref NATIVE_CACHE_DIR: PathBuf = setup_native_cache_dir();
}

fn generate_library_path(class_hash: ClassHash) -> PathBuf {
    let mut path = NATIVE_CACHE_DIR.clone();
    path.push(class_hash.to_string().trim_start_matches("0x"));
    path
}

/// Compiles and load contract
///
/// Modelled after [AotNativeExecutor::from_native_module].
/// Needs a sierra_program to workaround limitations of NativeModule
fn persist_from_native_module(
    mut native_module: NativeModule,
    sierra_program: &Program,
    library_output_path: &PathBuf,
) -> Result<AotNativeExecutor, Box<dyn std::error::Error>> {
    let object_data = cairo_native::module_to_object(native_module.module(), Default::default())
        .map_err(|err| NativeError::LLVMCompileError(err.to_string()))?; // cairo native didn't include a from instance

    cairo_native::object_to_shared_lib(&object_data, library_output_path)?;

    let gas_metadata = native_module
        .remove_metadata()
        .expect("native_module should have set gas_metadata");

    // Recreate the program registry as it can't be moved out of native module.
    let program_registry = ProgramRegistry::new(sierra_program)?;

    let library = unsafe { Library::new(library_output_path)? };

    Ok(AotNativeExecutor::new(
        library,
        program_registry,
        gas_metadata,
    ))
}

fn setup_native_cache_dir() -> PathBuf {
    let mut path: PathBuf = match std::env::var("NATIVE_CACHE_DIR") {
        Ok(path) => path.into(),
        Err(_err) => {
            let mut path = std::env::current_dir().unwrap();
            path.push("native_cache");
            path
        }
    };
    let _ = fs::create_dir_all(&path);
    path
}


/// Load a contract that is already compiled.
///
/// Returns None if the contract does not exist at the output_path.
///
/// To compile and load a contract use [persist_from_native_module] instead.
fn load_compiled_contract(
    sierra_program: &Program,
    library_output_path: &PathBuf,
) -> Option<Result<AotNativeExecutor, Box<dyn std::error::Error>>> {
    fn load(
        sierra_program: &Program,
        library_output_path: &PathBuf,
    ) -> Result<AotNativeExecutor, Box<dyn std::error::Error>> {
        let has_gas_builtin = sierra_program
            .type_declarations
            .iter()
            .any(|decl| decl.long_id.generic_id.0.as_str() == "GasBuiltin");
        let config = has_gas_builtin.then_some(Default::default());
        let gas_metadata = GasMetadata::new(sierra_program, config)?;
        let program_registry = ProgramRegistry::new(sierra_program)?;
        let library = unsafe { Library::new(library_output_path)? };
        Ok(AotNativeExecutor::new(
            library,
            program_registry,
            gas_metadata,
        ))
    }

    library_output_path
        .is_file()
        .then_some(load(sierra_program, library_output_path))
}

/// Compiled Native contracts

/// Load a compiled native contract into memory
///
/// Tries to load the compiled contract class from library_output_path if it
/// exists, otherwise it will compile the raw_contract_class, load it into memory
/// and save the compilation artifact to library_output_path.
fn native_try_from_json_string(
    raw_contract_class: &str,
    library_output_path: &PathBuf,
) -> Result<NativeContractClassV1, Box<dyn std::error::Error>> {
    fn compile_and_load(
        sierra_program: Program,
        library_output_path: &PathBuf,
    ) -> Result<AotNativeExecutor, Box<dyn std::error::Error>> {
        println!("Compiling native contract");
        let native_context = NativeContext::new();
        // Ignore the debug names, that might cause conflicts when retrieving entrypoints upon execution of blockifier.
        let native_module = native_context.compile(&sierra_program, false)?;

        persist_from_native_module(native_module, &sierra_program, library_output_path)
    }

    let sierra_contract_class: cairo_lang_starknet_classes::contract_class::ContractClass =
        serde_json::from_str(raw_contract_class)?;

    // todo(rodro): we are having two instances of a sierra program, one it's object form
    // and another in its felt encoded form. This can be avoided by either:
    //   1. Having access to the encoding/decoding functions
    //   2. Refactoring the code on the Cairo mono-repo

    let sierra_program = sierra_contract_class.extract_sierra_program()?;

    // todo(xrvdg) lift this match out of the function once we do not need sierra_program anymore
    let executor = match load_compiled_contract(&sierra_program, library_output_path) {
        Some(executor) => {
            println!("Loaded cached compiled contract from {:?}", library_output_path);
            executor.or_else(|_err| compile_and_load(sierra_program, library_output_path))
        }
        None => {
            compile_and_load(sierra_program, library_output_path)
        },
    }?;

    Ok(NativeContractClassV1::new(executor, sierra_contract_class)?)
}


pub fn class_from_json_str(
    raw_json: &str,
    class_hash: ClassHash,
) -> Result<ContractClass, String> {
    println!("raw json length {}", raw_json.len());
    let class_def = raw_json.to_string();
    println!("class def parsed");

        let class: ContractClass =if let Ok(class) = ContractClassV0::try_from_json_string(class_def.as_str()) {
            class.into()
        } else if let Ok(class) = ContractClassV1::try_from_json_string(class_def.as_str()) {
            println!("v1 contract");
            class.into()
        } else if let Ok(class) = {
            println!("native contract");
            let library_output_path = generate_library_path(class_hash);
            native_try_from_json_string(class_def.as_str(), &library_output_path)
        } {
            class.into()
        } else {
            return Err("not a valid contract class".to_string());
        };

        Ok(class)

    }
