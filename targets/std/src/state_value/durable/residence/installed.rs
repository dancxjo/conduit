//! Preparation-owned storage for the installed State Host Call boundary.
//! Unix filesystem calls use prepared C paths; non-Unix keeps the existing std
//! filesystem behavior without claiming allocation-free platform path conversion.
use super::*;

pub(crate) struct InstalledDurableStateHost {
    residence: FileDurableStateResidence,
    current: PreparedPath,
    candidate: PreparedPath,
    directory: PreparedPath,
    encoded: Vec<u8>,
    response: Vec<u8>,
    generation: u64,
}

impl InstalledDurableStateHost {
    pub(crate) fn open(
        root: impl AsRef<Path>,
        binding: DurableStateBinding,
    ) -> Result<Self, DurableStateRefusal> {
        let residence = FileDurableStateResidence::open(root, binding)?;
        let maximum = residence.binding.maximum_value_bytes as usize;
        Ok(Self {
            current: PreparedPath::new(residence.current_path())?,
            candidate: PreparedPath::new(residence.candidate_path())?,
            directory: PreparedPath::new(residence.directory.clone())?,
            encoded: vec![0; HEADER_BYTES + maximum + 1],
            response: vec![0; RECOVERY_HEADER_BYTES + maximum],
            residence,
            generation: 0,
        })
    }

    fn recover(&mut self) -> Result<Option<(u64, usize)>, DurableStateRefusal> {
        let Some(mut file) = self.current.open(false)? else {
            return Ok(None);
        };
        let length = read_bounded(&mut file, &mut self.encoded)?;
        let (generation, value) =
            decode_generation(&self.residence.binding, &self.encoded[..length])?;
        self.response[..2].copy_from_slice(&[1, 1]);
        self.response[2..10].copy_from_slice(&generation.to_be_bytes());
        self.response[10..RECOVERY_HEADER_BYTES].copy_from_slice(&digest(value));
        self.response[RECOVERY_HEADER_BYTES..RECOVERY_HEADER_BYTES + value.len()]
            .copy_from_slice(value);
        Ok(Some((generation, value.len())))
    }

    pub(crate) fn recover_metadata(&mut self) -> Result<&[u8], DurableStateRefusal> {
        if self.recover()?.is_some() {
            Ok(&self.response[..RECOVERY_HEADER_BYTES])
        } else {
            self.generation = 0;
            self.response[..2].copy_from_slice(&[1, 0]);
            Ok(&self.response[..2])
        }
    }

    pub(crate) fn recover_exact(
        &mut self,
        expected_metadata: &[u8],
    ) -> Result<&[u8], DurableStateRefusal> {
        if expected_metadata.len() != RECOVERY_HEADER_BYTES || expected_metadata[..2] != [1, 1] {
            return Err(DurableStateRefusal::InvalidReceipt);
        }
        match self.recover()? {
            Some((generation, length))
                if &self.response[..RECOVERY_HEADER_BYTES] == expected_metadata =>
            {
                self.generation = generation;
                Ok(&self.response[RECOVERY_HEADER_BYTES..RECOVERY_HEADER_BYTES + length])
            }
            _ => Err(DurableStateRefusal::StaleRecovery),
        }
    }

    pub(crate) fn commit(&mut self, value: &[u8]) -> Result<&[u8], DurableStateRefusal> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(DurableStateRefusal::GenerationGap)?;
        if value.len() > self.residence.binding.maximum_value_bytes as usize {
            return Err(DurableStateRefusal::ValueTooLarge);
        }
        let hash = digest(value);
        let existing = match self.current.open(false)? {
            Some(mut file) => {
                let length = read_bounded(&mut file, &mut self.encoded)?;
                let (current, previous) =
                    decode_generation(&self.residence.binding, &self.encoded[..length])?;
                Some((current, digest(previous)))
            }
            None => None,
        };
        let publish = requires_publication(existing, generation, hash)?;
        if publish {
            let length = encode_generation_into(
                &self.residence.binding,
                generation,
                value,
                &mut self.encoded,
            )?;
            let mut candidate = self
                .candidate
                .open(true)?
                .ok_or(DurableStateRefusal::Lost)?;
            candidate
                .write_all(&self.encoded[..length])
                .and_then(|()| candidate.sync_all())
                .map_err(|_| DurableStateRefusal::Lost)?;
            drop(candidate);
            self.candidate.rename_to(&self.current)?;
            self.directory
                .open(false)?
                .ok_or(DurableStateRefusal::Lost)?
                .sync_all()
                .map_err(|_| DurableStateRefusal::Lost)?;
        }
        self.generation = generation;
        self.response[..41].copy_from_slice(&receipt(generation, hash));
        Ok(&self.response[..41])
    }
}

fn read_bounded(file: &mut File, buffer: &mut [u8]) -> Result<usize, DurableStateRefusal> {
    let mut length = 0;
    while length < buffer.len() {
        match file.read(&mut buffer[length..]) {
            Ok(0) => return Ok(length),
            Ok(read) => length += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(DurableStateRefusal::Lost),
        }
    }
    Err(DurableStateRefusal::Corrupt)
}

// Unix paths are converted during preparation even when longer than std's
// internal stack conversion buffer. No path conversion allocation occurs in Play.
struct PreparedPath {
    #[cfg(unix)]
    path: std::ffi::CString,
    #[cfg(not(unix))]
    path: PathBuf,
}
impl PreparedPath {
    fn new(path: PathBuf) -> Result<Self, DurableStateRefusal> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Ok(Self {
                path: std::ffi::CString::new(path.as_os_str().as_bytes())
                    .map_err(|_| DurableStateRefusal::Lost)?,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self { path })
        }
    }
    fn open(&self, write: bool) -> Result<Option<File>, DurableStateRefusal> {
        #[cfg(unix)]
        {
            use std::os::fd::FromRawFd;
            let flags = if write {
                libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC
            } else {
                libc::O_RDONLY
            };
            // SAFETY: prepared NUL-terminated path; successful fd is owned once.
            let fd = unsafe { libc::open(self.path.as_ptr(), flags | libc::O_CLOEXEC, 0o666) };
            if fd >= 0 {
                return Ok(Some(unsafe { File::from_raw_fd(fd) }));
            }
            let error = std::io::Error::last_os_error();
            if !write && error.kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(DurableStateRefusal::Lost)
            }
        }
        #[cfg(not(unix))]
        {
            let result = if write {
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&self.path)
            } else {
                File::open(&self.path)
            };
            match result {
                Ok(file) => Ok(Some(file)),
                Err(error) if !write && error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(_) => Err(DurableStateRefusal::Lost),
            }
        }
    }
    fn rename_to(&self, destination: &Self) -> Result<(), DurableStateRefusal> {
        #[cfg(unix)]
        {
            // SAFETY: both paths remain valid NUL-terminated strings.
            if unsafe { libc::rename(self.path.as_ptr(), destination.path.as_ptr()) } == 0 {
                Ok(())
            } else {
                Err(DurableStateRefusal::Lost)
            }
        }
        #[cfg(not(unix))]
        {
            fs::rename(&self.path, &destination.path).map_err(|_| DurableStateRefusal::Lost)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn prepared_state_host_operations_do_not_allocate() {
        let root = tempfile::tempdir().unwrap();
        let mut host = InstalledDurableStateHost::open(
            root.path(),
            DurableStateBinding {
                body: "body/prepared-state".into(),
                state: "state/current".into(),
                value_kind: "value/text".into(),
                maximum_value_bytes: 4096,
            },
        )
        .unwrap();
        let probe = crate::allocation_probe::begin();
        assert_eq!(host.recover_metadata().unwrap(), &[1, 0]);
        assert_eq!(host.commit(b"first").unwrap().len(), 41);
        let mut metadata = [0; RECOVERY_HEADER_BYTES];
        metadata.copy_from_slice(host.recover_metadata().unwrap());
        assert_eq!(host.recover_exact(&metadata).unwrap(), b"first");
        host.commit(b"second").unwrap();
        assert_eq!(
            host.recover_exact(&metadata),
            Err(DurableStateRefusal::StaleRecovery)
        );
        assert_eq!(probe.finish(), 0);
        // Existing allocating API reads exactly the same published format.
        assert_eq!(
            host.residence.recover_value().unwrap().1.unwrap(),
            b"second"
        );
    }

    #[test]
    fn prepared_reads_refuse_corrupt_and_oversized_generations() {
        let root = tempfile::tempdir().unwrap();
        let mut host = InstalledDurableStateHost::open(
            root.path(),
            DurableStateBinding {
                body: "body/prepared-state".into(),
                state: "state/current".into(),
                value_kind: "value/text".into(),
                maximum_value_bytes: 4096,
            },
        )
        .unwrap();
        fs::write(host.residence.current_path(), b"corrupt").unwrap();
        assert_eq!(host.recover_metadata(), Err(DurableStateRefusal::Corrupt));
        assert_eq!(host.commit(b"next"), Err(DurableStateRefusal::Corrupt));
        let oversized =
            vec![0; HEADER_BYTES + host.residence.binding.maximum_value_bytes as usize + 1];
        fs::write(host.residence.current_path(), oversized).unwrap();
        let probe = crate::allocation_probe::begin();
        assert_eq!(host.recover_metadata(), Err(DurableStateRefusal::Corrupt));
        assert_eq!(host.commit(b"next"), Err(DurableStateRefusal::Corrupt));
        assert_eq!(probe.finish(), 0);
    }
}
