//! Explicit finite storage custody for automatic canonical observer return.
//! Neither a transport refusal nor a directory name authorizes a credential.
use super::*;
use std::path::PathBuf;

pub(crate) struct Options {
    directory: PathBuf,
    capacity: u8,
}
pub(crate) fn take_options(args: &mut Vec<String>) -> Result<Option<Options>> {
    let Some(index) = args.iter().position(|arg| arg == "--observer-return") else {
        return Ok(None);
    };
    if args.get(1).map(String::as_str) != Some("--record-joined-continuing")
        || index < 2
        || index + 2 >= args.len()
    {
        return Err(
            "expected continuing recorder --observer-return PRIVATE_DIRECTORY CAPACITY (1..8)"
                .into(),
        );
    }
    let capacity: u8 = args[index + 2].parse()?;
    if !(1..=8).contains(&capacity) {
        return Err("observer return capacity must be 1..8".into());
    }
    let directory = PathBuf::from(&args[index + 1]);
    args.drain(index..index + 3);
    if args.iter().any(|arg| arg == "--observer-return") {
        return Err("duplicate observer return policy".into());
    }
    Ok(Some(Options {
        directory,
        capacity,
    }))
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    original_binding: [u8; 32],
    continuity: [u8; 32],
    capacity: u8,
}
pub(crate) struct Owner {
    options: Options,
    binding: PathBuf,
    continuity: PathBuf,
    completed: u8,
}
impl Owner {
    pub(crate) fn open(binding: &Path, continuity: &Path, options: Options) -> Result<Self> {
        let admitted = crate::local_recorder::validated_binding(binding)?;
        let bootstrap: Bootstrap = load(continuity)?;
        if admitted.root() != &bootstrap.root_key
            || admitted.invitation() != &bootstrap.invitation.claim
            || admitted.producer_host() != &bootstrap.producer_host
            || admitted.credential().host_id.as_str()
                != format!(
                    "linux/{}",
                    std::fs::read_to_string("/etc/machine-id")?.trim()
                )
        {
            return Err("observer return original Host/continuity differs".into());
        }
        if !(1..=8).contains(&options.capacity) {
            return Err("observer return capacity must be 1..8".into());
        }
        crate::observed_history::create_durable_directory(&options.directory)?;
        private(&options.directory, true)?;
        let expected = Manifest {
            schema: "conduit.environment/observer-return-custody@1".into(),
            original_binding: digest(binding)?,
            continuity: digest(continuity)?,
            capacity: options.capacity,
        };
        let manifest = options.directory.join("custody.json");
        if manifest.exists() {
            if load::<Manifest>(&manifest)? != expected {
                return Err("observer return custody differs; capacity cannot be renewed".into());
            }
        } else {
            if std::fs::read_dir(&options.directory)?.next().is_some() {
                return Err("new observer return custody must be empty".into());
            }
            save(&manifest, &expected)?;
        }
        let mut owner = Self {
            options,
            binding: binding.into(),
            continuity: continuity.into(),
            completed: 0,
        };
        let mut gap = false;
        for index in 0..owner.options.capacity {
            let state = owner.state(index);
            if !state.exists() {
                gap = true;
            } else {
                private(&state, true)?;
                if gap {
                    return Err("observer return generation gap".into());
                }
                if state.join("recorder-binding.json").exists() {
                    // Completed-state reopening is local and verifies the exact
                    // original challenge; it must never make a network request.
                    super::returning::reopen_completed(&owner.binding, &owner.continuity, &state)?;
                    owner.binding = state.join("recorder-binding.json");
                    owner.completed += 1;
                } else {
                    gap = true;
                }
            }
        }
        // Reject out-of-policy generations and unrelated files with finite work.
        for (index, entry) in std::fs::read_dir(&owner.options.directory)?.enumerate() {
            if index > usize::from(owner.options.capacity) {
                return Err("observer return custody directory bound".into());
            }
            let path = entry?.path();
            if path != manifest && !(0..owner.options.capacity).any(|i| path == owner.state(i)) {
                return Err("unexpected observer return custody entry".into());
            }
        }
        Ok(owner)
    }
    fn state(&self, index: u8) -> PathBuf {
        self.options.directory.join(format!("return-{index:02}"))
    }
    pub(crate) fn binding(&self) -> &Path {
        &self.binding
    }
    pub(crate) fn advance(&mut self, peer: &str, deadline: Instant) -> Result<()> {
        if self.completed >= self.options.capacity {
            return Err(
                "observer return custody exhausted; existing history remains available".into(),
            );
        }
        let state = self.state(self.completed);
        return_observer_until(&self.binding, &self.continuity, &state, peer, deadline)?;
        // Only the canonical return verifier can publish this immutable binding.
        self.binding = state.join("recorder-binding.json");
        self.completed += 1;
        Ok(())
    }
}
fn digest(path: &Path) -> Result<[u8; 32]> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::OpenOptionsExt;
    private(path, false)?;
    let mut bytes = Vec::new();
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?
        .take(16_385)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16_384 {
        return Err("observer return input extent".into());
    }
    Ok(Sha256::digest(bytes).into())
}
