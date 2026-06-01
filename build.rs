use std::io::Result;

use prost_build::Config;

/// This build script compiles `message.proto` into Rust code.
///
/// `protoc-bin-vendored` bundles a pre-built `protoc` binary so the build
/// works on CI and developer machines without requiring a system-level protoc
/// installation.
fn main() -> Result<()> {
    // Point prost-build at the vendored protoc binary so the build never
    // requires a system-level installation.
    std::env::set_var(
        "PROTOC",
        protoc_bin_vendored::protoc_bin_path()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?,
    );

    let mut conf = Config::new();
    conf.protoc_arg("--experimental_allow_proto3_optional");
    conf.compile_protos(&["src/protos/message.proto"], &["src/"])?;
    Ok(())
}
