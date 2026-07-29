use std::env;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

const FIRMWARE_INPUTS: [&str; 11] = [
    "../../Cargo.lock",
    "../../Cargo.toml",
    "../../crates/conduit-core/Cargo.toml",
    "../../crates/conduit-core/src",
    "../../crates/conduit-embedded/Cargo.toml",
    "../../crates/conduit-embedded/src/lib.rs",
    "Cargo.toml",
    "build.rs",
    "memory.x",
    "src/lib.rs",
    "src/main.rs",
];

fn main() {
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    std::fs::copy("memory.x", output.join("memory.x")).expect("copy RP2040 memory layout");
    let mut digest = Sha256::new();
    for path in FIRMWARE_INPUTS {
        let input = Path::new(path);
        if input.is_dir() {
            let mut files = Vec::new();
            collect_files(input, input, &mut files);
            files.sort();
            for relative in files {
                let label = format!("{path}/{}", relative.display());
                hash_file(&mut digest, &label, &input.join(relative));
            }
        } else {
            hash_file(&mut digest, path, input);
        }
    }
    let target = env::var("TARGET").expect("Cargo supplies TARGET");
    let profile = env::var("PROFILE").expect("Cargo supplies PROFILE");
    let rustc = env::var_os("RUSTC").expect("Cargo supplies RUSTC");
    let rustc_version = Command::new(rustc)
        .arg("-vV")
        .output()
        .expect("query rustc version");
    assert!(rustc_version.status.success(), "rustc -vV must succeed");
    hash_bytes(&mut digest, "cargo-target", target.as_bytes());
    hash_bytes(&mut digest, "cargo-profile", profile.as_bytes());
    hash_bytes(&mut digest, "rustc-version", &rustc_version.stdout);
    let digest = digest.finalize();
    let mut generated = String::from(
        "pub const FIRMWARE_IDENTITY: conduit_core::SemanticHash = \
         conduit_core::SemanticHash::from_bytes([",
    );
    for (index, byte) in digest.iter().enumerate() {
        if index > 0 {
            generated.push(',');
        }
        write!(generated, "{byte}").expect("write generated firmware identity");
    }
    generated.push_str("]);\n");
    fs::write(output.join("firmware_identity.rs"), generated)
        .expect("write generated firmware identity");
    println!("cargo:rustc-env=CONDUIT_FIRMWARE_TARGET={target}");
    println!("cargo:rustc-env=CONDUIT_FIRMWARE_PROFILE={profile}");
    println!("cargo:rustc-link-search={}", output.display());
    if target == "thumbv6m-none-eabi" {
        println!("cargo:rustc-link-arg=-Tlink.x");
    }
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read firmware input directory") {
        let path = entry.expect("read firmware input entry").path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else {
            files.push(
                path.strip_prefix(root)
                    .expect("firmware input remains below root")
                    .to_owned(),
            );
        }
    }
}

fn hash_file(digest: &mut Sha256, label: &str, path: &Path) {
    let bytes = fs::read(path).expect("read firmware identity input");
    hash_bytes(digest, label, &bytes);
    println!("cargo:rerun-if-changed={}", path.display());
}

fn hash_bytes(digest: &mut Sha256, label: &str, bytes: &[u8]) {
    digest.update(label.as_bytes());
    digest.update([0]);
    digest.update(
        u64::try_from(bytes.len())
            .expect("firmware input length")
            .to_be_bytes(),
    );
    digest.update(bytes);
}
