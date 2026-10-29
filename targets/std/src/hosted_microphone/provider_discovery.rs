//! Bounded metadata discovery and explicit capture-mode command construction.
use super::*;

pub(super) fn capture_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    // Canonical arecord may be aplay; mode must not depend on argv[0].
    command.arg("--capture");
    // ALSA can negotiate a different rate with a warning despite exit success.
    // Keep diagnostics stable so capture cannot mislabel that PCM timebase.
    command.env("LC_ALL", "C");
    command
}

pub(super) fn bounded_discovery(executable: &Path) -> Result<Vec<u8>, MicrophoneFailure> {
    let mut child = capture_command(executable)
        .arg("-l")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| MicrophoneFailure::DiscoveryFailed)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(MicrophoneFailure::DiscoveryFailed)?;
    let stderr = child
        .stderr
        .take()
        .ok_or(MicrophoneFailure::DiscoveryFailed)?;
    let listing = std::thread::spawn(move || bounded_read(stdout, MAXIMUM_DISCOVERY_BYTES));
    let diagnostics = std::thread::spawn(move || bounded_read(stderr, MAXIMUM_DIAGNOSTIC_BYTES));
    let started = Instant::now();
    let status = loop {
        if started.elapsed() >= DISCOVERY_TIMEOUT {
            stop(&mut child);
            let _ = listing.join();
            let _ = diagnostics.join();
            return Err(MicrophoneFailure::DiscoveryFailed);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => {
                stop(&mut child);
                return Err(MicrophoneFailure::DiscoveryFailed);
            }
        }
    };
    let listing = listing
        .join()
        .map_err(|_| MicrophoneFailure::DiscoveryFailed)?
        .map_err(|_| MicrophoneFailure::DiscoveryFailed)?;
    let diagnostics = diagnostics
        .join()
        .map_err(|_| MicrophoneFailure::DiscoveryFailed)?
        .map_err(|_| MicrophoneFailure::DiscoveryFailed)?;
    if !status.success() || listing.overflowed || diagnostics.overflowed {
        return Err(MicrophoneFailure::DiscoveryFailed);
    }
    Ok(listing.bytes)
}

impl AlsaMicrophoneDiscovery {
    /// Enumerates ALSA control metadata without opening a PCM capture stream.
    pub fn inspect(executable: impl AsRef<Path>) -> Result<Self, MicrophoneFailure> {
        let executable = exact_file(executable.as_ref())?;
        let output = bounded_discovery(&executable)?;
        let listing =
            std::str::from_utf8(&output).map_err(|_| MicrophoneFailure::InvalidDiscovery)?;
        let observations = parse_arecord_list(listing, Path::new("/sys/class/sound"))?;
        Ok(Self {
            executable_sha256: digest_file(&executable)?,
            executable,
            observations,
        })
    }

    pub fn initialize(
        self,
        selected: &AlsaMicrophoneObservation,
        limits: MicrophoneLimits,
    ) -> Result<AlsaMicrophoneAdapter, MicrophoneFailure> {
        self.initialize_with_format(selected, limits, AlsaMicrophoneFormat::ExactHardware)
    }

    pub fn initialize_with_format(
        self,
        selected: &AlsaMicrophoneObservation,
        limits: MicrophoneLimits,
        format: AlsaMicrophoneFormat,
    ) -> Result<AlsaMicrophoneAdapter, MicrophoneFailure> {
        validate_limits(limits)?;
        if self.observations.is_empty() {
            return Err(MicrophoneFailure::NoEndpoint);
        }
        let selection = self
            .observations
            .iter()
            .find(|candidate| *candidate == selected)
            .cloned()
            .ok_or(MicrophoneFailure::SelectionDrift)?;
        Ok(AlsaMicrophoneAdapter {
            format,
            discovery: self,
            selection,
            limits,
            last_receipt: None,
        })
    }
}
