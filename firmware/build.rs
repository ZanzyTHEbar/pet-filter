fn main() {
    if let Ok(output) = std::process::Command::new("date")
        .arg("-u")
        .arg("+%Y-%m-%dT%H:%M:%SZ")
        .output()
    {
        if output.status.success() {
            let ts = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=BUILD_TIMESTAMP={ts}");
        }
    }

    println!("cargo:rerun-if-changed=schemas/petfilter.fbs");

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

    #[cfg(feature = "espidf")]
    embuild::espidf::sysenv::output();
}
