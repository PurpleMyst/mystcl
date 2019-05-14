extern crate bindgen;

use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rustc-link-lib=tcl8.6");
    println!("cargo:rustc-link-lib=tk8.6");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg("-I/usr/include/tcl8.6/")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
