use blockifier::execution::contract_class::{
    CompiledClassV0, CompiledClassV1, RunnableCompiledClass,
};
use blockifier::execution::native::contract_class::NativeCompiledClassV1;
use cairo_lang_starknet_classes::contract_class::ContractClass;
use cairo_native::executor::AotContractExecutor;
use cairo_native::OptLevel;
use starknet_api::core::ClassHash;

use lazy_static::lazy_static;
use std::{fs, path::PathBuf};

lazy_static! {
    static ref NATIVE_CACHE_DIR: PathBuf = setup_native_cache_dir();
}

fn generate_library_path(class_hash: ClassHash) -> PathBuf {
    let mut path = NATIVE_CACHE_DIR.clone();
    path.push(class_hash.to_string().trim_start_matches("0x"));
    path
}

fn setup_native_cache_dir() -> PathBuf {
    let path: PathBuf = match std::env::var("NATIVE_CACHE_DIR") {
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

/// Load a compiled native contract into memory
///
/// Tries to load the compiled contract class from library_output_path if it
/// exists, otherwise it will compile the raw_contract_class, load it into memory
/// and save the compilation artifact to library_output_path.
fn native_try_from_json_string(
    raw_contract_class: &str,
    library_output_path: &PathBuf,
) -> Result<NativeCompiledClassV1, Box<dyn std::error::Error>> {
    let sierra_contract_class: ContractClass = serde_json::from_str(raw_contract_class)?;

    let compiled_class = serde_json::from_str(raw_contract_class)?;

    let sierra_program = sierra_contract_class.extract_sierra_program()?;

    let maybe_cached_executor = AotContractExecutor::load(library_output_path);
    if let Ok(executor) = maybe_cached_executor {
        println!("Loaded cached executor");
        let native_class = NativeCompiledClassV1::new(executor, compiled_class);
        return Ok(native_class);
    }

    println!("Creating new executor");
    let mut executor = AotContractExecutor::new(
        &sierra_program,
        &sierra_contract_class.entry_points_by_type,
        OptLevel::Default,
    )?;
    executor.save(library_output_path)?;
    println!("Saved executor to {:?}", library_output_path);

    let native_class = NativeCompiledClassV1::new(executor, compiled_class);
    Ok(native_class)
}

pub fn class_from_json_str(
    raw_json: &str,
    class_hash: ClassHash,
) -> Result<RunnableCompiledClass, String> {
    println!("raw json length {}", raw_json.len());
    let class_def = raw_json.to_string();
    println!("class def parsed");
    let class: RunnableCompiledClass =
        if let Ok(class) = CompiledClassV0::try_from_json_string(class_def.as_str()) {
            class.into()
        } else if let Ok(class) = CompiledClassV1::try_from_json_string(class_def.as_str()) {
            println!("v1 contract");
            class.into()
        } else if let Ok(class) = {
            println!("native contract");
            let library_output_path = generate_library_path(class_hash);
            let maybe_class = native_try_from_json_string(class_def.as_str(), &library_output_path);
            if let Ok(class) = maybe_class {
                Ok(class)
            } else {
                println!(
                    "Native contract failed with error {:?}",
                    maybe_class.err().unwrap()
                );
                Err(())
            }
        } {
            class.into()
        } else {
            return Err("not a valid contract class".to_string());
        };

    Ok(class)
}
