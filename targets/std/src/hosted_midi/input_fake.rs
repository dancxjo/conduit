use super::MidiInputFailure;
use std::fs::File;

#[cfg(unix)]
pub(super) fn pipe(
    bytes: &[u8],
    stays_open: bool,
) -> Result<(File, Option<File>), MidiInputFailure> {
    use std::io::Write;
    use std::os::fd::FromRawFd;

    let mut descriptors = [0; 2];
    if unsafe { libc::pipe(descriptors.as_mut_ptr()) } != 0 {
        return Err(MidiInputFailure::BackendUnavailable);
    }
    let flags = unsafe { libc::fcntl(descriptors[0], libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(descriptors[0], libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0
    {
        unsafe {
            libc::close(descriptors[0]);
            libc::close(descriptors[1]);
        }
        return Err(MidiInputFailure::BackendUnavailable);
    }
    let reader = unsafe { File::from_raw_fd(descriptors[0]) };
    let mut writer = unsafe { File::from_raw_fd(descriptors[1]) };
    writer
        .write_all(bytes)
        .map_err(|_| MidiInputFailure::ProviderLost)?;
    let writer = stays_open.then_some(writer);
    Ok((reader, writer))
}

#[cfg(not(unix))]
pub(super) fn pipe(
    _bytes: &[u8],
    _stays_open: bool,
) -> Result<(File, Option<File>), MidiInputFailure> {
    Err(MidiInputFailure::BackendUnavailable)
}
