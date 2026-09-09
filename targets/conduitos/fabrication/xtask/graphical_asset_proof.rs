//! Product-image asset presence and absence, independent of host test linkage.
use super::{live_media, report::sha256_file, ConduitosError};
use crate::cli::GlobalOpts;
use std::{fs, path::Path};

pub(super) fn prove(
    root: &Path,
    native: &Path,
    opts: &GlobalOpts,
) -> Result<Vec<serde_json::Value>, ConduitosError> {
    let signatures = ["body", "heading", "title", "code"]
        .into_iter()
        .map(|name| {
            let bytes =
                fs::read(root.join(format!("targets/conduitos/assets/graphical/{name}.atlas")))
                    .map_err(io_error)?;
            Ok(bytes[..256].to_vec())
        })
        .collect::<Result<Vec<_>, ConduitosError>>()?;
    check(native, &signatures, true)?;
    let mut evidence = Vec::new();
    for host in [
        live_media::LiveHost::Ia32,
        live_media::LiveHost::Aarch64,
        live_media::LiveHost::Riscv64,
        live_media::LiveHost::Loongarch64,
    ] {
        live_media::build(host, opts)?;
        let row = host.row();
        let image = live_media::output(root, host).join(row.artifact);
        check(&image, &signatures, false)?;
        evidence.push(serde_json::json!({"host":row.host, "image_sha256":sha256_file(&image)?, "graphical_assets":false}));
    }
    Ok(evidence)
}

fn check(path: &Path, signatures: &[Vec<u8>], expected: bool) -> Result<(), ConduitosError> {
    let image = fs::read(path).map_err(io_error)?;
    let marker = b"CONDUIT_GRAPHICAL_PROFILE";
    if image.windows(marker.len()).any(|bytes| bytes == marker) != expected
        || signatures.iter().any(|signature| {
            image
                .windows(signature.len())
                .any(|bytes| bytes == signature)
                != expected
        })
    {
        return Err(ConduitosError::refusal(
            "graphical-asset-closure-mismatch",
            format!("{} must have graphical assets: {expected}", path.display()),
        ));
    }
    Ok(())
}

fn io_error(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("graphical-asset-proof-unavailable", error.to_string())
}
