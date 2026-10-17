//! Exact installed Todo checkpoint residence, selected before a fresh Boot.
//! A later generation transition must explicitly persist its new version;
//! this selection never infers it from directory contents.
use crate::cli::InstalledTodoCheckpointOptions;
use conduit_core::{
    kind_id, semantic_digest, ResourceAccessMode, ResourceContentRequirement, ResourceRetention,
    ResourceSemanticIdentity, ResourceSharing, ResourceVersionIdentity,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    root: PathBuf,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    version: [u8; 32],
}

pub(super) enum Change {
    Preserve,
    Replace(Selection),
    Remove,
}

impl Selection {
    pub(super) fn select(root: &Path, version: Option<&str>) -> Result<Self, String> {
        let root = checked_root(root)?;
        let version = match version {
            Some(hex) => decode_digest(hex)?,
            None => semantic_digest(
                "conduit.todo/installed-checkpoint-generation@1",
                super::fresh_identity("todo-checkpoint", &root.display().to_string()).as_bytes(),
            ),
        };
        if version == [0; 32] {
            return Err("selected Todo checkpoint version must be nonzero".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = root
                .metadata()
                .map_err(|error| format!("Todo checkpoint root: {error}"))?;
            Ok(Self {
                root,
                device: metadata.dev(),
                inode: metadata.ino(),
                version,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self { root, version })
        }
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if self.version == [0; 32]
            || self.root.as_os_str().is_empty()
            || self.root.as_os_str().len() > 4096
        {
            return Err("installed Todo checkpoint selection is invalid".into());
        }
        let current = checked_root(&self.root)?;
        if current != self.root {
            return Err("installed Todo checkpoint root changed; reselect it".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = current
                .metadata()
                .map_err(|error| format!("Todo checkpoint root: {error}"))?;
            if metadata.dev() != self.device || metadata.ino() != self.inode {
                return Err("installed Todo checkpoint root identity changed; reselect it".into());
            }
        }
        Ok(())
    }

    pub(super) fn validate_for_reselection(&self) -> Result<(), String> {
        if self.version == [0; 32]
            || self.root.as_os_str().is_empty()
            || self.root.as_os_str().len() > 4096
            || !self.root.is_absolute()
        {
            return Err("installed Todo checkpoint selection is invalid".into());
        }
        Ok(())
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn version_hex(&self) -> String {
        self.version
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// Installation-owned successor choice. This is selected before the next
    /// Plan and persisted with its Body transition, never inferred from files.
    pub(super) fn next_generation(&self) -> Result<Self, String> {
        self.validate()?;
        let seed = super::fresh_identity("todo-checkpoint-next", &self.version_hex());
        let version = semantic_digest("conduit.todo/installed-next-generation@1", seed.as_bytes());
        let hex: String = version.iter().map(|byte| format!("{byte:02x}")).collect();
        let selected = Self::select(&self.root, Some(&hex))?;
        if selected.version == self.version {
            return Err("Todo successor repeated the selected generation".into());
        }
        Ok(selected)
    }

    pub(super) fn content(&self) -> ResourceContentRequirement {
        ResourceContentRequirement {
            identity: ResourceSemanticIdentity::from_digest(semantic_digest(
                "conduit.todo/checkpoint-semantic@2",
                b"conduit.todo/checkpoint-envelope@1",
            )),
            version: ResourceVersionIdentity::from_digest(self.version),
            content_profile: kind_id("conduit.todo/checkpoint-envelope@1"),
            maximum_bytes: conduit_std_offers::TODO_CHECKPOINT_MAX_BYTES,
            maximum_items: 1,
            retention: ResourceRetention::ExternalDurable,
            sharing: ResourceSharing::SingleWriterPublished,
            access: ResourceAccessMode::WriteCandidatePublish,
            generation_slots: 1,
            reader_leases: 1,
            publication_slots: 1,
            sensitive: false,
        }
    }
}

pub(super) fn change(options: InstalledTodoCheckpointOptions) -> Result<Change, String> {
    if options.without_selected_todo_checkpoint {
        return Ok(Change::Remove);
    }
    match options.selected_todo_checkpoint_root {
        Some(root) => Ok(Change::Replace(Selection::select(
            &root,
            options.selected_todo_checkpoint_version.as_deref(),
        )?)),
        None => Ok(Change::Preserve),
    }
}

fn checked_root(root: &Path) -> Result<PathBuf, String> {
    let metadata = std::fs::symlink_metadata(root)
        .map_err(|error| format!("Todo checkpoint root: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Todo checkpoint root must be an existing non-symlink directory".into());
    }
    root.canonicalize()
        .map_err(|error| format!("Todo checkpoint root: {error}"))
}

fn decode_digest(input: &str) -> Result<[u8; 32], String> {
    if input.len() != 64 || !input.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Todo checkpoint version must be exactly 64 hexadecimal digits".into());
    }
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&input[index * 2..index * 2 + 2], 16)
            .map_err(|_| "Todo checkpoint version contains invalid hexadecimal digits")?;
    }
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        let root = std::env::temp_dir()
            .join(super::super::fresh_identity("todo-selection", "test").replace('/', "-"));
        std::fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn selection_is_exact_and_root_rebinding_refuses() {
        let root = root();
        let first = Selection::select(&root, None).unwrap();
        assert_eq!(
            first.content().content_profile,
            kind_id("conduit.todo/checkpoint-envelope@1")
        );
        assert_ne!(first.content().version.digest(), [0; 32]);
        assert_eq!(
            first.content().identity,
            Selection::select(&root, None).unwrap().content().identity
        );
        assert!(first.validate().is_ok());
        let reused = Selection::select(&root, Some(&first.version_hex())).unwrap();
        assert_eq!(first.version, reused.version);
        let moved = root.with_extension("moved");
        std::fs::rename(&root, &moved).unwrap();
        std::fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        assert!(first.validate().unwrap_err().contains("identity changed"));
        std::fs::remove_dir(root).unwrap();
        std::fs::remove_dir(moved).unwrap();
    }

    #[test]
    fn selection_requires_real_directory_and_exact_nonzero_version() {
        let root = root();
        assert!(Selection::select(&root.join("missing"), None).is_err());
        assert!(Selection::select(&root, Some("00")).is_err());
        assert!(Selection::select(&root, Some(&"00".repeat(32))).is_err());
        #[cfg(unix)]
        {
            let link = root.with_extension("link");
            std::os::unix::fs::symlink(&root, &link).unwrap();
            assert!(Selection::select(&link, None).is_err());
            std::fs::remove_file(link).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }
}
