//! Synchronized std residence for one exact Body-lived State binding.
//!
//! A candidate is written and synchronized under a separate name. Only a
//! synchronized rename followed by directory synchronization publishes it as
//! current. Recovery never consults the candidate path, so an interrupted
//! write cannot acquire committed-generation truth.

use super::{
    digest, DurableStateBinding, DurableStateRefusal, RecoveryDisposition, RECOVERY_HEADER_BYTES,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAGIC: &[u8; 8] = b"CDSTATE1";
const HEADER_BYTES: usize = MAGIC.len() + 32 + 8 + 4 + 32;

/// One host-selected residence. Its filesystem location is realization truth;
/// it never enters authored Plot meaning or the portable State identity.
pub struct FileDurableStateResidence {
    binding: DurableStateBinding,
    directory: PathBuf,
}

pub(crate) struct InstalledDurableStateHost {
    residence: FileDurableStateResidence,
    generation: u64,
}

impl InstalledDurableStateHost {
    pub(crate) fn open(
        root: impl AsRef<Path>,
        binding: DurableStateBinding,
    ) -> Result<Self, DurableStateRefusal> {
        Ok(Self {
            residence: FileDurableStateResidence::open(root, binding)?,
            generation: 0,
        })
    }

    pub(crate) fn recover_metadata(&mut self) -> Result<Vec<u8>, DurableStateRefusal> {
        let (disposition, encoded) = self.residence.recover()?;
        match disposition {
            RecoveryDisposition::Absent => {
                self.generation = 0;
                Ok(encoded)
            }
            RecoveryDisposition::Recovered { .. } => Ok(encoded[..RECOVERY_HEADER_BYTES].to_vec()),
        }
    }

    pub(crate) fn recover_exact(
        &mut self,
        expected_metadata: &[u8],
    ) -> Result<Vec<u8>, DurableStateRefusal> {
        if expected_metadata.len() != RECOVERY_HEADER_BYTES
            || expected_metadata[0] != 1
            || expected_metadata[1] != 1
        {
            return Err(DurableStateRefusal::InvalidReceipt);
        }
        let (disposition, encoded) = self.residence.recover()?;
        match disposition {
            RecoveryDisposition::Absent => Err(DurableStateRefusal::StaleRecovery),
            RecoveryDisposition::Recovered { generation }
                if &encoded[..RECOVERY_HEADER_BYTES] == expected_metadata =>
            {
                self.generation = generation;
                Ok(encoded[RECOVERY_HEADER_BYTES..].to_vec())
            }
            RecoveryDisposition::Recovered { .. } => Err(DurableStateRefusal::StaleRecovery),
        }
    }

    pub(crate) fn commit(&mut self, value: &[u8]) -> Result<[u8; 41], DurableStateRefusal> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(DurableStateRefusal::GenerationGap)?;
        let receipt = self.residence.commit(generation, value)?;
        self.generation = generation;
        Ok(receipt)
    }
}

impl FileDurableStateResidence {
    pub fn open(
        root: impl AsRef<Path>,
        binding: DurableStateBinding,
    ) -> Result<Self, DurableStateRefusal> {
        binding.validate()?;
        let key = binding_key(&binding);
        let directory = root.as_ref().join(hex(&key));
        fs::create_dir_all(&directory).map_err(|_| DurableStateRefusal::Lost)?;
        Ok(Self { binding, directory })
    }

    pub fn binding(&self) -> &DurableStateBinding {
        &self.binding
    }

    pub fn recover(&self) -> Result<(RecoveryDisposition, Vec<u8>), DurableStateRefusal> {
        let path = self.current_path();
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok((RecoveryDisposition::Absent, vec![1, 0]));
            }
            Err(_) => return Err(DurableStateRefusal::Lost),
        };
        let maximum = HEADER_BYTES
            .checked_add(self.binding.maximum_value_bytes as usize)
            .ok_or(DurableStateRefusal::Corrupt)?;
        let mut encoded = Vec::new();
        file.take((maximum + 1) as u64)
            .read_to_end(&mut encoded)
            .map_err(|_| DurableStateRefusal::Lost)?;
        if encoded.len() > maximum {
            return Err(DurableStateRefusal::Corrupt);
        }
        let (generation, value) = decode_generation(&self.binding, &encoded)?;
        let mut response = Vec::with_capacity(42 + value.len());
        response.extend_from_slice(&[1, 1]);
        response.extend_from_slice(&generation.to_be_bytes());
        response.extend_from_slice(&digest(value));
        response.extend_from_slice(value);
        Ok((RecoveryDisposition::Recovered { generation }, response))
    }

    /// Synchronize and publish one exact next generation.
    pub fn commit(&self, generation: u64, value: &[u8]) -> Result<[u8; 41], DurableStateRefusal> {
        if value.len() > self.binding.maximum_value_bytes as usize {
            return Err(DurableStateRefusal::ValueTooLarge);
        }
        let value_digest = digest(value);
        match self.read_current_generation()? {
            Some((current, current_digest)) if current == generation => {
                return if current_digest == value_digest {
                    Ok(receipt(generation, value_digest))
                } else {
                    Err(DurableStateRefusal::ConflictingDigest)
                };
            }
            Some((current, _)) if current.checked_add(1) != Some(generation) => {
                return Err(DurableStateRefusal::GenerationGap);
            }
            None if generation != 1 => return Err(DurableStateRefusal::GenerationGap),
            _ => {}
        }
        self.write_candidate(generation, value)?;
        fs::rename(self.candidate_path(), self.current_path())
            .map_err(|_| DurableStateRefusal::Lost)?;
        File::open(&self.directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| DurableStateRefusal::Lost)?;
        Ok(receipt(generation, value_digest))
    }

    fn write_candidate(&self, generation: u64, value: &[u8]) -> Result<(), DurableStateRefusal> {
        let encoded = encode_generation(&self.binding, generation, value)?;
        let mut candidate = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(self.candidate_path())
            .map_err(|_| DurableStateRefusal::Lost)?;
        candidate
            .write_all(&encoded)
            .and_then(|()| candidate.sync_all())
            .map_err(|_| DurableStateRefusal::Lost)
    }

    fn read_current_generation(&self) -> Result<Option<(u64, [u8; 32])>, DurableStateRefusal> {
        let mut encoded = Vec::new();
        match File::open(self.current_path()) {
            Ok(mut file) => {
                file.read_to_end(&mut encoded)
                    .map_err(|_| DurableStateRefusal::Lost)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(DurableStateRefusal::Lost),
        }
        let (generation, value) = decode_generation(&self.binding, &encoded)?;
        Ok(Some((generation, digest(value))))
    }

    fn current_path(&self) -> PathBuf {
        self.directory.join("current")
    }

    fn candidate_path(&self) -> PathBuf {
        self.directory.join("candidate")
    }

    #[cfg(test)]
    fn synchronize_candidate_without_publication(
        &self,
        generation: u64,
        value: &[u8],
    ) -> Result<(), DurableStateRefusal> {
        self.write_candidate(generation, value)
    }
}

fn binding_key(binding: &DurableStateBinding) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for value in [
        binding.body.as_bytes(),
        binding.state.as_str().as_bytes(),
        binding.value_kind.as_str().as_bytes(),
    ] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value);
    }
    hasher.update(binding.maximum_value_bytes.to_be_bytes());
    hasher.finalize().into()
}

fn encode_generation(
    binding: &DurableStateBinding,
    generation: u64,
    value: &[u8],
) -> Result<Vec<u8>, DurableStateRefusal> {
    if generation == 0 || value.len() > binding.maximum_value_bytes as usize {
        return Err(DurableStateRefusal::ValueTooLarge);
    }
    let mut encoded = Vec::with_capacity(HEADER_BYTES + value.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&binding_key(binding));
    encoded.extend_from_slice(&generation.to_be_bytes());
    encoded.extend_from_slice(&(value.len() as u32).to_be_bytes());
    encoded.extend_from_slice(&digest(value));
    encoded.extend_from_slice(value);
    Ok(encoded)
}

fn decode_generation<'a>(
    binding: &DurableStateBinding,
    encoded: &'a [u8],
) -> Result<(u64, &'a [u8]), DurableStateRefusal> {
    if encoded.len() < HEADER_BYTES || &encoded[..MAGIC.len()] != MAGIC {
        return Err(DurableStateRefusal::Corrupt);
    }
    if encoded[MAGIC.len()..MAGIC.len() + 32] != binding_key(binding) {
        return Err(DurableStateRefusal::Incompatible);
    }
    let mut cursor = MAGIC.len() + 32;
    let generation = u64::from_be_bytes(
        encoded[cursor..cursor + 8]
            .try_into()
            .map_err(|_| DurableStateRefusal::Corrupt)?,
    );
    cursor += 8;
    let value_len = u32::from_be_bytes(
        encoded[cursor..cursor + 4]
            .try_into()
            .map_err(|_| DurableStateRefusal::Corrupt)?,
    ) as usize;
    cursor += 4;
    let expected: [u8; 32] = encoded[cursor..cursor + 32]
        .try_into()
        .map_err(|_| DurableStateRefusal::Corrupt)?;
    cursor += 32;
    if generation == 0
        || value_len > binding.maximum_value_bytes as usize
        || encoded.len() != cursor + value_len
        || digest(&encoded[cursor..]) != expected
    {
        return Err(DurableStateRefusal::Corrupt);
    }
    Ok((generation, &encoded[cursor..]))
}

fn receipt(generation: u64, digest: [u8; 32]) -> [u8; 41] {
    let mut receipt = [0; 41];
    receipt[0] = 1;
    receipt[1..9].copy_from_slice(&generation.to_be_bytes());
    receipt[9..].copy_from_slice(&digest);
    receipt
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{KindId, StateId};
    use std::{process::Command, time::SystemTime};

    const CHILD_MODE: &str = "CONDUIT_DURABLE_STATE_CHILD_MODE";
    const CHILD_ROOT: &str = "CONDUIT_DURABLE_STATE_CHILD_ROOT";

    fn binding() -> DurableStateBinding {
        DurableStateBinding {
            body: "body/notebook".into(),
            state: StateId::from("note/current"),
            value_kind: KindId::from("value/text"),
            maximum_value_bytes: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
        }
    }

    fn temporary_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "conduit-durable-state-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn child(mode: &str, root: &Path) -> ! {
        let residence = FileDurableStateResidence::open(root, binding()).unwrap();
        match mode {
            "commit" => {
                residence
                    .commit(1, b"published before abrupt loss")
                    .unwrap();
            }
            "torn" => {
                residence
                    .synchronize_candidate_without_publication(2, b"unpublished candidate")
                    .unwrap();
            }
            _ => panic!("unknown child mode"),
        }
        std::process::abort()
    }

    fn spawn_abrupt(mode: &str, root: &Path) {
        let status = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("state_value::durable::residence::tests::fresh_process_abrupt_loss_recovers_only_published_generation")
            .arg("--nocapture")
            .env(CHILD_MODE, mode)
            .env(CHILD_ROOT, root)
            .status()
            .unwrap();
        assert!(!status.success(), "child must end by abrupt process loss");
    }

    #[test]
    fn fresh_process_abrupt_loss_recovers_only_published_generation() {
        if let (Ok(mode), Ok(root)) = (std::env::var(CHILD_MODE), std::env::var(CHILD_ROOT)) {
            child(&mode, Path::new(&root));
        }
        let root = temporary_root("abrupt");
        spawn_abrupt("commit", &root);
        let residence = FileDurableStateResidence::open(&root, binding()).unwrap();
        assert_eq!(
            residence.recover().unwrap().0,
            RecoveryDisposition::Recovered { generation: 1 }
        );
        spawn_abrupt("torn", &root);
        let reopened = FileDurableStateResidence::open(&root, binding()).unwrap();
        let (disposition, encoded) = reopened.recover().unwrap();
        assert_eq!(
            disposition,
            RecoveryDisposition::Recovered { generation: 1 }
        );
        assert!(encoded.ends_with(b"published before abrupt loss"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn commit_is_idempotent_and_conflicts_are_exact() {
        let root = temporary_root("conflict");
        let residence = FileDurableStateResidence::open(&root, binding()).unwrap();
        let first = residence.commit(1, b"A").unwrap();
        assert_eq!(residence.commit(1, b"A").unwrap(), first);
        assert_eq!(
            residence.commit(1, b"B"),
            Err(DurableStateRefusal::ConflictingDigest)
        );
        assert_eq!(
            residence.commit(3, b"C"),
            Err(DurableStateRefusal::GenerationGap)
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn exact_recovery_refuses_drift_and_carries_the_full_text_bound() {
        let root = temporary_root("exact-four-kib");
        let mut installed = InstalledDurableStateHost::open(&root, binding()).unwrap();
        let value = vec![b'n'; conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize];
        installed.commit(&value).unwrap();
        let metadata = installed.recover_metadata().unwrap();
        assert_eq!(installed.recover_exact(&metadata).unwrap(), value);

        installed.commit(b"new generation").unwrap();
        assert_eq!(
            installed.recover_exact(&metadata),
            Err(DurableStateRefusal::StaleRecovery)
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn residence_distinguishes_absent_corrupt_incompatible_and_lost() {
        let root = temporary_root("recovery-dispositions");
        let residence = FileDurableStateResidence::open(&root, binding()).unwrap();
        assert_eq!(residence.recover().unwrap().0, RecoveryDisposition::Absent);
        residence.commit(1, b"A").unwrap();
        fs::write(residence.current_path(), b"not a generation").unwrap();
        assert_eq!(residence.recover(), Err(DurableStateRefusal::Corrupt));

        let mut foreign_binding = binding();
        foreign_binding.body = "body/foreign".into();
        fs::write(
            residence.current_path(),
            encode_generation(&foreign_binding, 1, b"A").unwrap(),
        )
        .unwrap();
        assert_eq!(residence.recover(), Err(DurableStateRefusal::Incompatible));

        fs::remove_file(residence.current_path()).unwrap();
        fs::create_dir(residence.current_path()).unwrap();
        assert_eq!(residence.recover(), Err(DurableStateRefusal::Lost));
        fs::remove_dir_all(&root).unwrap();
    }
}
