//! Explicit operator selection sent over the existing authenticated owner control.
use crate::durable_host_control::body_recording::{RecordingOperation, RecordingRequest};
use clap::{Args, Subcommand};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
#[derive(Debug, Args)]
pub(crate) struct BodyRecordingArgs {
    #[arg(long)]
    state_dir: PathBuf,
    #[arg(long)]
    expected_body_id: String,
    #[arg(long)]
    expected_boot_id: String,
    #[arg(long,default_value_t=30_000,value_parser=clap::value_parser!(u64).range(1..=900_000))]
    maximum_millis: u64,
    /// Explicit preparation ceiling, bounded again by the selected Host's configuration.
    #[arg(long,value_parser=clap::value_parser!(u64).range(1..))]
    preparation_limit_bytes: u64,
    #[command(subcommand)]
    operation: Operation,
}
#[derive(Debug, Subcommand)]
enum Operation {
    Publish {
        #[arg(long)]
        authority_file: PathBuf,
    },
    Readback {
        #[arg(long)]
        authority_file: PathBuf,
    },
    Catalog {
        #[arg(long)]
        authority_file: PathBuf,
    },
    RecoverCatalog,
    /// Prepare the next chunk without executing a Play or retiring observations.
    NextChunk {
        #[arg(long,value_parser=clap::value_parser!(u8).range(0..16))]
        expected_index: u8,
        #[arg(long,value_parser=parse_sha256)]
        expected_canonical_sha256: [u8; 32],
    },
    ReadRecoveredCatalog {
        #[arg(long)]
        authority_file: PathBuf,
    },
    ReadRecoveredChunk {
        #[arg(long)]
        authority_file: PathBuf,
        #[arg(long,value_parser=clap::value_parser!(u8).range(0..16))]
        index: u8,
    },
}
pub(crate) fn run(args: BodyRecordingArgs) -> Result<(), String> {
    let operation = match args.operation {
        Operation::Publish { authority_file } => RecordingOperation::Publish {
            authority: read_authority(&authority_file)?,
        },
        Operation::Readback { authority_file } => RecordingOperation::Readback {
            authority: read_authority(&authority_file)?,
        },
        Operation::Catalog { authority_file } => RecordingOperation::Catalog {
            authority: read_authority(&authority_file)?,
        },
        Operation::RecoverCatalog => RecordingOperation::RecoverCatalog {},
        Operation::NextChunk {
            expected_index,
            expected_canonical_sha256,
        } => RecordingOperation::NextChunk {
            expected_index,
            expected_canonical_sha256,
        },
        Operation::ReadRecoveredCatalog { authority_file } => {
            RecordingOperation::ReadRecoveredCatalog {
                authority: read_authority(&authority_file)?,
            }
        }
        Operation::ReadRecoveredChunk {
            authority_file,
            index,
        } => RecordingOperation::ReadRecoveredChunk {
            authority: read_authority(&authority_file)?,
            index: usize::from(index),
        },
    };
    crate::durable_host_control::body_recording::recording(
        &args.state_dir,
        RecordingRequest {
            operation,
            maximum_millis: args.maximum_millis,
            preparation_limit_bytes: args.preparation_limit_bytes,
            expected_body_id: args.expected_body_id,
            expected_boot_id: args.expected_boot_id,
        },
    )
}
fn parse_sha256(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("expected canonical SHA256 must contain exactly 64 hexadecimal digits".into());
    }
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| "invalid canonical SHA256".to_string())?;
    }
    Ok(digest)
}
fn read_authority(path: &Path) -> Result<conduit_core::AuthorityGrant, String> {
    const LIMIT: u64 = 16 * 1024;
    let file =
        std::fs::File::open(path).map_err(|e| format!("open selected recording authority: {e}"))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > LIMIT {
        return Err("recording authority must be a regular JSON file of at most 16KiB".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() as u64 > LIMIT {
        return Err("recording authority exceeds its finite file bound".into());
    }
    crate::durable_host_control::body_recording::decode_selected_authority(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[test]
    fn recording_cli_requires_explicit_authority_only_for_snapshot_effects() {
        let base = [
            "conduit",
            "body",
            "recording",
            "--state-dir",
            "/tmp/fixture",
            "--expected-body-id",
            "body/test",
            "--expected-boot-id",
            "boot/test",
            "--preparation-limit-bytes",
            "8192",
        ];
        let parse = |tail: &[&str]| {
            crate::cli::Cli::try_parse_from(base.iter().copied().chain(tail.iter().copied()))
        };
        assert!(parse(&["recover-catalog"]).is_ok());
        assert!(parse(&["publish"]).is_err());
        assert!(parse(&["publish", "--authority-file", "/tmp/selected.json"]).is_ok());
        assert!(parse(&["recover-catalog", "--authority-file", "/tmp/selected.json"]).is_err());
        assert!(parse(&[
            "read-recovered-chunk",
            "--authority-file",
            "/tmp/selected.json",
            "--index",
            "16"
        ])
        .is_err());
    }
    #[test]
    fn next_chunk_requires_exact_compare_fields_and_no_authority_file() {
        let digest = "01".repeat(32);
        assert_eq!(parse_sha256(&digest).unwrap(), [1; 32]);
        for invalid in ["0".repeat(63), "x".repeat(64), "0".repeat(65)] {
            assert!(parse_sha256(&invalid).is_err());
        }
        let base = [
            "conduit",
            "body",
            "recording",
            "--state-dir",
            "/tmp/fixture",
            "--expected-body-id",
            "body/test",
            "--expected-boot-id",
            "boot/test",
            "--preparation-limit-bytes",
            "8192",
            "next-chunk",
        ];
        let parse = |tail: &[&str]| {
            crate::cli::Cli::try_parse_from(base.iter().copied().chain(tail.iter().copied()))
        };
        assert!(parse(&[
            "--expected-index",
            "0",
            "--expected-canonical-sha256",
            &digest
        ])
        .is_ok());
        assert!(parse(&[
            "--expected-index",
            "16",
            "--expected-canonical-sha256",
            &digest
        ])
        .is_err());
        assert!(parse(&["--expected-index", "0"]).is_err());
        assert!(parse(&[
            "--expected-index",
            "0",
            "--expected-canonical-sha256",
            &digest,
            "--authority-file",
            "/tmp/forbidden"
        ])
        .is_err());
    }
    #[test]
    fn selected_recording_authority_refuses_unknown_duplicate_and_oversized_input() {
        let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(
            "recording-cli",
            "authority",
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("selected.json");
        let json = r#"{"grant_id":"g","contract_id":"c","host_call_contract_id":"call","subject_kind":"kind","host_id":"host","boot_id":"boot","capability_id":"cap"}"#;
        std::fs::write(&path, json).unwrap();
        assert_eq!(read_authority(&path).unwrap().grant_id.as_str(), "g");
        std::fs::write(
            &path,
            json.replace("\"grant_id\":\"g\"", "\"grant_id\":\"g\",\"unknown\":true"),
        )
        .unwrap();
        assert!(read_authority(&path).is_err());
        std::fs::write(
            &path,
            json.replace(
                "\"grant_id\":\"g\"",
                "\"grant_id\":\"g\",\"grant_id\":\"other\"",
            ),
        )
        .unwrap();
        assert!(read_authority(&path).is_err());
        std::fs::write(&path, vec![b' '; 16 * 1024 + 1]).unwrap();
        assert!(read_authority(&path).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
