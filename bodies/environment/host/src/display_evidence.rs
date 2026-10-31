//! Durable owner custody precedes any signed acceptance sent back to the MCU.
use crate::{
    admission::Authority, native_display_evidence::Offer, native_owner_control::Request, Result,
};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
};
pub fn retain(
    authority: &Authority,
    request: &Request<'_>,
    reply: &serde_json::Value,
    offer: &Offer,
    bytes: &[u8],
) -> Result<()> {
    let directory = authority.state.join("display-evidence");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)?;
    let digest = offer
        .page
        .sha256
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let stem = format!("{:020}-{digest}", offer.page.invocation);
    let path = directory.join(format!("{stem}.bin"));
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
    {
        Ok(mut file) => {
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if std::fs::read(&path)? != bytes {
                return Err("retained display evidence differs".into());
            }
        }
        Err(error) => return Err(error.into()),
    }
    let mut metadata = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join(format!("{stem}-q{}.json", request.sequence)))?;
    serde_json::to_writer(
        &mut metadata,
        &serde_json::json!({"request":request,"offer":offer,"root_signature":reply["evidence_signature"]}),
    )?;
    metadata.write_all(b"\n")?;
    metadata.sync_all()?;
    File::open(&directory)?.sync_all()?;
    File::open(&authority.state)?.sync_all()?;
    Ok(())
}
