//! Actual local first-host installation and challenge admission of the MCU.
use crate::{checked, transport::Line, Result};
use conduit_body as b;
use conduit_core as c;
use std::{
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Instant,
};
pub fn provision(state: &Path) -> Result<()> {
    std::fs::create_dir_all(state)?;
    std::fs::set_permissions(state, std::fs::Permissions::from_mode(0o700))?;
    let mut secret = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(state.join("owner-secret.bin"))?;
    let mut seed = random()?;
    secret.write_all(&seed)?;
    secret.sync_all()?;
    let public = ed25519_dalek::SigningKey::from_bytes(&seed)
        .verifying_key()
        .to_bytes();
    seed.fill(0);
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(state.join("owner-public.bin"))?;
    output.write_all(&public)?;
    output.sync_all()?;
    println!(
        "Owner public key file: {}",
        state.join("owner-public.bin").display()
    );
    Ok(())
}
pub struct Authority {
    pub host: c::HostAdvertisement,
    pub state: PathBuf,
    pub key: ed25519_dalek::SigningKey,
    clock: Instant,
}
pub fn random() -> Result<[u8; 32]> {
    let mut bytes = [0; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes)
}
impl Authority {
    pub fn new(state: &Path) -> Result<Self> {
        std::fs::create_dir_all(state)?;
        std::fs::set_permissions(state, std::fs::Permissions::from_mode(0o700))?;
        let seed: [u8; 32] = std::fs::read(state.join("owner-secret.bin"))?
            .try_into()
            .map_err(|_| "owner key length")?;
        let host_id = format!(
            "linux/{}",
            std::fs::read_to_string("/etc/machine-id")?.trim()
        );
        let incarnation = std::fs::read_to_string("/proc/sys/kernel/random/uuid")?;
        let boot_id = format!("boot/{}", incarnation.trim());
        let state = state.join("runs").join(incarnation.trim());
        std::fs::create_dir_all(&state)?;
        std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700))?;
        Ok(Self {
            host: c::HostAdvertisement {
                protocol_version: c::PROTOCOL_VERSION,
                host_id: host_id.into(),
                boot_id: boot_id.into(),
                offer_generation: c::OfferGeneration(1),
                profile: "conduit/environment/linux-body-owner@1".into(),
                bases: vec![],
                resources: vec![],
                capabilities: vec![],
                planner_capabilities: vec![],
            },
            state,
            key: ed25519_dalek::SigningKey::from_bytes(&seed),
            clock: Instant::now(),
        })
    }
    pub fn now(&self) -> u64 {
        self.clock.elapsed().as_millis() as u64
    }
    pub fn sign(&self, sequence: u64) -> c::SignId {
        c::bind_sign(&self.host.host_id, &self.host.boot_id, None, sequence).sign_id
    }
    pub fn save(&self, body: &b::BodyLifecycleSession, file: &str) -> Result<()> {
        let path = self.state.join(file);
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        serde_json::to_writer_pretty(&mut out, body.evidence())?;
        out.sync_all()?;
        Ok(())
    }
    /// The caller has stopped the previous owner and retained signed quiescence.
    /// Preserve biography and Body identity; never revive a prior active Play.
    pub fn resume(&self, path: &Path) -> Result<b::BodyLifecycleSession> {
        let evidence: b::BodyBiographyEvidence = serde_json::from_slice(&std::fs::read(path)?)?;
        if evidence.body.state != b::BodyState::Lulled {
            return Err("resume requires retained Body lull evidence".into());
        }
        let body = checked(b::BodyLifecycleSession::resume_here(
            evidence,
            &self.host.host_id,
            &self.host.boot_id,
        ))?;
        if !body.pending_archives().is_empty() {
            return Err("resume requires explicit archive custody".into());
        }
        self.save(&body, "body-resumed.json")?;
        Ok(body)
    }
    pub fn birth(
        &self,
        source: &conduit_environment_runtime::construction::ApplianceSource,
        display: Option<&conduit_environment_make::DisplayOutputSource>,
    ) -> Result<b::BodyLifecycleSession> {
        let uuid = std::fs::read_to_string("/proc/sys/kernel/random/uuid")?;
        let plot = b::ResidentPlot::new(
            source.expanded.source_document_id.clone(),
            source.expanded.checked_plot_id.clone(),
        );
        let mut choices = vec![conduit_birth_plot::BirthPlotChoice {
            title: "Environmental sampling".into(),
            search_text: "temperature humidity".into(),
            plot,
            selected: true,
            refusal: None,
        }];
        if let Some(source) = display {
            choices.push(conduit_birth_plot::BirthPlotChoice {
                title: "Finite native display checkpoint".into(),
                search_text: "TM1637 output".into(),
                plot: crate::display_check::resident(source),
                selected: true,
                refusal: None,
            });
        }
        let image = checked(conduit_environment_make::PreparedBodyImage::prepare(
            uuid.trim().into(),
            choices,
            1,
            self.sign(1),
        ))?;
        let body = checked(
            conduit_environment_runtime::body_image::open_installed_image(
                image.into_evidence(),
                &self.host,
            ),
        )?;
        self.save(&body, "body-born.json")?;
        Ok(body)
    }
    pub fn admit(
        &self,
        body: &mut b::BodyLifecycleSession,
        line: &mut Line,
        port: &str,
    ) -> Result<(c::HostAdvertisement, [u8; 32])> {
        line.text("CONDUIT_BODY_ADVERTISE_V1")?;
        let document = line.document()?;
        let arrival: b::PicoAdmissionAdvertisement = serde_json::from_value(document.clone())?;
        if arrival.protocol != b::PICO_ADMISSION_PROTOCOL {
            return Err("admission protocol".into());
        }
        let key: [u8; 32] = arrival
            .verifying_key
            .try_into()
            .map_err(|_| "admission key length")?;
        let mut candidates = checked(b::CandidateInventory::new(body.evidence().body_id.clone()))?;
        let candidate = checked(candidates.observe(b::CandidateObservation {
            advertisement: arrival.advertisement.clone(),
            friendly_label: arrival.friendly_label,
            observed_binding_id: format!("carrier/{port}").into(),
            observation_sign_id: self.sign(20),
            proof_id: checked(b::DiscoveryProofId::bind(&format!("carrier/{port}")))?,
            freshness_sequence: arrival.freshness_sequence,
            encoded_bytes: serde_json::to_vec(&document)?.len() as u32,
        }))?;
        let mut admissions = checked(b::AdmissionManager::new(body.evidence().body_id.clone()))?;
        let now = self.now();
        let challenge = checked(admissions.begin_ambient(
            &mut candidates,
            &candidate,
            key,
            random()?,
            now,
            now + 10_000,
            self.sign(21),
        ))?;
        line.text(&serde_json::to_string(&b::PicoAdmissionChallenge {
            protocol: b::PICO_ADMISSION_PROTOCOL,
            admission_id: challenge.admission_id.as_str(),
            body_id: challenge.body_id.as_str(),
            host_id: challenge.host_id.as_str(),
            boot_id: challenge.boot_id.as_str(),
            offer_generation: challenge.offer_generation.0,
            nonce: challenge.nonce,
            issued_at_millis: challenge.issued_at_millis,
            expires_at_millis: challenge.expires_at_millis,
        })?)?;
        let reply = line.document()?;
        if reply["protocol"] != b::PICO_ADMISSION_PROTOCOL {
            return Err("admission proof protocol".into());
        }
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.state.join("mcu-admission.json"))?;
        serde_json::to_writer(
            &file,
            &serde_json::json!({"advertisement":document,"challenge":{
            "admission_id":challenge.admission_id,"body_id":challenge.body_id,"host_id":challenge.host_id,"boot_id":challenge.boot_id,
            "offer_generation":challenge.offer_generation,"nonce":challenge.nonce,"issued_at_millis":challenge.issued_at_millis,"expires_at_millis":challenge.expires_at_millis,
        },"reply":reply}),
        )?;
        file.sync_all()?;
        // Use the received identities, not expected values substituted over them.
        let proof = b::AmbientAdmissionProof {
            admission_id: serde_json::from_value(reply["admission_id"].clone())?,
            body_id: serde_json::from_value(reply["body_id"].clone())?,
            host_id: serde_json::from_value(reply["host_id"].clone())?,
            boot_id: serde_json::from_value(reply["boot_id"].clone())?,
            nonce: serde_json::from_value(reply["nonce"].clone())?,
            signature: serde_json::from_value::<Vec<u8>>(reply["signature"].clone())?
                .try_into()
                .map_err(|_| "proof signature length")?,
        };
        checked(body.admit_ambient_host(
            &mut admissions,
            &mut candidates,
            &proof,
            self.now(),
            &self.host.host_id,
            &self.host.boot_id,
        ))?;
        self.save(body, "body-admitted.json")?;
        println!(
            "{}",
            serde_json::json!({"event":"mcu-host-admitted","body":body.evidence().body_id,"first_host":self.host.host_id,"member":arrival.advertisement.host_id})
        );
        Ok((arrival.advertisement, key))
    }
}

#[cfg(test)]
#[path = "continuity_tests.rs"]
mod continuity_tests;
