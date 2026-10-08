//! Explicitly selected std residence for bounded Todo checkpoints.
//!
//! The caller owns the selected directory and supplies exact current Plan
//! placements. The installed checkpoint Back invokes this provider only through
//! an admitted Host Call. A successful commit means the immutable candidate
//! and selector have both been synced before the Back can emit committed state.
use conduit_core::{
    kind_id, semantic_digest, AuthorityBinding, PlannedGear, ResourceAccessMode, ResourceRetention,
    ResourceSharing,
};
use conduit_todo_plot::{TodoState, STATE_MAX_BYTES};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"CDTODO01";
const SCHEMA: u8 = 1;
const MAX_ID: usize = 128;
pub const CHECKPOINT_MAX_BYTES: usize =
    8 + 1 + 64 + 3 * (1 + MAX_ID) + 4 + 4 + 32 + STATE_MAX_BYTES;
pub const PUBLISH_OPERATION: &str = "conduit.host/todo-checkpoint-publish@1";
pub const READ_OPERATION: &str = "conduit.host/todo-checkpoint-read@1";
pub const AUTHORITY_CONTRACT: &str = "authority/todo-checkpoint@1";

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    InvalidBinding,
    WrongAuthority,
    WrongAccess,
    InvalidState,
    Missing,
    Corrupt,
    StaleRevision,
    UnknownOutcome,
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointIdentity {
    pub body: String,
    pub plot: String,
    pub workload: String,
}

impl CheckpointIdentity {
    fn valid(&self) -> bool {
        [&self.body, &self.plot, &self.workload]
            .iter()
            .all(|s| !s.is_empty() && s.len() <= MAX_ID && !s.chars().any(char::is_control))
    }
}

pub struct SelectedTodoResidence {
    root: PathBuf,
    authority: AuthorityBinding,
    access: ResourceAccessMode,
    identity: CheckpointIdentity,
    namespace: String,
    semantic: [u8; 32],
    generation: [u8; 32],
}

impl SelectedTodoResidence {
    /// The directory is an explicit selected residence supplied by the Host.
    /// This preparation never creates or discovers a storage root.
    pub fn prepare(
        root: &Path,
        placement: &PlannedGear,
        identity: CheckpointIdentity,
    ) -> Result<Self, Refusal> {
        if !identity.valid()
            || fs::symlink_metadata(root)
                .map_err(|_| Refusal::Storage)?
                .file_type()
                .is_symlink()
        {
            return Err(Refusal::InvalidBinding);
        }
        let root = root.canonicalize().map_err(|_| Refusal::Storage)?;
        if !root.is_dir() {
            return Err(Refusal::InvalidBinding);
        }
        let ([resource], [call], [authority]) = (
            placement.resources.as_slice(),
            placement.host_calls.as_slice(),
            placement.authority.as_slice(),
        ) else {
            return Err(Refusal::InvalidBinding);
        };
        let content = resource.content.as_ref().ok_or(Refusal::InvalidBinding)?;
        content.validate().map_err(|_| Refusal::InvalidBinding)?;
        let contract = &content.contract;
        let operation = match contract.access {
            ResourceAccessMode::ReadPublished => READ_OPERATION,
            ResourceAccessMode::WriteCandidatePublish => PUBLISH_OPERATION,
        };
        if content.owner_host != placement.host_id
            || content.owner_boot != placement.boot_id
            || content.base_id.as_str() != "std/explicit-shared-checkpoint"
            || content.residence_profile != kind_id("std/explicit-shared-checkpoint@1")
            || resource.class_id.as_str() != "resource/todo-checkpoint@1"
            || contract.content_profile != kind_id("conduit.todo/checkpoint-envelope@1")
            || contract.retention != ResourceRetention::ExternalDurable
            || contract.sharing != ResourceSharing::SingleWriterPublished
            || contract.maximum_bytes != CHECKPOINT_MAX_BYTES as u32
            || contract.maximum_items != 1
            || contract.generation_slots != 1
            || resource.units != 1
            || call.contract_id.as_str() != operation
            || call.target_kind.as_ref() != Some(&placement.kind_id)
            || call.maximum_in_flight != 1
            || call.maximum_input_bytes != 4096
            || call.maximum_output_bytes != 4096
            || authority.contract_id.as_str() != AUTHORITY_CONTRACT
            || authority.host_call_contract_id != call.contract_id
            || authority.host_id != placement.host_id
            || authority.boot_id != placement.boot_id
            || authority.subject_kind != placement.kind_id
            || authority.capability_id != placement.capability_id
            || authority.grant_id.as_str().is_empty()
        {
            return Err(Refusal::InvalidBinding);
        }
        let mut key = Vec::new();
        key.extend_from_slice(&contract.identity.digest());
        for part in [&identity.body, &identity.plot, &identity.workload] {
            key.extend_from_slice(&(part.len() as u16).to_le_bytes());
            key.extend_from_slice(part.as_bytes());
        }
        let namespace = hex(&semantic_digest(
            "conduit.todo/checkpoint-namespace@1",
            &key,
        ));
        Ok(Self {
            root,
            authority: authority.clone(),
            access: contract.access,
            identity,
            namespace,
            semantic: contract.identity.digest(),
            generation: contract.version.digest(),
        })
    }

    /// Candidate publication precedes the atomic current-selector replacement.
    /// A failed selector sync is never reported as a successful action commit.
    pub fn commit(&self, grant: &AuthorityBinding, state: &TodoState) -> Result<(), Refusal> {
        self.authorize(grant, ResourceAccessMode::WriteCandidatePublish)?;
        let payload = state.encode_info().map_err(|_| Refusal::InvalidState)?;
        if payload.len() > STATE_MAX_BYTES {
            return Err(Refusal::InvalidState);
        }
        let mut record = Vec::with_capacity(CHECKPOINT_MAX_BYTES);
        record.extend_from_slice(MAGIC);
        record.push(SCHEMA);
        record.extend_from_slice(&self.semantic);
        record.extend_from_slice(&self.generation);
        for part in [
            &self.identity.body,
            &self.identity.plot,
            &self.identity.workload,
        ] {
            record.push(part.len() as u8);
            record.extend_from_slice(part.as_bytes());
        }
        record.extend_from_slice(&state.revision.to_le_bytes());
        record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        record.extend_from_slice(&semantic_digest(
            "conduit.todo/checkpoint-content@1",
            &payload,
        ));
        record.extend_from_slice(&payload);
        let current = self.read_selector()?;
        if current.is_some_and(|(_, revision)| revision.checked_add(1) != Some(state.revision))
            || (current.is_none() && state.revision > 1)
        {
            return Err(Refusal::StaleRevision);
        }
        let candidate = self.root.join(format!(
            "{}.{}.checkpoint",
            self.namespace,
            hex(&self.generation)
        ));
        self.write_candidate(&candidate, &record)?;
        self.sync_root()?;
        let selector = self.root.join(format!("{}.current", self.namespace));
        let staged = self.root.join(format!("{}.next", self.namespace));
        let mut selected = Vec::with_capacity(36);
        selected.extend_from_slice(&self.generation);
        selected.extend_from_slice(&state.revision.to_le_bytes());
        self.write_candidate(&staged, &selected)?;
        fs::rename(&staged, &selector).map_err(|_| Refusal::Storage)?;
        self.sync_root().map_err(|_| Refusal::UnknownOutcome)?;
        Ok(())
    }

    pub fn recover(&self, grant: &AuthorityBinding) -> Result<TodoState, Refusal> {
        self.authorize(grant, ResourceAccessMode::ReadPublished)?;
        let (generation, revision) = self.read_selector()?.ok_or(Refusal::Missing)?;
        if generation != self.generation {
            return Err(Refusal::StaleRevision);
        }
        let path = self.root.join(format!(
            "{}.{}.checkpoint",
            self.namespace,
            hex(&generation)
        ));
        let mut bytes = Vec::new();
        File::open(&path)
            .map_err(|_| Refusal::Missing)?
            .take((CHECKPOINT_MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| Refusal::Storage)?;
        if bytes.len() > CHECKPOINT_MAX_BYTES
            || !bytes.starts_with(MAGIC)
            || bytes.get(8) != Some(&SCHEMA)
        {
            return Err(Refusal::Corrupt);
        }
        if bytes.get(9..41) != Some(self.semantic.as_slice())
            || bytes.get(41..73) != Some(self.generation.as_slice())
        {
            return Err(Refusal::Corrupt);
        }
        let mut offset = 73;
        for part in [
            &self.identity.body,
            &self.identity.plot,
            &self.identity.workload,
        ] {
            let len = *bytes.get(offset).ok_or(Refusal::Corrupt)? as usize;
            offset += 1;
            if bytes.get(offset..offset + len) != Some(part.as_bytes()) {
                return Err(Refusal::Corrupt);
            }
            offset += len;
        }
        let stored_revision = u32_at(&bytes, &mut offset)?;
        let len = u32_at(&bytes, &mut offset)? as usize;
        let digest = bytes.get(offset..offset + 32).ok_or(Refusal::Corrupt)?;
        offset += 32;
        let payload = bytes.get(offset..).ok_or(Refusal::Corrupt)?;
        if stored_revision != revision
            || len != payload.len()
            || digest != semantic_digest("conduit.todo/checkpoint-content@1", payload)
        {
            return Err(Refusal::Corrupt);
        }
        let state = TodoState::decode_info(payload).map_err(|_| Refusal::Corrupt)?;
        if state.revision != revision {
            return Err(Refusal::Corrupt);
        }
        Ok(state)
    }

    fn authorize(
        &self,
        grant: &AuthorityBinding,
        access: ResourceAccessMode,
    ) -> Result<(), Refusal> {
        if self.access != access {
            return Err(Refusal::WrongAccess);
        }
        if &self.authority != grant {
            return Err(Refusal::WrongAuthority);
        }
        Ok(())
    }
    fn read_selector(&self) -> Result<Option<([u8; 32], u32)>, Refusal> {
        let path = self.root.join(format!("{}.current", self.namespace));
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Refusal::Storage),
        };
        let mut bytes = Vec::with_capacity(37);
        file.take(37)
            .read_to_end(&mut bytes)
            .map_err(|_| Refusal::Storage)?;
        if bytes.len() != 36 {
            return Err(Refusal::Corrupt);
        }
        let generation: [u8; 32] = bytes[..32].try_into().expect("32 bytes");
        let revision = u32::from_le_bytes(bytes[32..].try_into().expect("four bytes"));
        Ok(Some((generation, revision)))
    }
    fn write_candidate(&self, path: &Path, bytes: &[u8]) -> Result<(), Refusal> {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(mut file) => {
                file.write_all(bytes).map_err(|_| Refusal::Storage)?;
                file.sync_all().map_err(|_| Refusal::Storage)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let mut existing = Vec::with_capacity(bytes.len() + 1);
                File::open(path)
                    .map_err(|_| Refusal::Storage)?
                    .take((bytes.len() + 1) as u64)
                    .read_to_end(&mut existing)
                    .map_err(|_| Refusal::Storage)?;
                if existing == bytes {
                    Ok(())
                } else {
                    Err(Refusal::Corrupt)
                }
            }
            Err(_) => Err(Refusal::Storage),
        }
    }
    fn sync_root(&self) -> Result<(), Refusal> {
        File::open(&self.root)
            .and_then(|file| file.sync_all())
            .map_err(|_| Refusal::Storage)
    }
}

fn u32_at(bytes: &[u8], offset: &mut usize) -> Result<u32, Refusal> {
    let end = offset.checked_add(4).ok_or(Refusal::Corrupt)?;
    let value: [u8; 4] = bytes
        .get(*offset..end)
        .ok_or(Refusal::Corrupt)?
        .try_into()
        .map_err(|_| Refusal::Corrupt)?;
    *offset = end;
    Ok(u32::from_le_bytes(value))
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").expect("string write");
    }
    result
}
