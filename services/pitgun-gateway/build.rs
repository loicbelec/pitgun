fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/telemetry.proto");

    if std::env::var_os("CARGO_FEATURE_PROTOBUF").is_none() {
        return Ok(());
    }

    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    // Build scripts can point prost-build at the bundled protoc via env.
    // SAFETY: build scripts may mutate their own process environment before spawning tools.
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    prost_build::compile_protos(&["proto/telemetry.proto"], &["proto"])?;

    Ok(())
}
