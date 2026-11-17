//! Qualify only the private std producers used by one fixed-environment process.
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf, process::Command};
const COMMIT: &str = "48a229ceaefd4985c50990b14116b6d856af0985";
const SOURCES: &[(&str, &str)] = &[
    (
        "alloc/src/slice.rs",
        "b1f0a5f6e3d104d6176699cf05efdfe1725095cd984fdf4422ddc3093726361c",
    ),
    (
        "std/src/sys/process/unix/unix.rs",
        "19eb534c196a88289e6d65f56af0b1c584f294cc67e80bf984be9f2b2cb9e926",
    ),
    (
        "std/src/sys/process/unix/common.rs",
        "6f8eecf4228f479c403b098a5a4392625f50ba7075d61a5674a64b4e3d82e143",
    ),
    (
        "std/src/sys/process/unix/common/cstring_array.rs",
        "da58be83d8e01123bcac5016a9764db12f578d02ce6c49879cc148ab1216ef7a",
    ),
    (
        "std/src/sys/process/env.rs",
        "7f3844cdb47f284e399161b65d05ae314a6764a566e9ef46733762704418ae69",
    ),
    (
        "alloc/src/collections/btree/node.rs",
        "2200ca0a4fe38592c314dc7f5f40bbb160e06b6f9e4abd1eb5bc40701540ab25",
    ),
    (
        "alloc/src/collections/btree/map.rs",
        "2ce3f3468c9686421c216a74b38211fefdfe506c794b3c350bbf69ce3fb71a19",
    ),
    (
        "alloc/src/ffi/c_str.rs",
        "a9a4e636ee388b4c9690ea643f2a6ea1d1f406431fdd5f0cb86fea5c9f465178",
    ),
    (
        "alloc/src/vec/mod.rs",
        "09171c3c4c767a5c0716a85786f7670e5e149f75325fc1b27f5dbce14f16711f",
    ),
    (
        "alloc/src/raw_vec/mod.rs",
        "a262b9e494236a00270be0dd0c57be5259d62bc1e538c56506f7f0ce8debccc0",
    ),
    (
        "std/src/ffi/os_str.rs",
        "fc18fd9479db9a95a94d96bd7b4c5ac5b479fbef97d13c2545aebebbbb2af9c0",
    ),
    (
        "std/src/sys/os_str/bytes.rs",
        "b8db748fa98d57b9c5358dd0ad48c4950691defea188786b0d17c2c23533a721",
    ),
];
fn qualification() -> Result<(), &'static str> {
    let flags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    if flags.contains("randomize-layout") || flags.contains("layout-seed") {
        return Err("funded process layout rejects unreviewed randomized Rust layout");
    }
    let target = env::var("TARGET").map_err(|_| "missing target")?;
    if !matches!(
        target.as_str(),
        "x86_64-unknown-linux-gnu" | "aarch64-unknown-linux-gnu"
    ) {
        return Err("funded process layout requires reviewed Linux GNU 64-bit target");
    }
    let rustc = env::var_os("RUSTC").ok_or("missing compiler")?;
    let version = Command::new(&rustc)
        .arg("-vV")
        .output()
        .map_err(|_| "compiler identity unavailable")?;
    let version = std::str::from_utf8(&version.stdout).map_err(|_| "compiler identity invalid")?;
    if !version
        .lines()
        .any(|line| line == format!("commit-hash: {COMMIT}"))
    {
        return Err("funded process layout requires reviewed Rust 1.98.1 commit");
    }
    let sysroot = Command::new(&rustc)
        .args(["--print", "sysroot"])
        .output()
        .map_err(|_| "sysroot unavailable")?;
    let sysroot = std::str::from_utf8(&sysroot.stdout).map_err(|_| "sysroot invalid")?;
    let library = PathBuf::from(sysroot.trim()).join("lib/rustlib/src/rust/library");
    for (path, expected) in SOURCES {
        let path = library.join(path);
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = fs::read(path)
            .map_err(|_| "reviewed rust-src unavailable; use repository toolchain components")?;
        let actual = format!("{:x}", Sha256::digest(bytes));
        if &actual != expected {
            return Err("reviewed std process allocation source changed");
        }
    }
    Ok(())
}
fn main() {
    println!("cargo:rustc-check-cfg=cfg(conduit_prepared_process_layout)");
    println!("cargo:rerun-if-env-changed=RUSTC");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    match qualification() {
        Ok(()) => {
            println!("cargo:rustc-cfg=conduit_prepared_process_layout");
            println!("cargo:rustc-env=CONDUIT_PREPARED_PROCESS_LAYOUT=qualified");
        }
        Err(reason) => println!("cargo:rustc-env=CONDUIT_PREPARED_PROCESS_LAYOUT={reason}"),
    }
}
