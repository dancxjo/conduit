//! One private, bounded Seed candidate, sealed only by the Started provider.
use super::*;
use conduit_body::{PortableInvitation, INVITATION_SCHEMA};
use conduit_body_make::{
    seal_joining_host_release, write_joining_host_package, JoiningHostPackageReceipt,
    JoiningReleaseSelection,
};
use conduit_host_make::{
    build_host_image, BuildInputs, MakeCatalog, MakePackageSet, SporeOutputKind,
};
use std::io::{Cursor, Write};

const MAXIMUM_BYTES: usize = 64 * 1024 * 1024;
pub(super) struct Seed {
    pub(super) bytes: Vec<u8>,
    pub(super) receipt: Option<JoiningHostPackageReceipt>,
}
impl Drop for Seed {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}
impl Seed {
    pub(super) fn reserve() -> Self {
        Self {
            bytes: Vec::with_capacity(MAXIMUM_BYTES),
            receipt: None,
        }
    }
}
struct Output<'a>(&'a mut Vec<u8>);
impl Write for Output<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .0
            .len()
            .checked_add(bytes.len())
            .is_none_or(|size| size > MAXIMUM_BYTES)
        {
            return Err(std::io::Error::other("Seed exceeds its reserved bound"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) fn prepare(
    material: &mut staging::Material,
    body: &BodyLifecycleSession,
    configuration: &CheckedHostConfiguration,
    invitation: &SpawnInvitation,
    now: u64,
) -> Result<(), String> {
    let derived = material
        .derived
        .as_ref()
        .ok_or("Browser IMAGE is not prepared")?;
    let verified = derived
        .verified
        .as_ref()
        .ok_or("Derived browser release bytes are unverified")?;
    let output = material
        .seed
        .as_mut()
        .ok_or("Seed output capacity was not reserved before Play")?;
    if output.receipt.is_some() {
        return Ok(());
    }
    let packages = MakePackageSet::compose(&[&conduit_host_browser_make::BrowserMakePackage])
        .map_err(|error| format!("Browser Make: {error:?}"))?;
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let (image, image_bytes) = build_host_image(
        configuration.profile().clone(),
        &catalog,
        &packages,
        &SporeOutputKind::BrowserBundle,
        &BuildInputs {
            source_identity: derived.manifest.source_identity.clone(),
            toolchain_available: true,
        },
    )
    .map_err(|error| format!("Browser Make IMAGE: {error:?}"))?;
    let built = seal_joining_host_release(
        JoiningReleaseSelection {
            body: body.evidence(),
            invitation: &invitation.claim(),
            configuration,
            output: SporeOutputKind::BrowserBundle,
            image: &image,
            image_manifest: &image_bytes,
            release: verified,
        },
        &catalog,
        &packages,
        now,
    )?;
    let mut portable = PortableInvitation {
        schema: INVITATION_SCHEMA.into(),
        claim: invitation.claim(),
        secret: invitation.secret.copy_for_target_provisioning(),
        rendezvous: None,
    };
    output.bytes.clear();
    let result = write_joining_host_package(
        verified,
        &built,
        configuration,
        &portable,
        now,
        |path| {
            if path == "conduit-browser-image.json" {
                return Ok(Cursor::new(derived.image_bytes.as_slice()));
            }
            material
                .manifest
                .files
                .iter()
                .position(|file| file.path == path)
                .map(|index| Cursor::new(material.payloads[index].as_slice()))
                .ok_or_else(|| "Undeclared Seed payload".into())
        },
        &mut Output(&mut output.bytes),
        || false,
    );
    portable.secret.fill(0);
    match result {
        Ok(receipt) => {
            output.receipt = Some(receipt);
            Ok(())
        }
        Err(error) => {
            output.bytes.fill(0);
            output.bytes.clear();
            Err(error)
        }
    }
}
