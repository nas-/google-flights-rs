use std::io::Result;

use prost_build::Config;
fn main() -> Result<()> {
    let mut conf = Config::new();
    conf.protoc_arg("--experimental_allow_proto3_optional");

    conf.compile_protos(&["src/message.proto"], &["src/"])?;
    Ok(())
}
