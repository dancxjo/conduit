//! Actual current-Boot receipt installation. This does not create a Plan or Play.
#[path = "membership_execution.rs"]
mod execution;
#[path = "membership_mixed.rs"]
mod mixed;
#[path = "membership_preparation.rs"]
mod preparation;
#[path = "membership_return.rs"]
mod returning;
use crate::{
    seed::Seed,
    transport::{Link, LIMIT},
};
use alloc::{format, string::String, vec::Vec};
use conduit_body::PICO_SPAWN_REQUEST;
use conduit_core::{BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration};
use core::{cell::Cell, fmt::Write};
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};
static RECEIPT_DIGEST: critical_section::Mutex<Cell<Option<[u8; 32]>>> =
    critical_section::Mutex::new(Cell::new(None));
#[cfg(any(sensor_dht11, neutral_source_execution))]
pub(crate) fn observe_receipt(digest:Option<[u8;32]>) {
    critical_section::with(|cs| RECEIPT_DIGEST.borrow(cs).set(digest));
}
pub fn is_member() -> bool {
    critical_section::with(|cs| RECEIPT_DIGEST.borrow(cs).get().is_some())
}

pub fn append_status<const N: usize>(out: &mut heapless::String<N>) -> Result<(), ()> {
    if let Some(digest) = critical_section::with(|cs| RECEIPT_DIGEST.borrow(cs).get()) {
        out.push_str(",\"membership_receipt_sha256\":\"")
            .map_err(|_| ())?;
        for byte in digest {
            write!(out, "{byte:02x}").map_err(|_| ())?;
        }
        out.push('"').map_err(|_| ())?;
    }
    Ok(())
}

pub struct Membership {
    state: MembershipState,
    input: conduit_body::HostCommandInput<LIMIT>,
    execution_attempted: bool,
}
impl Membership {
    pub fn new(seed: Seed<'static>, host_id: String, boot_nonce: [u64; 2]) -> Self {
        Self {
            state: MembershipState::new(seed, host_id, boot_nonce),
            input: conduit_body::HostCommandInput::new(),
            execution_attempted: false,
        }
    }
    pub async fn run(&mut self, link: &mut Link) -> ! {
        loop {
            if !self.input.is_retained() {
                self.execution_attempted = false;
            }
            if self.input.is_retained() && !self.execution_attempted {
                if let Some(policy) = crate::config::EXECUTION_DELEGATION.as_ref() {
                    self.execution_attempted = true;
                    // Once admission succeeds, this supervisor outlives every
                    // connection. A lost delivery ACK cannot cancel its work.
                    let _ = self.state.run_retained(link, &self.input, policy).await;
                }
            }
            link.connected().await;
            if self.state.serve_once(link, &mut self.input).await.is_err() {
                link.reset();
            }
            embassy_time::Timer::after_millis(10).await;
        }
    }
}

struct MembershipState {
    seed: Seed<'static>,
    // This existing Core owner retains the actual local pool and reservations.
    // The compact advertisement contains no decoded capability catalog.
    resources: conduit_core::ResourceAdmissionOwner,
    key: SigningKey,
    freshness: u64,
    native_offer: Option<&'static [u8]>,
    // Boot-scoped issuance attempt custody outlives every carrier and Play.
    native_issuer: crate::i2c_issuance::Issuer,
    receipt: Option<conduit_body::MembershipCredential>,
}
impl MembershipState {
    pub fn new(seed: Seed<'static>, host_id: String, boot_nonce: [u64; 2]) -> Self {
        let key = SigningKey::from_bytes(seed.admission.secret());
        let ad = HostAdvertisement {
            declared_name: None,
            protocol_version: 1,
            host_id: HostId::from(host_id),
            boot_id: BootId::from(format!(
                "rp2040/boot/{:016x}{:016x}",
                boot_nonce[0], boot_nonce[1]
            )),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from(crate::config::CONFIGURATION_ID),
            bases: Vec::new(),
            resources: alloc::vec![conduit_core::resource_offer(
                crate::config::RESIDENT_WORK_POOL,
                conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID,
                crate::config::RESIDENT_WORK_UNITS,
            )],
            capabilities: Vec::new(),
            planner_capabilities: Vec::new(),
        };
        Self {
            seed,
            resources: conduit_core::ResourceAdmissionOwner::new(ad),
            key,
            freshness: 0,
            native_offer: None,
            native_issuer: crate::i2c_issuance::Issuer::new(),
            receipt: None,
        }
    }
    async fn serve_once(
        &mut self,
        link: &mut Link,
        workspace: &mut conduit_body::HostCommandInput<LIMIT>,
    ) -> Result<(), ()> {
        self.reclaim_expired_preparation(workspace);
        if workspace.is_retained() {
            return preparation::serve_retained_input(link, workspace).await;
        }
        let request = link.receive(workspace.writable().map_err(|_| ())?).await?;
        if request == conduit_body::HOST_PREPARATION_REQUEST {
            return self.serve_preparation(link, workspace).await;
        }
        if request == conduit_body::PICO_MEMBER_RETURN_IMAGES_REQUEST {
            return self
                .serve_return(link, workspace.writable().map_err(|_| ())?)
                .await;
        }
        if request != PICO_SPAWN_REQUEST {
            return Err(());
        }
        // A retained credential must return through canonical member return;
        // never silently admit the same persistent subject as a new Part.
        if self.seed.admission.credential().is_some() || self.receipt.is_some() {
            return Err(());
        }
        self.freshness = self.freshness.checked_add(1).ok_or(())?;
        let input = workspace.writable().map_err(|_| ())?;
        let mut output = [0; LIMIT];
        let accepted = conduit_mcu_host_runtime::membership::SpawnAdmission {
            bootstrap: &self.seed,
            actual: self.resources.host(),
            image_id: crate::config::IMAGE_ID,
            key: &self.key,
            current: self.receipt.as_ref(),
            freshness: self.freshness,
            friendly_label: "RP2040 Host",
        }
        .exchange(link, input, &mut output)
        .await
        .map_err(|_| ())?;
        self.receipt = Some(accepted.credential);
        critical_section::with(|cs| {
            RECEIPT_DIGEST.borrow(cs).set(Some(accepted.receipt_digest));
        });
        Ok(())
    }
}
