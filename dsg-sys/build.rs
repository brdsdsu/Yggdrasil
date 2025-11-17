use std::{env, path::PathBuf};

fn main() {

    // Allow the user to point to Spark-DSG via env vars.
    // Set CMAKE_PREFIX_PATH if Spark-DSG is installed to a prefix.
    let cmake_prefix_path = env::var("CMAKE_PREFIX_PATH").unwrap_or_default();

    // Build the capi via CMake
    let dst = cmake::Config::new("./dsg-shim/")
        .define("CMAKE_BUILD_TYPE", "Release")
        .define("CMAKE_PREFIX_PATH", cmake_prefix_path)
        .build();

    // Tell cargo to link the produced dsg_c_api
    let libdir = dst.join("lib");
    println!("cargo:rustc-link-search=native={}", libdir.display());
    println!("cargo:rustc-link-lib=dylib=dsg_c_api");

    // in build.rs (after cmake build)
    // println!("cargo:rustc-link-lib=dylib=spark_dsg");

    // If spark_dsg was a shared lib not pulled transitively, you might need:
    // println!("cargo:rustc-link-lib=dylib=spark_dsg");

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
