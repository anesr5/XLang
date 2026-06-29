use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=LLVM_HOME");

    let llvm_dir = env::var_os("LLVM_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\LLVM"));
    let lib_dir = llvm_dir.join("lib");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=LLVM-C");
    println!("cargo:rustc-link-lib=dylib=LTO");
    println!("cargo:rustc-link-lib=dylib=Remarks");
    println!("cargo:rustc-link-lib=dylib=libclang");
}
