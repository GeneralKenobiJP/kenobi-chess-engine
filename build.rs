fn main() {
    let fathom_src = "vendor/fathom/src";

    cc::Build::new()
        .include(fathom_src)
        .file(format!("{fathom_src}/tbprobe.c"))
        .file("native/fathom_shim.c")
        .cpp(true)
        .std("c++17")
        .flag("/TP")
        .compile("fathom");

    println!("cargo:rerun-if-changed=vendor/fathom/src/tbprobe.c");
    println!("cargo:rerun-if-changed=vendor/fathom/src/tbprobe.h");
    println!("cargo:rerun-if-changed=vendor/fathom/src/tbchess.c");
    println!("cargo:rerun-if-changed=vendor/fathom/src/tbconfig.h");
    println!("cargo:rerun-if-changed=vendor/fathom/src/stdendian.h");
    println!("cargo:rerun-if-changed=native/fathom_shim.c");
}