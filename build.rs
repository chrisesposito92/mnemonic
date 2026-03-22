// build.rs
fn main() {
    // Only run proto codegen when interface-grpc feature is active.
    // IMPORTANT: cfg!(feature = "...") does NOT work in build scripts.
    // Build scripts are compiled as standalone binaries — feature flags
    // propagate via CARGO_FEATURE_ environment variables, not cfg attributes.
    if std::env::var("CARGO_FEATURE_INTERFACE_GRPC").is_err() {
        return;
    }

    // Explicit rerun-if-changed BEFORE compile_protos.
    // Prevents always-dirty incremental build bug (tonic issue #2239):
    // compile_protos may emit cargo:rerun-if-changed with an unresolvable
    // path. Our explicit directive ensures Cargo tracks the real file.
    println!("cargo:rerun-if-changed=proto/mnemonic.proto");
    println!("cargo:rerun-if-changed=build.rs");

    // Switch to builder pattern to support file_descriptor_set_path for tonic-reflection.
    // The shorthand compile_protos() does NOT support file_descriptor_set_path.
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("mnemonic_descriptor.bin"))
        .compile_protos(&["proto/mnemonic.proto"], &["proto"])
        .expect("Failed to compile proto/mnemonic.proto");
}
