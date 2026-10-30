//! Developer-only framebuffer adapter build; this creates no Host release or admission.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const TARGET: &str = "aarch64-unknown-linux-gnu";
const TOOLCHAIN: &str = "+1.98.1";
const HELP: &str = "make linux-face-display --target linux-aarch64 --output NEW_DIRECTORY --source-identity FULL_GIT_SHA --linker ABSOLUTE_PATH --linker-sha256 SHA256 [--dry-run] [--json] [--quiet] [--locked]";

#[derive(Debug)]
struct Options {
    output: PathBuf,
    source: String,
    linker: PathBuf,
    linker_digest: String,
    dry_run: bool,
    json: bool,
    quiet: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut values = BTreeMap::new();
    let mut flags = BTreeMap::new();
    let mut args = args.iter();
    while let Some(key) = args.next() {
        if ["--dry-run", "--json", "--quiet", "--locked"].contains(&key.as_str()) {
            flags.insert(key.as_str(), true);
        } else if [
            "--target",
            "--output",
            "--source-identity",
            "--linker",
            "--linker-sha256",
        ]
        .contains(&key.as_str())
        {
            let value = args
                .next()
                .filter(|v| !v.starts_with("--"))
                .ok_or_else(|| format!("missing value for {key}; {HELP}"))?;
            if values.insert(key.as_str(), value.clone()).is_some() {
                return Err(format!("duplicate {key}"));
            }
        } else {
            return Err(format!("unknown argument {key}; {HELP}"));
        }
    }
    let mut take = |key| {
        values
            .remove(key)
            .ok_or_else(|| format!("required {key}; {HELP}"))
    };
    if take("--target")? != "linux-aarch64" {
        return Err("only the explicit linux-aarch64 developer target is supported".into());
    }
    let output = PathBuf::from(take("--output")?);
    let source = take("--source-identity")?;
    let linker = PathBuf::from(take("--linker")?);
    let linker_digest = take("--linker-sha256")?;
    if !hex(&source, 40)
        || !hex(&linker_digest, 64)
        || !linker.is_absolute()
        || output.as_os_str().is_empty()
    {
        return Err("require full lowercase source SHA, SHA256, absolute linker and explicit output directory".into());
    }
    Ok(Options {
        output,
        source,
        linker,
        linker_digest,
        dry_run: flags.contains_key("--dry-run"),
        json: flags.contains_key("--json"),
        quiet: flags.contains_key("--quiet"),
    })
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn digest(path: &Path, maximum: u64) -> Result<String, String> {
    use std::io::Read;
    if !fs::metadata(path).map_err(|e| e.to_string())?.is_file() {
        return Err(format!("{} must be a regular file", path.display()));
    }
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > maximum {
        return Err(format!("{} exceeds {maximum} bytes", path.display()));
    }
    let mut hash = Sha256::new();
    let mut reader = file.take(maximum + 1);
    let mut buffer = [0; 16384];
    let mut total = 0;
    loop {
        let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > maximum {
            return Err("file grew beyond admitted bound".into());
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

// Spool trusted developer probes instead of risking an unbounded output pipe.
// This limits captured output and waits; it is not hostile-process confinement.
fn probe(command: &mut Command, scratch: &Path) -> Result<String, String> {
    use std::io::Read;
    let path = scratch.join(format!("probe-{}", std::process::id()));
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        let mut child = command
            .stdout(Stdio::from(file.try_clone().map_err(|e| e.to_string())?))
            .stderr(Stdio::from(file))
            .spawn()
            .map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if Instant::now() >= deadline
                || fs::metadata(&path).map_err(|e| e.to_string())?.len() > 65536
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err("developer probe exceeded ten seconds or 64 KiB capture".into());
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let mut output = String::new();
        fs::File::open(&path)
            .map_err(|e| e.to_string())?
            .take(65537)
            .read_to_string(&mut output)
            .map_err(|e| e.to_string())?;
        if output.len() > 65536 || !status.success() {
            return Err(format!("developer probe failed: {status}"));
        }
        Ok(output.trim().to_owned())
    })();
    let _ = fs::remove_file(path);
    result
}

fn check_source(root: &Path, scratch: &Path, expected: &str) -> Result<(), String> {
    let head = probe(
        Command::new("git")
            .current_dir(root)
            .args(["rev-parse", "HEAD"]),
        scratch,
    )?;
    if head != expected {
        return Err(format!("source identity mismatch: HEAD is {head}"));
    }
    let status = probe(
        Command::new("git").current_dir(root).args([
            "status",
            "--porcelain",
            "--untracked-files=normal",
        ]),
        scratch,
    )?;
    if !status.is_empty() {
        return Err("qualification requires a clean source checkout".into());
    }
    Ok(())
}

fn qualify_elf(bytes: &[u8]) -> Result<(), String> {
    let refuse = || {
        "expected bounded ELF64 little-endian AArch64 Linux executable with canonical interpreter"
            .to_string()
    };
    if bytes.len() < 64
        || &bytes[..7] != b"\x7fELF\x02\x01\x01"
        || ![2, 3].contains(&u16::from_le_bytes([bytes[16], bytes[17]]))
        || u16::from_le_bytes([bytes[18], bytes[19]]) != 183
    {
        return Err(refuse());
    }
    let offset = usize::try_from(u64::from_le_bytes(bytes[32..40].try_into().unwrap()))
        .map_err(|_| refuse())?;
    let size = u16::from_le_bytes(bytes[54..56].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(bytes[56..58].try_into().unwrap()) as usize;
    if size != 56 || count == 0 || count > 128 {
        return Err(refuse());
    }
    let end = offset
        .checked_add(size * count)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(refuse)?;
    let mut interpreters = 0;
    for header in bytes[offset..end].chunks_exact(size) {
        if u32::from_le_bytes(header[..4].try_into().unwrap()) == 3 {
            interpreters += 1;
            let start = usize::try_from(u64::from_le_bytes(header[8..16].try_into().unwrap()))
                .map_err(|_| refuse())?;
            let length = usize::try_from(u64::from_le_bytes(header[32..40].try_into().unwrap()))
                .map_err(|_| refuse())?;
            let end = start
                .checked_add(length)
                .filter(|end| *end <= bytes.len())
                .ok_or_else(refuse)?;
            if &bytes[start..end] != b"/lib/ld-linux-aarch64.so.1\0" {
                return Err(refuse());
            }
        }
    }
    if interpreters != 1 {
        return Err(refuse());
    }
    Ok(())
}

pub fn run(arguments: &[String]) -> Result<(), String> {
    if arguments == ["--help"] {
        println!("{HELP}");
        return Ok(());
    }
    let options = parse(arguments)?;
    let root = crate::workspace::workspace_root()?;
    let scratch = root.join("target/linux-face-display-probes");
    fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    check_source(&root, &scratch, &options.source)?;
    if options.output.exists() {
        return Err("output directory already exists; choose a new explicit path".into());
    }
    if digest(&options.linker, 8 * 1024 * 1024)? != options.linker_digest {
        return Err("linker SHA256 mismatch".into());
    }
    let machine = probe(Command::new(&options.linker).arg("-dumpmachine"), &scratch)?;
    if machine != "aarch64-linux-gnu" {
        return Err(format!("unsupported linker target: {machine}"));
    }
    let compiler = probe(Command::new(&options.linker).arg("--version"), &scratch)?;
    let rust = probe(
        Command::new("rustc").args([TOOLCHAIN, "--version"]),
        &scratch,
    )?;
    if !rust.starts_with("rustc 1.98.1 ") {
        return Err("unexpected Rust toolchain".into());
    }
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/linux-face-display"));
    let target_dir = if target_dir.is_absolute() {
        target_dir
    } else {
        root.join(target_dir)
    };
    let mut report = json!({"schema":"conduit.developer/linux-face-display-build@1", "scope":"development-framebuffer-adapter", "source_identity":options.source,"target":"linux-aarch64", "rust_target":TARGET,"rust_toolchain":rust,"linker":options.linker,"linker_sha256":options.linker_digest,"linker_version":compiler,"output":options.output,"dry_run":options.dry_run,"host_release":false,"hardware_acceptance":false});
    if !options.dry_run {
        let mut build = Command::new("cargo");
        build
            .current_dir(&root)
            .args([
                TOOLCHAIN,
                "rustc",
                "--locked",
                "--manifest-path",
                "targets/linux-face-display/Cargo.toml",
                "--release",
                "--target",
                TARGET,
                "-j2",
                "--",
                "-C",
            ])
            .arg(format!("linker={}", options.linker.display()))
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("CARGO_INCREMENTAL", "0")
            .env(
                "PATH",
                std::env::join_paths(
                    std::iter::once(options.linker.parent().unwrap().to_path_buf()).chain(
                        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
                    ),
                )
                .map_err(|e| e.to_string())?,
            )
            .env(
                "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER",
                &options.linker,
            )
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        for key in [
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
        ] {
            build.env_remove(key);
        }
        let status = build.status().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("adapter build failed: {status}"));
        }
        check_source(&root, &scratch, &options.source)?;
        if digest(&options.linker, 8 * 1024 * 1024)? != options.linker_digest {
            return Err("linker changed during build".into());
        }
        let artifact = target_dir
            .join(TARGET)
            .join("release/conduit-linux-face-display");
        let sha = digest(&artifact, 64 * 1024 * 1024)?;
        use std::io::Read;
        let mut bytes = Vec::new();
        fs::File::open(&artifact)
            .map_err(|e| e.to_string())?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("ELF exceeds 64 MiB".into());
        }
        qualify_elf(&bytes)?;
        if format!("{:x}", Sha256::digest(&bytes)) != sha {
            return Err("artifact changed during qualification".into());
        }
        fs::create_dir(&options.output).map_err(|e| e.to_string())?;
        let destination = options.output.join("conduit-linux-face-display");
        fs::copy(&artifact, &destination).map_err(|e| e.to_string())?;
        if digest(&destination, 64 * 1024 * 1024)? != sha {
            return Err("copied artifact digest mismatch".into());
        }
        report["artifact_sha256"] = json!(sha);
        report["artifact_bytes"] = json!(bytes.len());
        report["qualification"] = json!(
            "source-clean; ELF64 AArch64; canonical Linux interpreter; copied SHA256 verified"
        );
        fs::write(
            options.output.join("qualification.json"),
            serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    if options.json {
        println!("{report}");
    } else if !options.quiet {
        println!(
            "{}: {}",
            if options.dry_run {
                "Validated build inputs (no artifact produced)"
            } else {
                "Qualified developer adapter"
            },
            options.output.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args() -> Vec<String> {
        [
            "--target",
            "linux-aarch64",
            "--output",
            "/tmp/new-output",
            "--source-identity",
            &"a".repeat(40),
            "--linker",
            "/compiler",
            "--linker-sha256",
            &"b".repeat(64),
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }
    #[test]
    fn explicit_target_source_and_toolchain_are_required() {
        assert!(parse(&args()).is_ok());
        let mut wrong = args();
        wrong[1] = "linux-armv6-face".into();
        assert!(parse(&wrong).is_err());
        let mut wrong = args();
        wrong[5] = "dev".into();
        assert!(parse(&wrong).is_err());
        let mut wrong = args();
        wrong[7] = "compiler".into();
        assert!(parse(&wrong).is_err());
        let mut wrong = args();
        wrong.extend(["--target".into(), "linux-aarch64".into()]);
        assert!(parse(&wrong).is_err());
        assert!(parse(&args()[..8]).is_err());
    }
    fn elf() -> Vec<u8> {
        let mut bytes = vec![0; 120];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[16..18].copy_from_slice(&3u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&183u16.to_le_bytes());
        bytes[32..40].copy_from_slice(&64u64.to_le_bytes());
        bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
        bytes[56..58].copy_from_slice(&1u16.to_le_bytes());
        bytes[64..68].copy_from_slice(&3u32.to_le_bytes());
        bytes[72..80].copy_from_slice(&120u64.to_le_bytes());
        bytes[96..104].copy_from_slice(&27u64.to_le_bytes());
        bytes.extend(b"/lib/ld-linux-aarch64.so.1\0");
        bytes
    }
    #[test]
    fn wrong_architecture_and_truncated_or_foreign_interpreter_refuse() {
        assert!(qualify_elf(&elf()).is_ok());
        for index in [4, 5, 18, 54, 56, 120] {
            let mut bytes = elf();
            bytes[index] ^= 1;
            assert!(qualify_elf(&bytes).is_err(), "index {index}");
        }
        assert!(qualify_elf(&elf()[..119]).is_err());
        let mut bytes = elf();
        bytes[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(qualify_elf(&bytes).is_err());
    }
}
