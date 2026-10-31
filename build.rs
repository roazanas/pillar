fn main() {
    let target = std::env::var("TARGET").expect("Cargo TARGET");
    cc::Build::new()
        .file("runtime.c")
        .std("c11")
        .warnings(true)
        .cargo_metadata(false)
        .compile("pillar_runtime");
    let archive = if target.contains("msvc") {
        "pillar_runtime.lib"
    } else {
        "libpillar_runtime.a"
    };
    println!(
        "cargo:rustc-env=PILLAR_RUNTIME={}/{archive}",
        std::env::var("OUT_DIR").expect("Cargo OUT_DIR")
    );
    println!("cargo:rustc-env=PILLAR_TARGET={target}");
    println!("cargo:rerun-if-changed=runtime.c");
}
