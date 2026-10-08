fn main() {
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-changed=vendor/detours");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .include("vendor/detours")
        .define("WIN32_LEAN_AND_MEAN", None)
        .define("NOMINMAX", None)
        .file("native/protection.cpp");
    for file in [
        "detours",
        "modules",
        "image",
        "creatwth",
        "disasm",
        "disolx86",
        "disolx64",
        "disolia64",
        "disolarm",
        "disolarm64",
    ] {
        build.file(format!("vendor/detours/{file}.cpp"));
    }
    build
        .flag_if_supported("/std:c++17")
        .warnings(false)
        .compile("hmw_protection");
}
