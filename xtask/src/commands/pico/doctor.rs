use anyhow::{bail, Result};
use std::path::Path;
use std::process::Command;

const REQUIRED_TOOLS: &[(&str, &str)] = &[
    ("cargo", "Install Rust via https://rustup.rs"),
    ("rustc", "Install Rust via https://rustup.rs"),
    ("rustup", "Install Rust via https://rustup.rs"),
    ("elf2uf2-rs", "cargo install elf2uf2-rs --locked"),
    ("lsblk", "Required on Linux to discover BOOTSEL volumes"),
    ("sync", "Required to flush UF2 writes"),
];

const OPTIONAL_TOOLS: &[(&str, &str)] = &[
    ("udisksctl", "Optional: auto-mount BOOTSEL volume without sudo"),
    ("sha256sum", "Optional: system sha256sum (xtask has an internal fallback)"),
];

const REQUIRED_TARGET: &str = "thumbv6m-none-eabi";

/// SHA-256 digests of the vendored CYW43 assets at the pinned Embassy commit.
pub const CYW43_ASSETS: &[(&str, &str)] = &[
    (
        "43439A0.bin",
        "5555e0261da2610a500d68c18d895cace0152bbefbf76f4aa683ebce77e3d7eb",
    ),
    (
        "43439A0_clm.bin",
        "e712b3d218e8b1e2747b092e03b8b0afcb8c8c8e355d2a4a0d47b493800f3f89",
    ),
    (
        "nvram_rp2040.bin",
        "4904bdbb0c937bd0ac2eb2a1d62f2da4dd90e32082384e02874e8d671b0f330d",
    ),
    (
        "LICENSE-permissive-binary-license-1.0.txt",
        "5f65b8a496ac27afda41917c18cb6e690b4a022df1f5a12ea823eb38a287f50e",
    ),
];

pub const CYW43_COMMIT: &str = "6a823b96b3d270b6da1cc667f8acea749e588dab";
pub const CYW43_ASSET_DIR: &str =
    "firmware/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab";

pub fn run_doctor(dry_run: bool) -> Result<()> {
    println!("==> pico doctor: checking prerequisites");

    let mut missing = Vec::new();

    for (tool, repair) in REQUIRED_TOOLS {
        let found = which_tool(tool);
        let mark = if found { "✓" } else { "✗" };
        println!("  {} {}", mark, tool);
        if !found {
            println!("      fix: {}", repair);
            missing.push(*tool);
        }
    }

    for (tool, note) in OPTIONAL_TOOLS {
        let found = which_tool(tool);
        let mark = if found { "✓" } else { "-" };
        println!("  {} {} (optional) — {}", mark, tool, note);
    }

    // Check RP2040 target is installed
    let target_ok = check_rustup_target(REQUIRED_TARGET);
    println!(
        "  {} rustup target {}",
        if target_ok { "✓" } else { "✗" },
        REQUIRED_TARGET
    );
    if !target_ok {
        println!(
            "      fix: rustup target add {}",
            REQUIRED_TARGET
        );
        missing.push(REQUIRED_TARGET);
    }

    // Verify vendored CYW43 assets
    let repo_root = repo_root();
    let asset_dir = repo_root.join(CYW43_ASSET_DIR);
    println!("==> pico doctor: verifying vendored CYW43 assets in {}", asset_dir.display());
    let mut asset_ok = true;
    for (filename, expected_hex) in CYW43_ASSETS {
        let path = asset_dir.join(filename);
        match verify_sha256(&path, expected_hex) {
            Ok(()) => println!("  ✓ {}", filename),
            Err(e) => {
                println!("  ✗ {} — {}", filename, e);
                asset_ok = false;
            }
        }
    }
    if !asset_ok {
        println!("  fix: cargo run -p xtask -- pico --refresh-radio-assets");
    }

    if dry_run {
        println!("(dry-run: skipping prerequisite failure exit)");
        return Ok(());
    }

    if !missing.is_empty() {
        bail!("Missing required tools: {}. See fix instructions above.", missing.join(", "));
    }
    if !asset_ok {
        bail!("CYW43 asset verification failed. Run with --refresh-radio-assets to re-download.");
    }

    println!("==> pico doctor: all checks passed");
    Ok(())
}

fn which_tool(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn check_rustup_target(target: &str) -> bool {
    Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.trim() == target)
        })
        .unwrap_or(false)
}

pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {}", path.display(), e))?;
    let digest = Sha256::digest(&bytes);
    let actual = hex::encode(digest);
    if actual != expected_hex {
        bail!(
            "SHA-256 mismatch for {}:\n  expected: {}\n  actual:   {}",
            path.display(),
            expected_hex,
            actual
        );
    }
    Ok(())
}

pub fn repo_root() -> std::path::PathBuf {
    // xtask is always run from the workspace root via `cargo run -p xtask`
    std::env::var("CARGO_MANIFEST_DIR")
        .map(|d| {
            // CARGO_MANIFEST_DIR is xtask/ — go up one level
            Path::new(&d).parent().unwrap().to_path_buf()
        })
        .unwrap_or_else(|_| std::env::current_dir().expect("cwd"))
}
