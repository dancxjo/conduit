//! Private custody of an already admitted native Part; never owner authority.
use super::*;
use conduit_body::SpawnInvitationSecret;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

const SCHEMA: &str = "conduit.native/part-custody@1";
const LIMIT: u64 = MAXIMUM_BODY_ADMISSION_BYTES * 2;

#[path = "part/renewal.rs"]
mod renewal;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    // A raw receipt correlated to the selected TLS invitation route, NOT a
    // Root-signed receipt. The invitation preserves the original route pins.
    pending: PendingBodyJoin,
    receipt: PortableAdmissionReceipt,
}
impl Drop for Record {
    fn drop(&mut self) {
        self.pending.invitation.secret.fill(0);
    }
}
impl Record {
    fn validate(&self, host: &str) -> Result<(), String> {
        let p = &self.pending;
        let r = &p.request;
        if self.schema != SCHEMA
            || p.schema != "conduit.body/pending-join@1"
            || r.host_advertisement.host_id.as_str() != host
            || p.invitation.claim.body_id != r.body_id
            || p.invitation.claim.invitation_id != r.invitation_id
            || p.invitation.claim.nonce != r.nonce
            || r.schema != conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA
            || self.receipt.credential.part_id.as_str().is_empty()
            || r.membership_admitted
            || r.plan_created
            || r.play_created
        {
            return Err("native Part custody identity mismatch".into());
        }
        if p.invitation.rendezvous.as_ref().is_some_and(|route| {
            route.body_id != r.body_id.as_str() || route.invitation_id != r.invitation_id.as_str()
        }) {
            return Err("native Part retained route identity mismatch".into());
        }
        self.receipt
            .validate_against(r)
            .map_err(|_| "native Part receipt mismatch")?;
        let secret = SpawnInvitationSecret::from_csprng_bytes(p.invitation.secret)
            .map_err(|_| "native Part signer missing")?;
        let transcript = p.invitation.claim.signing_transcript(
            &r.host_advertisement.host_id,
            &r.host_advertisement.boot_id,
            r.host_advertisement.offer_generation,
        );
        if r.signature.as_slice() != secret.sign(&transcript) {
            return Err("native Part signer does not match admitted request".into());
        }
        Ok(())
    }
}

fn load(path: &Path, host: &str) -> Result<Record, String> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| "native Part custody unavailable")?;
    let meta = file
        .metadata()
        .map_err(|_| "native Part custody metadata")?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o777 != 0o600
        || meta.len() == 0
        || meta.len() > LIMIT
    {
        return Err("native Part custody requires a bounded private regular file".into());
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "native Part custody read")?;
    let parsed = serde_json::from_slice::<Record>(&bytes);
    bytes.fill(0);
    let record = parsed.map_err(|_| "native Part custody encoding")?;
    record.validate(host)?;
    Ok(record)
}

pub(super) fn verify_exact(
    state: &Path,
    pending: &PendingBodyJoin,
    receipt: &PortableAdmissionReceipt,
    host: &str,
) -> Result<(), String> {
    let record = load(&state.join("body/native-part-private.json"), host)?;
    let mut expected =
        serde_json::to_vec(&(pending, receipt)).map_err(|_| "native Part encoding")?;
    let mut actual = serde_json::to_vec(&(&record.pending, &record.receipt))
        .map_err(|_| "native Part encoding")?;
    let same = expected == actual;
    expected.fill(0);
    actual.fill(0);
    if same {
        Ok(())
    } else {
        Err("conflicting native Part custody".into())
    }
}

pub(super) fn retain(
    state: &Path,
    pending: PendingBodyJoin,
    receipt: PortableAdmissionReceipt,
    host: &str,
) -> Result<(), String> {
    let record = Record {
        schema: SCHEMA.into(),
        pending,
        receipt,
    };
    record.validate(host)?;
    let path = state.join("body/native-part-private.json");
    if path.symlink_metadata().is_ok() {
        let previous = load(&path, host)?;
        // Compare private documents without exposing them in errors or output.
        let mut old = serde_json::to_vec(&previous).map_err(|_| "native Part encoding")?;
        let mut new = serde_json::to_vec(&record).map_err(|_| "native Part encoding")?;
        let same = old == new;
        old.fill(0);
        new.fill(0);
        return if same {
            Ok(())
        } else {
            Err("conflicting native Part custody".into())
        };
    }
    let staging = state.join("body").join(format!(
        "part-{}.json",
        crate::birth_identity::fresh_uuid()?
    ));
    let temporary = staging.with_extension("tmp");
    // Existing fsynced atomic helper writes this securely precreated inode.
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| "native Part private staging unavailable")?;
    let mut bytes = serde_json::to_vec(&record).map_err(|_| "native Part encoding")?;
    let written = if bytes.len() as u64 > LIMIT {
        Err("native Part custody exceeds bound".into())
    } else {
        super::super::write_bytes_atomic(&staging, &bytes)
    };
    bytes.fill(0);
    let result = written.and_then(|()| {
        fs::hard_link(&staging, &path).map_err(|_| "native Part exclusive publication failed")?;
        fs::File::open(path.parent().unwrap())
            .and_then(|d| d.sync_all())
            .map_err(|_| "native Part directory sync failed".to_string())?;
        load(&path, host).map(|_| ())
    });
    let _ = fs::remove_file(staging);
    let _ = fs::remove_file(temporary);
    result
}

/// Validated continuity signer. This is neither a current session nor Root authority.
pub(crate) struct RetainedPart {
    state: std::path::PathBuf,
    record: Record,
    advertisement: HostAdvertisement,
    current: MembershipCredential,
}
impl RetainedPart {
    #[cfg(test)]
    pub(crate) fn from_test_document(
        bytes: &[u8],
        advertisement: HostAdvertisement,
        state: &Path,
    ) -> Self {
        let record: Record = serde_json::from_slice(bytes).unwrap();
        record.validate(advertisement.host_id.as_str()).unwrap();
        let current = record.receipt.credential.clone();
        Self {
            state: state.to_path_buf(),
            current,
            record,
            advertisement,
        }
    }

    pub(crate) fn open(state: &Path, advertisement: HostAdvertisement) -> Result<Self, String> {
        let installation = read_installation(&state.join("installation.json"))?;
        let binding = installation
            .joined_body_state
            .as_ref()
            .ok_or("Host has no joined Part")?;
        super::validate(binding, &installation)?;
        if advertisement.host_id.as_str() != installation.host_id {
            return Err("Return advertisement belongs to another installed Host".into());
        }
        let runtime = crate::durable_host_control::current(state)?;
        if runtime.advertisement != advertisement {
            return Err("Return advertisement is not the current installed Boot and offers".into());
        }
        let record = load(
            &state.join("body/native-part-private.json"),
            &installation.host_id,
        )?;
        let credential = renewal::public_credential(Path::new(&binding.credential_path))?;
        let credential = renewal::reconcile(state, &record, &installation, &credential)?;
        Ok(Self {
            state: state.to_path_buf(),
            current: credential,
            record,
            advertisement,
        })
    }
    pub(crate) fn credential(&self) -> &MembershipCredential {
        &self.current
    }
    pub(crate) fn commit_return(
        &mut self,
        proof: &crate::host_rendezvous::VerifiedReturn,
    ) -> Result<(), String> {
        renewal::commit(self, proof)?;
        self.current = proof.credential().clone();
        Ok(())
    }
    pub(crate) fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }
    pub(crate) fn rendezvous(&self) -> Option<&conduit_body::SpawnRendezvousDescriptor> {
        self.record.pending.invitation.rendezvous.as_ref()
    }
    pub(crate) fn prove(
        &self,
        challenge: &conduit_body::PartReturnChallenge,
    ) -> Result<conduit_body::PartReturnProof, String> {
        let credential = self.credential();
        let ad = self.advertisement();
        if challenge.body_id != credential.body_id
            || challenge.part_id != credential.part_id
            || challenge.host_id != ad.host_id
            || challenge.boot_id != ad.boot_id
            || challenge.offer_generation != ad.offer_generation
            || challenge.nonce == [0; 32]
            || challenge.expires_at_millis <= challenge.issued_at_millis
            || challenge.expires_at_millis - challenge.issued_at_millis
                > conduit_body::MAX_ADMISSION_TTL_MILLIS
        {
            return Err("Return challenge differs from retained Part or current Boot".into());
        }
        // Recheck actual authenticated control immediately before key use.
        // A captured advertisement is not a continuing signing entitlement.
        let current = crate::durable_host_control::current(&self.state)?;
        if current.advertisement != self.advertisement {
            return Err("Return signer current Host/Boot/offers changed".into());
        }
        let secret =
            SpawnInvitationSecret::from_csprng_bytes(self.record.pending.invitation.secret)
                .map_err(|_| "native Part signer missing")?;
        Ok(conduit_body::PartReturnProof {
            admission_id: challenge.admission_id.clone(),
            body_id: challenge.body_id.clone(),
            part_id: challenge.part_id.clone(),
            host_id: challenge.host_id.clone(),
            boot_id: challenge.boot_id.clone(),
            nonce: challenge.nonce,
            signature: secret.sign(&challenge.signing_transcript()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn fixture() -> Record {
        let host = conduit_std_host::StdHost::new();
        let ad = host.advertisement().clone();
        let claim: conduit_body::SpawnInvitationClaim = serde_json::from_value(serde_json::json!({
            "invitation_id":"02".repeat(32), "body_id":"01".repeat(32),
            "nonce":vec![3;32], "expires_at_millis":300000
        }))
        .unwrap();
        let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([5; 32]).unwrap();
        let signature =
            secret.sign(&claim.signing_transcript(&ad.host_id, &ad.boot_id, ad.offer_generation));
        let request = conduit_body::PortableSpawnAdmissionRequest {
            schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: claim.invitation_id.clone(),
            body_id: claim.body_id.clone(),
            host_advertisement: ad.clone(),
            nonce: claim.nonce,
            signature: signature.to_vec(),
            membership_admitted: false,
            plan_created: false,
            play_created: false,
        };
        let credential = serde_json::from_value(serde_json::json!({
            "credential_id":"06".repeat(32), "body_id":claim.body_id,
            "part_id":"07".repeat(32), "host_id":ad.host_id,
            "boot_id":ad.boot_id, "issued_at_millis":100
        }))
        .unwrap();
        Record {
            schema: SCHEMA.into(),
            pending: PendingBodyJoin {
                schema: "conduit.body/pending-join@1".into(),
                invitation: conduit_body::PortableInvitation {
                    schema: conduit_body::INVITATION_SCHEMA.into(),
                    claim,
                    secret: [5; 32],
                    rendezvous: None,
                },
                request,
            },
            receipt: PortableAdmissionReceipt {
                schema: conduit_body::SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
                credential,
                current_offers_available: !ad.capabilities.is_empty(),
                host_advertisement: ad,
                membership_admitted: true,
                plan_created: false,
                play_created: false,
            },
        }
    }
    #[test]
    fn native_part_custody_reopens_and_refuses_changed_signer() {
        let root = std::env::temp_dir().join(format!(
            "native-part-{}",
            crate::birth_identity::fresh_uuid().unwrap()
        ));
        fs::create_dir_all(root.join("body")).unwrap();
        super::super::restrict_directory(&root.join("body")).unwrap();
        let record = fixture();
        let host = record.receipt.credential.host_id.as_str().to_owned();
        let expected = record.receipt.credential.clone();
        let pending: PendingBodyJoin =
            serde_json::from_value(serde_json::to_value(&record.pending).unwrap()).unwrap();
        retain(&root, pending, record.receipt.clone(), &host).unwrap();
        let reopened = load(&root.join("body/native-part-private.json"), &host).unwrap();
        assert_eq!(reopened.receipt.credential, expected);
        let exact: PendingBodyJoin =
            serde_json::from_value(serde_json::to_value(&record.pending).unwrap()).unwrap();
        retain(&root, exact, record.receipt.clone(), &host).unwrap();
        let conflict: PendingBodyJoin =
            serde_json::from_value(serde_json::to_value(&record.pending).unwrap()).unwrap();
        let mut other_receipt = record.receipt.clone();
        other_receipt.credential.part_id =
            serde_json::from_value(serde_json::json!("08".repeat(32))).unwrap();
        assert!(retain(&root, conflict, other_receipt, &host).is_err());
        // The public credential remains ordinary credential material only.
        let public = serde_json::to_value(&record.receipt.credential).unwrap();
        assert!(public.get("secret").is_none());
        assert!(public.get("invitation").is_none());
        assert!(load(&root.join("body/native-part-private.json"), "foreign-host").is_err());
        let mut wrong: PendingBodyJoin =
            serde_json::from_value(serde_json::to_value(&record.pending).unwrap()).unwrap();
        wrong.invitation.secret = [8; 32];
        assert!(retain(&root, wrong, record.receipt.clone(), &host).is_err());
        assert_eq!(
            load(&root.join("body/native-part-private.json"), &host)
                .unwrap()
                .receipt
                .credential,
            expected
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_part_custody_write_failure_preserves_pending() {
        let root = std::env::temp_dir().join(format!(
            "native-part-{}",
            crate::birth_identity::fresh_uuid().unwrap()
        ));
        fs::create_dir_all(root.join("body")).unwrap();
        let record = fixture();
        let host = record.receipt.credential.host_id.as_str().to_owned();
        let pending_bytes = serde_json::to_vec(&record.pending).unwrap();
        fs::write(root.join("body/pending-join.json"), &pending_bytes).unwrap();
        fs::create_dir(root.join("body/native-part-private.json")).unwrap();
        let pending = serde_json::from_slice(&pending_bytes).unwrap();
        assert!(retain(&root, pending, record.receipt.clone(), &host).is_err());
        assert_eq!(
            fs::read(root.join("body/pending-join.json")).unwrap(),
            pending_bytes
        );
        fs::remove_dir_all(root).unwrap();
    }
}
