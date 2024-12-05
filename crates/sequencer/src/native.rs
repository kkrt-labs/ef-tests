use blockifier::execution::contract_class::{
    CompiledClassV0, CompiledClassV1, RunnableCompiledClass,
};
use blockifier::execution::native::contract_class::NativeCompiledClassV1;
use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_lang_starknet_classes::contract_class::ContractClass as SierraContractClass;
use cairo_native::executor::AotContractExecutor;
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
/// exists, otherwise it will compile the raw_sierra_class, load it into memory
/// and save the compilation artifact to library_output_path.
fn native_try_from_json_string(
    raw_sierra_class: &str,
    library_output_path: &PathBuf,
) -> Result<NativeCompiledClassV1, Box<dyn std::error::Error>> {
    let maybe_cached_executor = AotContractExecutor::load(library_output_path);

    // see blockifier/src/test_utils/struct_impls.rs
    let sierra_contract_class: SierraContractClass =
        serde_json::from_str(raw_sierra_class).unwrap();

    let sierra_program = sierra_contract_class
        .extract_sierra_program()
        .expect("Cannot extract sierra program from sierra contract class");

    // Compile the sierra contract class into casm
    let casm_contract_class =
        CasmContractClass::from_contract_class(sierra_contract_class.clone(), false, usize::MAX)
            .expect("Cannot compile sierra contract class into casm contract class");
    let casm = CompiledClassV1::try_from(casm_contract_class)
        .expect("Cannot get CompiledClassV1 from CasmContractClass");

    if let Ok(executor) = maybe_cached_executor {
        println!("Loading cached executor");
        let native_class = NativeCompiledClassV1::new(executor, casm);
        return Ok(native_class);
    }

    println!("Creating new executor");
    let start_time = std::time::Instant::now();
    let mut executor = AotContractExecutor::new(
        &sierra_program,
        &sierra_contract_class.entry_points_by_type,
        cairo_native::OptLevel::Default,
    )
    .expect("Cannot compile sierra into native");
    let duration = start_time.elapsed();
    executor.save(library_output_path)?;
    println!("Created and saved AoTExecutor in {:.2?}", duration);

    let native_class = NativeCompiledClassV1::new(executor, casm);
    Ok(native_class)
}

pub fn class_from_json_str(
    raw_sierra: &str,
    class_hash: ClassHash,
) -> Result<RunnableCompiledClass, String> {
    let class_def = raw_sierra.to_string();
    let class: RunnableCompiledClass =
        if let Ok(class) = CompiledClassV0::try_from_json_string(class_def.as_str()) {
            class.into()
        } else if let Ok(class) = CompiledClassV1::try_from_json_string(class_def.as_str()) {
            class.into()
        } else if let Ok(class) = {
            let library_output_path = generate_library_path(class_hash);
            let maybe_class = native_try_from_json_string(class_def.as_str(), &library_output_path);

            maybe_class.map_or_else(
                |err| {
                    println!("Native contract failed with error {:?}", err);
                    Err(())
                },
                Ok,
            )
        } {
            class.into()
        } else {
            return Err("not a valid contract class".to_string());
        };

    Ok(class)
}
