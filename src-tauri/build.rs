use std::path::Path;

/// FNV-1a 64. Names payload folders by content; not a security boundary.
fn content_id(parts: &[&[u8]]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for part in parts {
        for byte in *part {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

/// Bundle the payload at `resources/payload/<content id>/hmw_payload.dll`.
///
/// Windows stores a global hook as (DLL path, offset into the DLL). A process
/// that still has an older build loaded from the same path gets that old
/// module back from `LoadLibrary` and jumps to the new offset inside old
/// code, which crashes it. A path that only ever holds one build avoids that.
fn stage_payload() -> String {
    let source = Path::new("resources").join("hmw_payload.dll");
    println!("cargo:rerun-if-changed=resources/hmw_payload.dll");
    let Ok(bytes) = std::fs::read(&source) else {
        // tauri_build reports the missing resource below.
        return String::new();
    };
    let id = content_id(&[&bytes]);
    let root = Path::new("resources").join("payload");
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy() != id.as_str() {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    let dir = root.join(&id);
    let dest = dir.join("hmw_payload.dll");
    if std::fs::read(&dest).ok().as_deref() != Some(bytes.as_slice()) {
        std::fs::create_dir_all(&dir).expect("create payload folder");
        std::fs::write(&dest, &bytes).expect("stage payload");
    }
    id
}

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
    println!("cargo:rustc-env=HMW_PAYLOAD_ID={}", stage_payload());
    // The x86 files are extracted under %TEMP%; the same rule applies there.
    let wow64_id = match (std::fs::read(&helper), std::fs::read(&payload)) {
        (Ok(helper), Ok(payload)) if arch == "x86_64" => content_id(&[&helper, &payload]),
        _ => String::new(),
    };
    println!("cargo:rustc-env=HMW_WOW64_ID={wow64_id}");

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
