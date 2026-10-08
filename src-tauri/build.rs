fn main() {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("wow64_helper.rs");
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let helper = std::path::Path::new("resources").join("hmw-release-x86.exe");
    let payload = std::path::Path::new("resources").join("hmw_payload_x86.dll");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/hmw-release-x86.exe");
    println!("cargo:rerun-if-changed=resources/hmw_payload_x86.dll");
    if arch == "x86_64" && std::env::var("PROFILE").as_deref() == Ok("release") {
        assert!(helper.is_file() && payload.is_file(),
            "Build and copy the x86 helper and payload before the x64 release app (see BUILDING.md)");
    }

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

    let payload_dest = out_dir.join("wow64_payload.rs");
    if arch == "x86_64" && payload.is_file() {
        let abs = std::fs::canonicalize(&payload).expect("wow64 payload path");
        let path = abs.to_string_lossy().replace('\\', "/");
        let path = path.trim_start_matches("//?/");
        std::fs::write(
            payload_dest,
            format!(
                "pub static WOW64_PAYLOAD: Option<&[u8]> = Some(include_bytes!(r\"{path}\"));\n"
            ),
        )
        .expect("write wow64 payload include");
    } else {
        std::fs::write(
            payload_dest,
            "pub static WOW64_PAYLOAD: Option<&[u8]> = None;\n",
        )
        .expect("write empty wow64 payload include");
    }

    tauri_build::build();
}
