//! Reserve durable receipt custody before consuming membership authority.
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};
pub(super) fn reserve(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|e| format!("reserve new private receipt output: {e}"))?;
    #[cfg(unix)]
    File::open(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
    .and_then(|directory| directory.sync_all())
    .map_err(|e| format!("retain receipt directory entry before admission: {e}"))?;
    Ok(file)
}
pub(super) fn retain(
    file: &mut File,
    receipt: &conduit_body::SignedPortableAdmissionReceipt,
) -> Result<(), String> {
    let bytes =
        serde_json::to_vec(receipt).map_err(|e| format!("encode committed receipt: {e}"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("membership committed; receipt persistence failed: {e}"))
}
