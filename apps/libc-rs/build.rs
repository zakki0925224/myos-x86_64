use std::{fs, path::PathBuf, process::Command};

fn find_headers_recursively(dir: PathBuf) -> Vec<PathBuf> {
    let mut headers = Vec::new();

    for entry in fs::read_dir(&dir).expect("Failed to read directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if path.is_dir() {
            headers.extend(find_headers_recursively(path));
        } else if let Some(ext) = path.extension() {
            if ext == "h" {
                headers.push(path);
            }
        }
    }

    headers
}

fn main() {
    let is_for_kernel = std::env::var("CARGO_FEATURE_FOR_KERNEL_STUBS").is_ok();

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = std::path::PathBuf::from(out_dir);

    let libc_path = PathBuf::from("../libc");

    println!(
        "cargo:rerun-if-changed={}",
        libc_path.join("Makefile").display()
    );

    let headers = find_headers_recursively(libc_path.clone());
    if headers.is_empty() {
        Command::new("make")
            .arg("-C")
            .arg("../libc")
            .status()
            .expect("Failed to run make");
    }

    let libc_abs_path = libc_path
        .canonicalize()
        .expect("Failed to get absolute path");
    println!("cargo:rustc-link-search={}", libc_abs_path.display());
    if !is_for_kernel {
        println!("cargo:rustc-link-lib=static=c_with_main");
    }

    let headers = find_headers_recursively(libc_path);
    let mut builder = bindgen::Builder::default();

    for header in headers {
        println!("cargo:rerun-if-changed={}", header.display());
        builder = builder.header(header.to_str().unwrap());
    }

    let bindings = builder
        .use_core()
        .generate()
        .expect("Failed to generate bindings");
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Failed to write bindings");
}
