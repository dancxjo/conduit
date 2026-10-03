//! Seal a checked Body invitation into one verified, bootable ConduitOS ISO.

use std::{
    fs::{self, File, OpenOptions},
    io::{Cursor, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use clap::Args as ClapArgs;
use conduit_body::{PortableInvitation, RendezvousLineFamily, SpawnRendezvousDescriptor};
use conduit_body_make::{
    seal_reviewed_prebuilt_body_spore_with_content_digest, SelectedPrebuiltContent, SporeBinding,
};
use conduit_host_make::HostImage;
use conduitos::spore_provision::{
    decode as decode_native_media, validate_image_binding, validate_route_certificates,
    RouteCertificate, MAGIC, MAX_ROUTE_CERTIFICATE_BYTES, REGION_BYTES,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{cli::GlobalOpts, commands::host::host_target};

use super::load;

const TARGET: &str = "conduitos/x86_64/pc";
const HEADER_BYTES: usize = 32;
const MAXIMUM_ISO_BYTES: u64 = 80 * 1024 * 1024;
const MAXIMUM_INVITATION_BYTES: u64 = 64 * 1024;

#[derive(ClapArgs, Debug)]
pub(super) struct Args {
    /// Checked Body construction source containing the self-joining Host.
    body: PathBuf,
    /// Host entry in that Body description.
    #[arg(long)]
    host: String,
    /// Directory made by `cargo xtask make conduitos live x86_64`.
    #[arg(long)]
    build: PathBuf,
    /// Fresh JSON invitation issued by the running Body owner.
    #[arg(long)]
    invitation: PathBuf,
    /// Repeat for each distinct TLS leaf certificate named by the owner route.
    #[arg(long = "route-tls-cert")]
    route_tls_certs: Vec<PathBuf>,
    /// New, private provisioned ISO; an existing path is refused.
    #[arg(long)]
    output: PathBuf,
}

#[derive(Serialize)]
struct ProvisionReceipt<'a> {
    schema: &'static str,
    body_id: &'a str,
    spore_id: &'a str,
    invitation_id: &'a str,
    image_sha256: &'a str,
    artifact_sha256: String,
    source_identity: &'a str,
    output: String,
    created: bool,
    membership_admitted: bool,
}

pub(super) fn run(args: Args, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let body = load(&args.body)?;
    let build = host_target::verify_target(&args.build)?;
    if build.target != TARGET {
        return Err(format!(
            "ConduitOS provisioning requires {TARGET}, found {}",
            build.target
        )
        .into());
    }
    let image_path = args.build.join(&build.image.file);
    let mut image_bytes = read_regular(&image_path, MAXIMUM_ISO_BYTES)?;
    let image_sha256 = sha256(&image_bytes);
    if image_sha256 != build.image.sha256 {
        return Err("verified ConduitOS image changed while provisioning".into());
    }
    let description_bytes = read_regular(&args.build.join("resolved-image.json"), 1024 * 1024)?;
    let image: HostImage = serde_json::from_slice(&description_bytes)?;
    let selected_digest = format!("sha256:{image_sha256}");
    let spore = seal_reviewed_prebuilt_body_spore_with_content_digest(
        &body,
        &args.host,
        &build.source_identity,
        &image,
        SelectedPrebuiltContent {
            image_manifest_bytes: &description_bytes,
            image_content_digest: &selected_digest,
        },
        &conduit_workspace_make::package_set(),
    )
    .map_err(|error| format!("checked Body/ISO binding refused: {error:?}"))?;
    if spore.manifest.target != TARGET || spore.manifest.image_content_digest != selected_digest {
        return Err("checked spore lost exact ConduitOS image binding".into());
    }
    let mut invitation_bytes = read_regular(&args.invitation, MAXIMUM_INVITATION_BYTES)?;
    let mut invitation: PortableInvitation = serde_json::from_slice(&invitation_bytes)?;
    invitation_bytes.fill(0);
    let now_millis = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    invitation
        .validate(now_millis)
        .map_err(|error| format!("owner invitation refused: {error:?}"))?;
    let SporeBinding::SelfJoining { invitation_id } = &spore.manifest.binding else {
        return Err("checked Body Host is not self-joining".into());
    };
    if spore.manifest.body_id != invitation.claim.body_id.as_str()
        || invitation_id != invitation.claim.invitation_id.as_str()
    {
        return Err("checked Body spore differs from the owner's exact invitation".into());
    }
    let route_certificates =
        certificates_for_route(invitation.rendezvous.as_ref(), &args.route_tls_certs)?;
    let provision = serde_json::json!({
        "schema": "conduit.spore/native-media-provision@1",
        "image_bytes": image_bytes.len(),
        "spore": spore.manifest,
        "invitation_provision": {
            "invitation_id": invitation_id,
            "nonce": invitation.claim.nonce,
            "expires_at_millis": invitation.claim.expires_at_millis,
            "secret": invitation.secret,
            "rendezvous_candidates": [],
            "rendezvous": invitation.rendezvous.as_ref(),
            "route_certificates": route_certificates,
        }
    });
    let mut encoded = serde_json::to_vec(&provision)?;
    invitation.secret.fill(0);
    let mut region = encode_region(&encoded)?;
    encoded.fill(0);
    let guest_provision = decode_native_media(&region)
        .map_err(|error| format!("guest media decoder refused provision: {error:?}"))?
        .ok_or("guest media decoder returned a blank provision")?;
    validate_image_binding(&guest_provision, TARGET, &build.profile_id, &build.build_id)
        .map_err(|error| format!("guest image binding refused provision: {error:?}"))?;
    let offset = locate_blank_region(&image_bytes)?;
    image_bytes[offset..offset + REGION_BYTES].copy_from_slice(&region);
    region.fill(0);
    let artifact_sha256 = sha256(&image_bytes);
    if !opts.dry_run {
        write_private_new(&args.output, &image_bytes)?;
    }
    image_bytes.fill(0);
    let receipt = ProvisionReceipt {
        schema: "conduit.conduitos/native-spore-provision@1",
        body_id: &spore.manifest.body_id,
        spore_id: &spore.manifest.spore_id,
        invitation_id,
        image_sha256: &selected_digest,
        artifact_sha256,
        source_identity: &build.source_identity,
        output: args.output.display().to_string(),
        created: !opts.dry_run,
        membership_admitted: false,
    };
    if opts.json {
        println!("{}", serde_json::to_string(&receipt)?);
    } else if !opts.quiet {
        if opts.dry_run {
            println!("Would provision ConduitOS spore: {}", args.output.display());
        } else {
            println!("ConduitOS spore: {}", args.output.display());
        }
        println!("spore: {}", receipt.spore_id);
        println!("membership: pending owner admission");
    }
    Ok(())
}

fn certificates_for_route(
    rendezvous: Option<&SpawnRendezvousDescriptor>,
    paths: &[PathBuf],
) -> Result<Vec<RouteCertificate>, Box<dyn std::error::Error>> {
    if rendezvous.is_none() && !paths.is_empty() {
        return Err("unrouted invitation cannot use an owner route certificate".into());
    }
    if paths.len() > conduit_body::MAX_RENDEZVOUS_CANDIDATES {
        return Err("owner route certificate count exceeds the finite candidate bound".into());
    }
    let mut supplied = Vec::new();
    for path in paths {
        let bytes = read_regular(path, 8 * 1024)?;
        let mut reader = Cursor::new(bytes);
        let certificates = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
        let leaf = certificates
            .first()
            .ok_or("owner route certificate PEM contains no certificate")?;
        let der = leaf.as_ref().to_vec();
        if der.is_empty() || der.len() > MAX_ROUTE_CERTIFICATE_BYTES {
            return Err("owner route leaf certificate exceeds the native TLS bound".into());
        }
        let digest: [u8; 32] = Sha256::digest(&der).into();
        if supplied.iter().any(|(prior, _)| *prior == digest) {
            return Err("duplicate owner route leaf certificate".into());
        }
        supplied.push((digest, der));
    }
    let mut selected = Vec::new();
    if let Some(rendezvous) = rendezvous {
        for candidate in &rendezvous.candidates {
            if candidate.line_family == RendezvousLineFamily::AuthenticatedTlsStream {
                let (_, der) = supplied
                    .iter()
                    .find(|(digest, _)| {
                        *digest == candidate.authentication.transport_binding_sha256
                    })
                    .ok_or("owner route TLS certificate does not match its invitation binding")?;
                selected.push(RouteCertificate {
                    candidate_id: candidate.candidate_id.clone(),
                    certificate_der: der.clone(),
                });
            }
        }
    }
    if supplied.iter().any(|(digest, _)| {
        !rendezvous.is_some_and(|route| {
            route.candidates.iter().any(|candidate| {
                candidate.line_family == RendezvousLineFamily::AuthenticatedTlsStream
                    && candidate.authentication.transport_binding_sha256 == *digest
            })
        })
    }) {
        return Err("owner route certificate was not named by the invitation".into());
    }
    validate_route_certificates(rendezvous, &selected)
        .map_err(|_| "owner route certificate lost its exact candidate binding")?;
    Ok(selected)
}

fn read_regular(path: &Path, maximum: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > maximum {
        return Err(format!("expected bounded regular file: {}", path.display()).into());
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(format!("file changed while reading: {}", path.display()).into());
    }
    Ok(bytes)
}

fn blank_region() -> [u8; REGION_BYTES] {
    let mut region = [0xff; REGION_BYTES];
    region[..MAGIC.len()].copy_from_slice(MAGIC);
    region[24..HEADER_BYTES].fill(0);
    region
}

fn locate_blank_region(bytes: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
    let blank = blank_region();
    let mut found = None;
    for offset in 0..bytes.len().saturating_sub(REGION_BYTES - 1) {
        if bytes[offset] == MAGIC[0]
            && bytes[offset..offset + REGION_BYTES] == blank
            && found.replace(offset).is_some()
        {
            return Err("ConduitOS ISO has ambiguous blank spore regions".into());
        }
    }
    found.ok_or_else(|| "ConduitOS ISO has no unique blank spore region".into())
}

fn encode_region(encoded: &[u8]) -> Result<[u8; REGION_BYTES], Box<dyn std::error::Error>> {
    if encoded.is_empty() || encoded.len() > REGION_BYTES - HEADER_BYTES {
        return Err(format!(
            "native spore provision uses {} bytes; finite media region permits at most {}",
            encoded.len(),
            REGION_BYTES - HEADER_BYTES
        )
        .into());
    }
    let mut region = blank_region();
    region[24..28].copy_from_slice(&1_u32.to_le_bytes());
    region[28..32].copy_from_slice(&u32::try_from(encoded.len())?.to_le_bytes());
    region[HEADER_BYTES..HEADER_BYTES + encoded.len()].copy_from_slice(encoded);
    Ok(region)
}

fn write_private_new(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() || fs::symlink_metadata(path).is_ok() {
        return Err(format!(
            "refusing to replace existing spore media: {}",
            path.display()
        )
        .into());
    }
    let temp = path.with_extension(format!("partial-{}", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file: File = options.open(&temp)?;
    let result = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::hard_link(&temp, path));
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    } else {
        fs::remove_file(&temp)?;
    }
    result?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::{
        blank_region, certificates_for_route, encode_region, locate_blank_region,
        write_private_new, REGION_BYTES,
    };
    use conduit_body::{
        RendezvousAuthentication, RendezvousCandidate, RendezvousLineFamily,
        SpawnRendezvousDescriptor, RENDEZVOUS_DESCRIPTOR_PROTOCOL,
    };
    use sha2::{Digest, Sha256};

    #[test]
    fn refuses_missing_and_ambiguous_native_media_regions() {
        let mut image = vec![0; REGION_BYTES * 3];
        assert!(locate_blank_region(&image).is_err());
        let blank = blank_region();
        image[120..120 + REGION_BYTES].copy_from_slice(&blank);
        assert_eq!(locate_blank_region(&image).unwrap(), 120);
        image[REGION_BYTES * 2..].copy_from_slice(&blank);
        assert!(locate_blank_region(&image).is_err());
    }

    #[test]
    fn finite_encoded_region_cannot_be_reused_as_blank_media() {
        let mut image = vec![0; REGION_BYTES * 2];
        image[512..512 + REGION_BYTES].copy_from_slice(&blank_region());
        let region = encode_region(br#"{"schema":"example"}"#).unwrap();
        image[512..512 + REGION_BYTES].copy_from_slice(&region);
        assert!(locate_blank_region(&image).is_err());
        assert_eq!(
            encode_region(&vec![0; REGION_BYTES])
                .unwrap_err()
                .to_string(),
            "native spore provision uses 4096 bytes; finite media region permits at most 4064"
        );
    }

    #[test]
    fn completed_secret_media_is_private_and_never_replaced() {
        let root = std::env::temp_dir().join(format!(
            "conduit-native-spore-write-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let output = root.join("spore.iso");
        write_private_new(&output, b"first").unwrap();
        assert!(write_private_new(&output, b"second").is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"first");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::remove_file(output).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn route_certificate_is_selected_only_by_its_exact_owner_binding() {
        let der = [0x30, 0x03, 0x02, 0x01, 0x01];
        let binding: [u8; 32] = Sha256::digest(der).into();
        let route = SpawnRendezvousDescriptor {
            protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
            body_id: "body/one".into(),
            invitation_id: "invitation/one".into(),
            candidates: vec![RendezvousCandidate {
                candidate_id: "candidate/owner".into(),
                line_family: RendezvousLineFamily::AuthenticatedTlsStream,
                reachability: "wss://owner.example:443/conduit".into(),
                authentication: RendezvousAuthentication {
                    server_identity: "owner.example".into(),
                    transport_binding_sha256: binding,
                },
                expires_at_millis: 1_800_000_000_000,
                maximum_attempts: 1,
                attempt_timeout_millis: 2_000,
            }],
        };
        let root = std::env::temp_dir().join(format!(
            "conduit-route-cert-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("owner.pem");
        std::fs::write(
            &path,
            b"-----BEGIN CERTIFICATE-----\nMAMCAQE=\n-----END CERTIFICATE-----\n",
        )
        .unwrap();
        let selected = certificates_for_route(Some(&route), std::slice::from_ref(&path)).unwrap();
        assert_eq!(selected[0].candidate_id, "candidate/owner");
        assert_eq!(selected[0].certificate_der, der);
        assert!(certificates_for_route(Some(&route), &[]).is_err());
        assert!(certificates_for_route(None, std::slice::from_ref(&path)).is_err());
        let mut wrong = route;
        wrong.candidates[0].authentication.transport_binding_sha256 = [7; 32];
        assert!(certificates_for_route(Some(&wrong), std::slice::from_ref(&path)).is_err());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
