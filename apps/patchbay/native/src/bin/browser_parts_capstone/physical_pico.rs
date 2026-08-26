//! Optional physical Pico admission into the browser capstone's one canonical Body.

use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use conduit_body::{
    AdmissionManager, AdmissionSigns, Body, BodyMembership, CandidateInventory, DiscoveryProofId,
};
use conduit_core::{LinkBindingId, SignId};
use conduit_std_host::pico_admission::{PicoAdmissionArrival, PicoAdmissionSocket};

const IO_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) struct AdmittedPhysicalPico {
    _socket: PicoAdmissionSocket,
}

pub(super) struct PendingPhysicalPico {
    pub(super) advertisement: conduit_core::HostAdvertisement,
    arrival: PicoAdmissionArrival,
}

pub(super) fn observe(path: &str) -> Result<PendingPhysicalPico, String> {
    let socket = PicoAdmissionSocket::open(path).map_err(debug("open physical Pico Line"))?;
    let arrival = socket
        .observe(
            LinkBindingId::from(format!("body-capstone/pico/usb-cdc/{path}")),
            SignId::from("body-capstone/pico-observed"),
            DiscoveryProofId::bind("body-capstone/physical-pico-observation")
                .map_err(debug("Pico discovery proof"))?,
            IO_TIMEOUT,
        )
        .map_err(debug("observe physical Pico"))?;
    Ok(PendingPhysicalPico {
        advertisement: arrival.observation.advertisement.clone(),
        arrival,
    })
}

impl PendingPhysicalPico {
    pub(super) fn admit(
        self,
        body: &Body,
        candidates: &mut CandidateInventory,
        membership: &mut BodyMembership,
        manager: &mut AdmissionManager,
    ) -> Result<AdmittedPhysicalPico, String> {
        let advertisement = self.advertisement;
        let arrival = self.arrival;
        let candidate_id = candidates
            .observe(arrival.observation)
            .map_err(debug("record inert Pico candidate"))?;
        println!(
            "pico_wants_to_join={} members_before_pico_admit={} host={} boot={}",
            candidate_id.as_str(),
            membership.parts.len(),
            advertisement.host_id.as_str(),
            advertisement.boot_id.as_str()
        );
        let mut nonce = [0_u8; 32];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut nonce))
            .map_err(|error| format!("read Pico admission nonce: {error}"))?;
        let now = now_millis()?;
        let challenge = manager
            .begin_ambient(
                candidates,
                &candidate_id,
                arrival.verifying_key,
                nonce,
                now,
                now.checked_add(60_000)
                    .ok_or("Pico challenge expiry overflow")?,
                SignId::from("body-capstone/pico-admission-requested"),
            )
            .map_err(debug("begin explicit Pico admission"))?;
        nonce.fill(0);
        let (proof, socket) = arrival
            .socket
            .prove(&challenge, IO_TIMEOUT)
            .map_err(debug("receive physical Pico proof"))?;
        let credential = manager
            .complete_ambient(
                candidates,
                membership,
                &proof,
                now_millis()?,
                AdmissionSigns {
                    part_admitted: SignId::from("body-capstone/pico-part-admitted"),
                    host_attached: SignId::from("body-capstone/pico-host-attached"),
                    candidate_admitted: SignId::from("body-capstone/pico-candidate-admitted"),
                },
            )
            .map_err(debug("complete explicit Pico admission"))?;
        if credential.body_id != body.body_id || credential.host_id != advertisement.host_id {
            return Err(
                "physical Pico admission escaped the capstone Body or Host identity".into(),
            );
        }
        println!(
            "pico_admitted part={} host={} boot={} capabilities={}",
            credential.part_id.as_str(),
            credential.host_id.as_str(),
            credential.boot_id.as_str(),
            advertisement.capabilities.len()
        );
        Ok(AdmittedPhysicalPico { _socket: socket })
    }
}

fn now_millis() -> Result<u64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock before Unix epoch: {error}"))?;
    u64::try_from(duration.as_millis()).map_err(|_| "millisecond clock overflow".into())
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}
