//! One QMP Todo action on a selected checkpoint fork, with original-state guard.

use std::{
    fs,
    os::unix::{fs::MetadataExt, net::UnixStream},
    path::{Path, PathBuf},
    process::Child,
    time::Duration,
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{
    ConduitosError, LiveOwnerTodoActionProofArgs, journey_input, qmp, qmp_display, refusal,
    todo_validation, wait_for_action, wait_for_resume, write_checkpoint,
};

pub(super) struct Isolation {
    selected: PathBuf,
    selected_device: u64,
    selected_inode: u64,
    selected_before: Inventory,
    protected: PathBuf,
    protected_device: u64,
    protected_inode: u64,
    protected_before: Inventory,
}

#[derive(Debug, PartialEq, Eq)]
struct Inventory {
    files: usize,
    sha256: String,
}

pub(super) fn validate_isolated_installation(
    args: &LiveOwnerTodoActionProofArgs,
    source: &str,
) -> Result<Isolation, ConduitosError> {
    let selected = fs::canonicalize(&args.isolated_checkpoint_root)
        .map_err(|_| refusal("native-todo-selected-checkpoint-absent"))?;
    let protected = fs::canonicalize(&args.protected_checkpoint_root)
        .map_err(|_| refusal("native-todo-protected-checkpoint-absent"))?;
    if selected == protected || selected.starts_with(&protected) || protected.starts_with(&selected)
    {
        return Err(refusal("native-todo-checkpoint-not-isolated"));
    }
    let selected_metadata = fs::symlink_metadata(&selected)
        .map_err(|_| refusal("native-todo-selected-checkpoint-absent"))?;
    let protected_metadata = fs::symlink_metadata(&protected)
        .map_err(|_| refusal("native-todo-protected-checkpoint-absent"))?;
    if !selected_metadata.is_dir()
        || !protected_metadata.is_dir()
        || (selected_metadata.dev(), selected_metadata.ino())
            == (protected_metadata.dev(), protected_metadata.ino())
    {
        return Err(refusal("native-todo-checkpoint-not-isolated"));
    }
    let state = fs::canonicalize(&args.face.owner_state_dir)
        .map_err(|_| refusal("native-todo-owner-state-absent"))?;
    let bin = fs::canonicalize(&args.face.owner_conduit_bin)
        .map_err(|_| refusal("native-todo-owner-bin-absent"))?;
    let installation: Value = serde_json::from_slice(
        &fs::read(state.join("installation.json"))
            .map_err(|_| refusal("native-todo-installation-absent"))?,
    )
    .map_err(|_| refusal("native-todo-installation-invalid"))?;
    let selected_version = installation["selected_todo_checkpoint"]["version"]
        .as_array()
        .ok_or_else(|| refusal("native-todo-selection-version-invalid"))?;
    let selected_hex = selected_version
        .iter()
        .map(|byte| {
            byte.as_u64()
                .filter(|value| *value <= 255)
                .map(|value| format!("{value:02x}"))
        })
        .collect::<Option<Vec<String>>>()
        .ok_or_else(|| refusal("native-todo-selection-version-invalid"))?
        .join("");
    if args.selected_checkpoint_version_hex.len() != 64
        || !args
            .selected_checkpoint_version_hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || selected_hex != args.selected_checkpoint_version_hex.to_ascii_lowercase()
        || installation["release_source_identity"] != source
        || installation["product_executable"] != bin.to_string_lossy().as_ref()
        || installation["selected_todo_checkpoint"]["root"] != selected.to_string_lossy().as_ref()
        || installation["selected_todo_checkpoint"]["device"] != selected_metadata.dev()
        || installation["selected_todo_checkpoint"]["inode"] != selected_metadata.ino()
    {
        return Err(refusal("native-todo-installation-selection-mismatch"));
    }
    let selected_before = inventory(&selected)?;
    let protected_before = inventory(&protected)?;
    if selected_before.files == 0 || selected_before != protected_before {
        return Err(refusal("native-todo-isolated-checkpoint-copy-differs"));
    }
    Ok(Isolation {
        selected_before,
        protected_before,
        selected,
        selected_device: selected_metadata.dev(),
        selected_inode: selected_metadata.ino(),
        protected,
        protected_device: protected_metadata.dev(),
        protected_inode: protected_metadata.ino(),
    })
}

fn inventory(root: &Path) -> Result<Inventory, ConduitosError> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(root).map_err(|_| refusal("native-todo-checkpoint-read-refused"))? {
        let entry = entry.map_err(|_| refusal("native-todo-checkpoint-read-refused"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| refusal("native-todo-checkpoint-read-refused"))?;
        if !metadata.file_type().is_file() || metadata.len() > 1024 * 1024 {
            return Err(refusal("native-todo-checkpoint-entry-invalid"));
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| refusal("native-todo-checkpoint-name-invalid"))?;
        if name.is_empty() || name.len() > 256 {
            return Err(refusal("native-todo-checkpoint-name-invalid"));
        }
        let bytes =
            fs::read(entry.path()).map_err(|_| refusal("native-todo-checkpoint-read-refused"))?;
        entries.push((name, format!("{:x}", Sha256::digest(&bytes))));
        if entries.len() > 256 {
            return Err(refusal("native-todo-checkpoint-inventory-bound"));
        }
    }
    entries.sort();
    let mut hasher = Sha256::new();
    for (name, digest) in &entries {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(digest.as_bytes());
        hasher.update([0]);
    }
    Ok(Inventory {
        files: entries.len(),
        sha256: format!("sha256:{:x}", hasher.finalize()),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn continue_after_arrival(
    args: &LiveOwnerTodoActionProofArgs,
    isolation: &Isolation,
    directory: &Path,
    serial_path: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    route: &super::owner_boot::PreparedOwnerBoot,
    qemu_args: &[String],
    standby_part: &Value,
    standby_face: &Value,
    part: &Value,
    before: &Value,
    before_ack: &Value,
    standby_image: &Value,
    before_image: &Value,
) -> Result<Value, ConduitosError> {
    if before["face_revision"].as_u64().unwrap_or(0)
        <= standby_face["face_revision"].as_u64().unwrap_or(u64::MAX)
        || before["face_id"] == standby_face["face_id"]
    {
        return Err(refusal("native-todo-action-face-was-not-refreshed"));
    }
    let owner_before = todo_validation::read_and_validate_current(
        &args.face,
        before,
        &args.face.expected_status,
        Some(&args.expected_action_id),
    )?;
    write_checkpoint(
        directory,
        "native-arrived.json",
        &json!({
            "schema":"conduit.conduitos/native-owner-coordination@1",
            "stage":"todo-action-ready", "guest_part":part,
            "face":before, "show_ack":before_ack, "owner_face":owner_before,
            "selected_checkpoint_root":isolation.selected,
            "protected_checkpoint_root":isolation.protected,
        }),
    )?;
    wait_for_resume(
        directory,
        "resume-native-action",
        child,
        Duration::from_secs(45),
    )?;
    for _ in 0..args.tab_count {
        journey_input::key_pair(qmp, reader, "tab", "native-owner-todo-focus")?;
    }
    journey_input::key_pair(qmp, reader, "ret", "native-owner-todo-submit")?;
    let (action, after, after_ack) = wait_for_action(serial_path, child, Duration::from_secs(30))?;
    if action["action_id"] != args.expected_action_id
        || action["prior_show_id"] != before["show_id"]
        || action["face_id"] != before["face_id"]
        || action["face_revision"] != before["face_revision"]
        || before_ack["show_id"] != before["show_id"]
        || after_ack["show_id"] != after["show_id"]
        || after["show_id"] == before["show_id"]
        || after["face_id"] == before["face_id"]
    {
        return Err(refusal("native-todo-action-receipt-mismatch"));
    }
    let owner_after = todo_validation::read_and_validate_current(
        &args.face,
        &after,
        &args.expected_after_status,
        None,
    )?;
    let (after_image, health) = qmp_display::capture(qmp, reader, directory, "owner-after")?;
    if let Some(error) = health {
        return Err(error);
    }
    let protected_after = inventory(&isolation.protected)?;
    let selected_after = inventory(&isolation.selected)?;
    let protected_metadata = fs::symlink_metadata(&isolation.protected)
        .map_err(|_| refusal("native-todo-protected-checkpoint-absent"))?;
    let selected_metadata = fs::symlink_metadata(&isolation.selected)
        .map_err(|_| refusal("native-todo-selected-checkpoint-absent"))?;
    if protected_after != isolation.protected_before
        || (protected_metadata.dev(), protected_metadata.ino())
            != (isolation.protected_device, isolation.protected_inode)
        || selected_after == isolation.selected_before
        || (selected_metadata.dev(), selected_metadata.ino())
            != (isolation.selected_device, isolation.selected_inode)
    {
        return Err(refusal("native-todo-checkpoint-fork-proof-failed"));
    }
    if child
        .try_wait()
        .map_err(|_| refusal("native-todo-guest-status-unknown"))?
        .is_some()
    {
        return Err(refusal("native-todo-guest-not-live-at-capture"));
    }
    write_checkpoint(
        directory,
        "native-action.json",
        &json!({
            "schema":"conduit.conduitos/native-owner-coordination@1",
            "stage":"todo-action-accepted", "action":action,
            "face":after, "show_ack":after_ack, "owner_face":owner_after,
        }),
    )?;
    Ok(json!({
        "schema":"conduit.conduitos/native-todo-action-proof@1",
        "proof_class":"live-local-qmp-installed-owner-isolated-checkpoint-fork",
        "source_commit":route.source_identity,
        "spore_build_id":route.build_id,
        "spore_sha256":route.artifact_sha256,
        "candidate_id":route.candidate_id,
        "reachability":route.reachability,
        "qemu_argv":qemu_args,
        "guest_part":part,
        "guest_standby_part":standby_part,
        "face_standby":standby_face,
        "face_before":before,
        "show_ack_before":before_ack,
        "owner_face_before":owner_before,
        "action":action,
        "face_after":after,
        "show_ack_after":after_ack,
        "owner_face_after":owner_after,
        "selected_checkpoint_root":isolation.selected,
        "selected_checkpoint_dev_inode":[isolation.selected_device,isolation.selected_inode],
        "selected_checkpoint_before":{"files":isolation.selected_before.files,"sha256":isolation.selected_before.sha256},
        "selected_checkpoint_after":{"files":selected_after.files,"sha256":selected_after.sha256},
        "protected_checkpoint_root":isolation.protected,
        "protected_checkpoint_dev_inode":[isolation.protected_device,isolation.protected_inode],
        "protected_checkpoint_before":{"files":isolation.protected_before.files,"sha256":isolation.protected_before.sha256},
        "protected_checkpoint_after":{"files":protected_after.files,"sha256":protected_after.sha256},
        "screenshots":[standby_image,before_image,after_image],
        "qemu_alive_at_capture":true,
        "coordinated":true,
        "native_mutations":1,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::fs::symlink, time::{SystemTime, UNIX_EPOCH}};

    struct PrivateFixture(PathBuf);

    impl PrivateFixture {
        fn new() -> Self {
            let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let root = std::env::temp_dir().join(format!("conduit-native-todo-isolation-{}-{nonce}", std::process::id()));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for PrivateFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn checkpoint_inventory_distinguishes_a_changed_fork_and_refuses_symlinks() {
        let fixture = PrivateFixture::new();
        let original = fixture.0.join("original");
        let fork = fixture.0.join("fork");
        fs::create_dir(&original).unwrap();
        fs::create_dir(&fork).unwrap();
        fs::write(original.join("generation.checkpoint"), b"before").unwrap();
        fs::write(fork.join("generation.checkpoint"), b"before").unwrap();
        let prior = inventory(&original).unwrap();
        assert_eq!(prior.files, 1);
        assert_eq!(inventory(&fork).unwrap(), prior);
        fs::write(fork.join("generation.checkpoint"), b"after").unwrap();
        assert_ne!(inventory(&fork).unwrap(), prior);
        assert_eq!(inventory(&original).unwrap(), prior);
        symlink(original.join("generation.checkpoint"), fork.join("alias")).unwrap();
        assert!(inventory(&fork).is_err());
    }
}
