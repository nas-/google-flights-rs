use std::io::Result;

use prost_build::Config;
/// This build script is used to compile the `message.proto` file into Rust code.
fn main() -> Result<()> {
    let mut conf = Config::new();
    conf.protoc_arg("--experimental_allow_proto3_optional");

    conf.compile_protos(&["src/protos/message.proto"], &["src/"])?;
    Ok(())
}
