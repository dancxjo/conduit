//! Explicit, digest-bound handoff from the external pinned C build. No download.
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};
pub fn prepare(out: &Path) -> Vec<PathBuf> {
    for name in [
        "CONDUIT_ZENOH_EVAL_ARCHIVE",
        "CONDUIT_ZENOH_EVAL_LOCATOR",
        "CONDUIT_ZENOH_EVAL_CONNECT_TIMEOUT_MS",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let archive = PathBuf::from(
        env::var_os("CONDUIT_ZENOH_EVAL_ARCHIVE").expect("explicit ARM evaluation archive"),
    );
    let manifest = archive.with_extension("manifest.json");
    let proof: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).expect("archive source manifest")).unwrap();
    assert_eq!(proof["schema"], "conduit.zenoh-pico-evaluation-archive@1");
    assert_eq!(
        proof["upstream"],
        "6a881c21dad1f778d0529eb9f297bea339016526"
    );
    let bytes = fs::read(&archive).unwrap();
    assert_eq!(
        proof["archive_sha256"],
        format!("{:x}", Sha256::digest(&bytes))
    );
    let inputs = [
        ("driver", "src/zenoh_evaluation.c"),
        ("pal", "../../../embassy/zenoh-pico-platform/c/platform.c"),
        (
            "types",
            "../../../embassy/zenoh-pico-platform/include/conduit_zenoh_pico_types.h",
        ),
        (
            "callbacks",
            "../../../embassy/zenoh-pico-platform/include/conduit_zenoh_pico_callbacks.h",
        ),
        (
            "patch",
            "../../../embassy/zenoh-pico-platform/patches/zenoh-pico-1.10.2-send-oom.patch",
        ),
    ];
    let mut files = vec![archive, manifest];
    for (key, path) in inputs {
        let path = PathBuf::from(path);
        assert_eq!(
            proof["source_sha256"][key],
            format!("{:x}", Sha256::digest(fs::read(&path).unwrap())),
            "stale C archive source: {key}"
        );
        files.push(path);
    }
    let locator = env::var("CONDUIT_ZENOH_EVAL_LOCATOR")
        .expect("explicit test router locator, not execution authority");
    let target = locator.strip_prefix("tcp/").expect("numeric TCP locator");
    let (address, port) = target.rsplit_once(':').expect("IPv4:port");
    let ip: std::net::Ipv4Addr = address.parse().expect("numeric IPv4");
    let port: u16 = port.parse().expect("numeric port");
    assert!(
        ip.is_private() && port != 0,
        "private unicast evaluation router"
    );
    // Upstream's default socket timeout is only 100ms. Keep a separately
    // explicit connect budget, bounded by this PAL's existing ten-second cap.
    let timeout: u32 = env::var("CONDUIT_ZENOH_EVAL_CONNECT_TIMEOUT_MS")
        .expect("explicit evaluation TCP connect budget")
        .parse()
        .expect("numeric TCP connect budget");
    assert!(
        (1..=10_000).contains(&timeout),
        "TCP connect budget exceeds PAL bound"
    );
    let generated = out.join("zenoh.rs");
    fs::write(
        &generated,
        format!("pub const LOCATOR:&[u8]=b\"tcp/{ip}:{port}#tout={timeout}\\0\";\n"),
    )
    .unwrap();
    files.push(generated);
    fs::write(out.join("libzenoh_evaluation.a"), bytes).unwrap();
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=zenoh_evaluation");
    for directory in [
        "../../../embassy/zenoh-pico-platform/src",
        "../../../../mechanisms/io/zenoh-pico-platform/src",
    ] {
        collect(Path::new(directory), &mut files);
    }
    files
}
fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}
