use std::env::VarError;
use std::str::FromStr;

const SIZE_ENV_VAR: &str = "FSHARING_SIZE";

fn main() {
    println!("cargo::rerun-if-env-changed={SIZE_ENV_VAR}");
    println!("cargo::rerun-if-changed=");
    let size_str = match std::env::var(SIZE_ENV_VAR) {
        Ok(size) => size,
        Err(VarError::NotPresent) => "8".to_owned(),
        Err(VarError::NotUnicode(e)) => {
            println!("cargo::error=Invalid value of {SIZE_ENV_VAR}: {e:?}");
            std::process::exit(1);
        }
    };
    match usize::from_str(&size_str) {
        Ok(size) => {
            save_params(size);
        }
        Err(_) => {
            println!("cargo::error=Invalid value of {SIZE_ENV_VAR}: {size_str:?}");
            std::process::exit(1);
        }
    }
}

fn save_params(size: usize) {
    let params_path = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("build is not from cargo"),
    )
    .join("src/params.rs");
    let contents = format!(
        "pub(crate) const N: usize = 1_000_000_000;
pub(crate) const SIZE: usize = {size};
"
    );
    if let Err(e) = std::fs::write(&params_path, &contents) {
        println!("cargo::error=Failed to save the file {params_path:?}: {e}");
    }
}
