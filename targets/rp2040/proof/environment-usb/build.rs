//! The actual Host Conduitese checker/identity feeds executable pin selection.
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};
fn collect(root: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            collect(&p, out)
        } else if matches!(
            p.extension().and_then(|s| s.to_str()),
            Some("rs" | "conduit" | "toml")
        ) {
            out.push(p)
        }
    }
}
fn main() {
    println!("cargo:rerun-if-env-changed=CONDUIT_HOST_CONFIGURATION");
    let profile = env::var_os("CONDUIT_HOST_CONFIGURATION")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../../profiles/pico-environment-usb.host.conduit"));
    let source = fs::read_to_string(&profile).unwrap();
    let packages =
        conduit_host_make::MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage])
            .unwrap();
    let catalog = conduit_host_make::MakeCatalog::canonical().with_packages(&packages);
    let configuration =
        conduit_host_make::parse_host_configuration_conduit(&source).expect("Host Conduitese");
    let checked = conduit_host_make::check_host_configuration(configuration, &catalog, &packages)
        .expect("checked Host configuration");
    assert_eq!(
        checked.configuration().target.machine == "pico-w",
        env::var_os("CARGO_FEATURE_WIFI").is_some(),
        "checked Pico W Host requires wifi feature; plain Pico must omit it"
    );
    let generated =
        conduit_host_rp2040::environment_usb_rust(&checked).expect("RP2040 physical bindings");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("host.rs"), &generated).unwrap();
    let memory = fs::read_to_string("../../firmware/pico-w-signal/memory.x").unwrap();
    fs::write(out.join("memory.x"), memory).unwrap();
    fs::write(out.join("bounds.x"),format!("ASSERT(__euninit - __sdata <= {}, \"Host static memory bound exceeded\");\nASSERT(_stack_start - __euninit >= 16384, \"USB Host stack below16KiB\");\n",checked.configuration().limits.static_memory_bytes)).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-link-arg=-T{}", out.join("bounds.x").display());
    println!("cargo:rustc-link-arg=--nmagic");
    let mut files = vec![
        profile,
        PathBuf::from("../../firmware/pico-w-signal/memory.x"),
        PathBuf::from("../usb-http-relay/src/lib.rs"),
    ];
    for path in [
        "src",
        "../../make/src",
        "../../../../make/host/src",
        "../../../../mechanisms/io/ht16k33/src",
        "../../../../mechanisms/io/bosch-env/src",
        "../../../../bodies/environment/face/src",
        "../../../../bodies/environment/measurement/src",
        "../../../../plots/seven-segment-mask/src",
    ] {
        collect(Path::new(path), &mut files)
    }
    for p in [
        "Cargo.toml",
        "build.rs",
        "../../../../plots/seven-segment-mask/main.conduit",
        "../../../../plots/seven-segment-mask/types.conduit",
    ] {
        files.push(p.into())
    }
    // Local dependencies are not content-pinned by Cargo.lock. Their generated
    // Types, features and renderers are part of this exact recovery build.
    let mut optional = Vec::new();
    for directory in [
        "../../make",
        "../../../../make/host",
        "../../../../mechanisms/io/ht16k33",
        "../../../../mechanisms/io/bosch-env",
        "../../../../bodies/environment/face",
        "../../../../bodies/environment/measurement",
        "../../../../plots/seven-segment-mask",
    ] {
        files.push(Path::new(directory).join("Cargo.toml"));
        optional.push(Path::new(directory).join("build.rs"));
    }
    files.push("Cargo.lock".into());
    if env::var_os("CARGO_FEATURE_WIFI").is_some() {
        files.push("../../firmware/pico-w-signal/src/radio.rs".into());
        collect(Path::new("../../../../semantics/net/src"), &mut files);
        files.push("../../../../semantics/net/Cargo.toml".into());
        for name in [
            "43439A0.bin",
            "nvram_rp2040.bin",
            "43439A0_clm.bin",
            "LICENSE-permissive-binary-license-1.0.txt",
        ] {
            files.push(
                Path::new(
                    "../../firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab",
                )
                .join(name),
            );
        }
    }
    files.sort();
    let mut hash = Sha256::new();
    hash.update(generated.as_bytes());
    for p in optional {
        println!("cargo:rerun-if-changed={}", p.display());
        hash.update(p.to_string_lossy().as_bytes());
        match fs::read(&p) {
            Ok(bytes) => {
                hash.update(b"present");
                hash.update((bytes.len() as u64).to_le_bytes());
                hash.update(bytes);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                hash.update(b"absent");
            }
            Err(error) => panic!("cannot hash {}: {error}", p.display()),
        }
    }
    for p in files {
        println!("cargo:rerun-if-changed={}", p.display());
        let bytes = fs::read(&p).unwrap();
        hash.update(p.to_string_lossy().as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    println!(
        "cargo:rustc-env=CONDUIT_RELAY_BUILD_ID={:x}",
        hash.finalize()
    );
}
