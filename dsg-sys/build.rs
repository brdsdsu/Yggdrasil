use std::{env, path::PathBuf};

fn main() {
    // Build the dsg-shim via CMake
    let dst = cmake::Config::new("./dsg-shim/")
        .define("CMAKE_BUILD_TYPE", "Release")
        .build();

    // Tell cargo to link the produced dsg_c_api
    let libdir = dst.join("lib");
    println!("cargo:rustc-link-search=native={}", libdir.display());
    println!("cargo:rustc-link-lib=dylib=dsg_c_api");

    // Generate bindings
    let bindings = bindgen::Builder::default()
        .header("./dsg-shim/dsg_c_api.h")
        .allowlist_type("dsg_.*")
        .allowlist_function("dsg_.*")
        .clang_arg("-I./dsg-shim")
        .generate()
        .expect("bindgen failed");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
