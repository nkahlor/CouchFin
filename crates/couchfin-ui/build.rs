//! Compiles the Slint markup in `ui/` into Rust at build time.

fn main() -> Result<(), slint_build::CompileError> {
    // Debug info lets tests find elements through the testing backend's
    // `ElementHandle` API. It only adds element names to the binary.
    let config = slint_build::CompilerConfiguration::new().with_debug_info(true);
    slint_build::compile_with_config("ui/hello.slint", config)
}
