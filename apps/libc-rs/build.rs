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
    Command::new("make")
        .arg("-C")
        .arg("../libc")
        .status()
        .expect("Failed to run make");

    println!("cargo::rustc-link-search=../libc");
    println!("cargo::rustc-link-lib=c");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = std::path::PathBuf::from(out_dir);

    let libc_path = PathBuf::from("../libc");
    let headers = find_headers_recursively(libc_path);
    let mut builder = bindgen::Builder::default();

    for header in headers {
        builder = builder.header(header.to_str().unwrap());
    }

    let bindings = builder
        .use_core()
        .generate()
        .expect("Failed to generate bindings");
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Fialed to wrote bindings");
}
