use cbindgen::Config;
use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let config_path = PathBuf::from(&crate_dir).join("cbindgen.toml");
    let config = Config::from_file(config_path).expect("Failed to parse cbindgen.toml");

    let bindings = match cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
    {
        Ok(bindings) => bindings,
        Err(error) => {
            eprintln!("==== [ ERROR ] ====");
            eprintln!("{error}");
            eprintln!("==== [ ERROR ] ====");
            eprintln!("{error:?}");
            eprintln!("===================");
            std::process::exit(1);
        }
    };
    bindings.write_to_file("netsim.h");
}
