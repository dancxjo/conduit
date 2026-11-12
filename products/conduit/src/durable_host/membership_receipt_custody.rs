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

/// Reserve a separate public cursor before consuming return authority. The
/// original basis remains available if delivery of the new receipt is lost.
pub(super) fn reserve_return_basis(
    receipt_path: &Path,
) -> Result<(std::path::PathBuf, File), String> {
    let path = receipt_path.with_extension("return-basis.json");
    let file = reserve(&path).map_err(|error| format!("reserve next return basis: {error}"))?;
    Ok((path, file))
}

/// A current owner credential, not device acceptance, Play or recording custody.
/// This authenticates the complete receipt/projection before publishing fields.
pub(super) fn retain_return_basis(
    file: &mut File,
    original: &conduit_std_host::pico_spawn::PicoMemberReturnBasis,
    signed: &conduit_body::SignedPortableAdmissionReceipt,
    actual: &conduit_body::HostAdvertisementImageProjection,
) -> Result<(), String> {
    let next = original
        .advance_from_verified_receipt(signed, actual)
        .map_err(|error| format!("return committed; next basis verification refused: {error:?}"))?;
    let bytes = serde_json::to_vec(&next)
        .map_err(|error| format!("encode verified return basis: {error}"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("return committed; next basis persistence failed: {error}"))
}
