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
        "src/environment.rs",
        "src/environment_show.rs",
        "src/heap.rs",
        "src/test_ap.rs",
        "src/bench_mdns.rs",
        "../../firmware/pico-w-signal/src/radio.rs",
        "../../firmware/pico-w-signal/memory.x",
    ] {
        println!("cargo:rerun-if-changed={path}");
        let bytes = std::fs::read(path).unwrap();
        sources.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        sources.extend_from_slice(&bytes);
    }
    // Local path dependencies have no content digest in Cargo.lock. Include
    // their shared presentation sources so a changed Face/Look cannot retain
    // the previous firmware's recovery identity.
    fn collect(path: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, files);
            } else if matches!(
                path.extension().and_then(|x| x.to_str()),
                Some("rs" | "conduit")
            ) || path.file_name().is_some_and(|x| x == "Cargo.toml")
            {
                files.push(path);
            }
        }
    }
    let mut presentation = Vec::new();
    for file in [
        "../../../../plots/seven-segment-mask/Cargo.toml",
        "../../../../plots/seven-segment-mask/build.rs",
        "../../../../plots/seven-segment-mask/main.conduit",
        "../../../../plots/seven-segment-mask/types.conduit",
        "../../../../bodies/environment/face/Cargo.toml",
        "../../../../bodies/environment/face/build.rs",
        "../../../../bodies/environment/face/main.conduit",
        "../../../../bodies/environment/measurement/Cargo.toml",
        "../../../../targets/rp2040/network-realization/Cargo.toml",
        "../../../../mechanisms/io/device-carrier-protocol/Cargo.toml",
    ] {
        if std::path::Path::new(file).is_file() {
            println!("cargo:rerun-if-changed={file}");
            presentation.push(std::path::PathBuf::from(file));
        }
    }
    for directory in [
        "../../../../plots/seven-segment-mask/src",
        "../../../../bodies/environment/face/src",
        "../../../../bodies/environment/measurement/src",
        "../../../../targets/rp2040/network-realization/src",
        "../../../../mechanisms/io/device-carrier-protocol/src",
    ] {
        println!("cargo:rerun-if-changed={directory}");
        collect(std::path::Path::new(directory), &mut presentation);
    }
    presentation.sort();
    for path in presentation {
        let name = path.to_str().unwrap().as_bytes();
        sources.extend_from_slice(&(name.len() as u64).to_le_bytes());
        sources.extend_from_slice(name);
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
