fn main() {
    println!("cargo:rerun-if-changed=schemas/petfilter.fbs");

    // Regenerate FlatBuffers Rust bindings if flatc is on PATH.
    // If flatc is absent, the checked-in generated code is used as-is.
    if let Ok(output) = std::process::Command::new("flatc")
        .args(["--rust", "-o", "src/rpc/generated", "schemas/petfilter.fbs"])
        .output()
    {
        if output.status.success() {
            println!("cargo:warning=flatc: regenerated petfilter_generated.rs");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("cargo:warning=flatc codegen failed: {stderr}");
        }
    }

    embuild::espidf::sysenv::output();
}
