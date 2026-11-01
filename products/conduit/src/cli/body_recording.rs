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
        Operation::RecoverCatalog => RecordingOperation::RecoverCatalog,
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
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectedAuthority {
    grant_id: conduit_core::AuthorityGrantId,
    contract_id: conduit_core::AuthorityContractId,
    host_call_contract_id: conduit_core::HostCallContractId,
    subject_kind: conduit_core::KindId,
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
    capability_id: conduit_core::CapabilityId,
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
    let selected: SelectedAuthority =
        serde_json::from_slice(&bytes).map_err(|e| format!("selected recording authority: {e}"))?;
    Ok(conduit_core::AuthorityGrant {
        grant_id: selected.grant_id,
        contract_id: selected.contract_id,
        host_call_contract_id: selected.host_call_contract_id,
        subject_kind: selected.subject_kind,
        host_id: selected.host_id,
        boot_id: selected.boot_id,
        capability_id: selected.capability_id,
    })
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
