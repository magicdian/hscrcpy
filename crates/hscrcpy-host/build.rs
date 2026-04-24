fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/scrcpy.proto");

    let mut prost_config = tonic_prost_build::Config::new();
    prost_config.default_package_filename("scrcpy");

    tonic_prost_build::configure()
        .build_server(false)
        .btree_map(".")
        .compile_with_config(prost_config, &["proto/scrcpy.proto"], &["proto"])?;

    Ok(())
}
