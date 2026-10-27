fn main() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut sources = Vec::new();
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "src/lib.rs",
        "src/main.rs",
        "../../firmware/pico-w-signal/src/radio.rs",
        "../../firmware/pico-w-signal/memory.x",
    ] {
        println!("cargo:rerun-if-changed={path}");
        let bytes = std::fs::read(path).unwrap();
        sources.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        sources.extend_from_slice(&bytes);
    }
    let assets = "../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab";
    let mut files = std::fs::read_dir(assets)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file())
        .collect::<Vec<_>>();
    files.sort();
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        sources.extend_from_slice(&std::fs::read(path).unwrap());
    }
    let mut hash = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum build identity");
    hash.stdin.take().unwrap().write_all(&sources).unwrap();
    let result = hash.wait_with_output().unwrap();
    assert!(result.status.success());
    let digest = std::str::from_utf8(&result.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    assert_eq!(digest.len(), 64);
    println!("cargo:rustc-env=CONDUIT_RELAY_BUILD_ID={digest}");
    println!("cargo:rerun-if-changed=../../firmware/pico-w-signal/memory.x");
    if std::env::var("TARGET").unwrap().starts_with("thumbv6m-") {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        std::fs::write(
            out.join("memory.x"),
            include_bytes!("../../firmware/pico-w-signal/memory.x"),
        )
        .unwrap();
        println!("cargo:rustc-link-search={}", out.display());
        println!("cargo:rustc-link-arg=-Tlink.x");
    }
}
