use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn find_lib_flac() -> Option<PathBuf> {
    let standard_dirs = [
        PathBuf::from("/usr/lib"),
        PathBuf::from("/usr/lib64"),
        PathBuf::from("/usr/local/lib"),
        PathBuf::from("/usr/lib/x86_64-linux-gnu"),
        PathBuf::from("/usr/lib/aarch64-linux-gnu"),
    ];

    for dir in &standard_dirs {
        let candidate = dir.join("libFLAC.so");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // Also inspect any multiarch subdirectories in /usr/lib
    if let Ok(entries) = fs::read_dir("/usr/lib") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let candidate = path.join("libFLAC.so");
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR must be set");
    let out_path = Path::new(&out_dir);

    if let Some(flac_so) = find_lib_flac() {
        let symlink_path = out_path.join("libflac.so");
        let _ = fs::remove_file(&symlink_path);
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            if symlink(&flac_so, &symlink_path).is_ok() {
                println!("cargo:rustc-link-search=native={}", out_dir);
            }
        }
    }
}
