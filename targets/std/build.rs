//! Build-only qualification of the collection layout used by the closed native
//! preparation recipe. This is not Host, Body, Line or execution authority.
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf, process::Command};

const REVISION: &str = "48a229ceaefd4985c50990b14116b6d856af0985";
const SOURCES: &[(&str, &str)] = &[
    (
        "alloc/src/collections/btree/node.rs",
        "2200ca0a4fe38592c314dc7f5f40bbb160e06b6f9e4abd1eb5bc40701540ab25",
    ),
    (
        "alloc/src/raw_vec/mod.rs",
        "a262b9e494236a00270be0dd0c57be5259d62bc1e538c56506f7f0ce8debccc0",
    ),
    (
        "alloc/src/vec/mod.rs",
        "09171c3c4c767a5c0716a85786f7670e5e149f75325fc1b27f5dbce14f16711f",
    ),
    (
        "alloc/src/vec/spec_from_iter_nested.rs",
        "f3c1a06f43734a65e102b6a107e2fd5adadd7d94d9a226d6ffe1610d1e7e1466",
    ),
    (
        "core/src/iter/adapters/mod.rs",
        "1ca887977e92e291f2d576743ad21d8f07268c2def688589c1efb01685c585ae",
    ),
    (
        "core/src/slice/sort/stable/mod.rs",
        "3f2b2d8543a1cb91c9352e041bed49949a31cbd4b1030ec1a82cca1304cb22d3",
    ),
];
fn value(name: &str) -> String {
    env::var(name).unwrap_or_default()
}
fn present(name: &str) -> bool {
    env::var_os(name).is_some_and(|s| !s.is_empty())
}
fn compiler_output(rustc: &std::ffi::OsStr, args: &[&str]) -> Result<String, &'static str> {
    let result = Command::new(rustc)
        .args(args)
        .output()
        .map_err(|_| "compiler-unavailable")?;
    if !result.status.success() {
        return Err("compiler-query-failed");
    }
    String::from_utf8(result.stdout).map_err(|_| "compiler-output-invalid")
}
fn qualify() -> Result<(), &'static str> {
    let target = value("TARGET");
    let arch = value("CARGO_CFG_TARGET_ARCH");
    if !matches!(
        (target.as_str(), arch.as_str()),
        ("x86_64-unknown-linux-gnu", "x86_64") | ("aarch64-unknown-linux-gnu", "aarch64")
    ) || value("CARGO_CFG_TARGET_POINTER_WIDTH") != "64"
        || value("CARGO_CFG_TARGET_ENDIAN") != "little"
        || value("CARGO_CFG_TARGET_OS") != "linux"
        || value("CARGO_CFG_TARGET_ENV") != "gnu"
        || present("CARGO_CFG_TARGET_ABI")
    {
        return Err("target-abi-unsupported");
    }
    // Do not try to classify arbitrary compiler flags or wrapper behavior. A
    // custom build remains supported, but it gets no closed-layout witness.
    for name in [
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTFLAGS",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTC_BOOTSTRAP",
    ] {
        if present(name) {
            return Err("unqualified-compiler-options");
        }
    }
    let rustc = env::var_os("RUSTC").ok_or("compiler-not-selected")?;
    let version = compiler_output(&rustc, &["--version", "--verbose"])?;
    if !version.lines().any(|line| line == "release: 1.98.1")
        || !version
            .lines()
            .any(|line| line.strip_prefix("commit-hash: ") == Some(REVISION))
    {
        return Err("compiler-revision-unqualified");
    }
    let root = compiler_output(&rustc, &["--print", "sysroot"])?;
    let library = PathBuf::from(root.trim()).join("lib/rustlib/src/rust/library");
    // Watch the component directory as well so installing a previously absent
    // rust-src can re-run qualification; missing source never blocks std itself.
    println!(
        "cargo:rerun-if-changed={}",
        library.parent().unwrap().display()
    );
    for (relative, expected) in SOURCES {
        let path = library.join(relative);
        if path.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
        let bytes = fs::read(path).map_err(|_| "audited-library-source-unavailable")?;
        let digest = format!("{:x}", Sha256::digest(bytes));
        if digest != *expected {
            return Err("audited-library-source-mismatch");
        }
    }
    Ok(())
}
fn main() {
    println!("cargo:rustc-check-cfg=cfg(conduit_pinned_collection_layout)");
    println!("cargo:rerun-if-changed=build.rs");
    for name in [
        "RUSTC",
        "TARGET",
        "CARGO_CFG_TARGET_ARCH",
        "CARGO_CFG_TARGET_POINTER_WIDTH",
        "CARGO_CFG_TARGET_ENDIAN",
        "CARGO_CFG_TARGET_OS",
        "CARGO_CFG_TARGET_ENV",
        "CARGO_CFG_TARGET_ABI",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTFLAGS",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTC_BOOTSTRAP",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    match qualify() {
        Ok(()) => {
            println!("cargo:rustc-cfg=conduit_pinned_collection_layout");
            println!("cargo:rustc-env=CONDUIT_STD_COLLECTION_LAYOUT_QUALIFICATION=rust-1.98.1-{REVISION}-linux64-audited-sources");
        }
        Err(reason) => println!(
            "cargo:rustc-env=CONDUIT_STD_COLLECTION_LAYOUT_QUALIFICATION=unqualified/{reason}"
        ),
    }
}
