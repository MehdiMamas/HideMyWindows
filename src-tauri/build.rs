fn main() {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("wow64_helper.rs");
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let helper = std::path::Path::new("resources").join("hmw-release-x86.exe");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/hmw-release-x86.exe");

    if arch == "x86_64" && helper.is_file() {
        let abs = std::fs::canonicalize(&helper).expect("wow64 helper path");
        let path = abs.to_string_lossy().replace('\\', "/");
        let path = path.trim_start_matches("//?/").to_string();
        std::fs::write(
            &dest,
            format!(
                "pub static WOW64_HELPER: Option<&[u8]> = Some(include_bytes!(r\"{path}\"));\n"
            ),
        )
        .expect("write wow64 helper include");
    } else {
        std::fs::write(&dest, "pub static WOW64_HELPER: Option<&[u8]> = None;\n")
            .expect("write empty wow64 helper include");
    }

    tauri_build::build();
}
