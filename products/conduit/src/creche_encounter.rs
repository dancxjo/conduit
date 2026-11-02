//! One admitted Crèche operation on the original installed Host. The service
//! owns presentation and pumps production; kernel begin/poll never perform I/O.
use conduit_body::{BodyBiographyEvidence, PortableInvitation};
use conduit_body_make::{
    deployment_runtime::{CrecheDisposition, CrecheOutcome, CrecheRequest},
    HostConfigurationEncounter, HostConfigurationEncounterBasis, HostConfigurationEncounterOutcome,
    JoiningProductionBounds, ProducedJoiningHost,
};
use conduit_core::{ResourceOffer, ResourcePoolId};
use conduit_face::{Face, FaceBasis, FaceInteraction, MaskShow};
use conduit_host_make::{
    CheckedHostConfiguration, HostImage, MakeCatalog, MakePackageSet, SporeOutputKind,
};
use conduit_std_host::hosted_creche::{
    CrecheExecutionContext, CrecheLimits, CrechePoll, HostedCrecheAdapter, PreparedCrecheRequest,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    sync::{Arc, Mutex, TryLockError},
    thread::JoinHandle,
};

mod controller;
mod factory;
mod hosted;
pub use controller::CrecheEncounterController;

/// Supplied by the service's actual selected Mask route, never a structural ACK
/// or a default renderer. Calls except retirement must be bounded/nonblocking.
pub trait CrecheMaskShowCustody: Send {
    fn validate_acknowledged(&mut self, face: &Face, show: &MaskShow) -> Result<(), String>;
    fn validate_returned_interaction(
        &mut self,
        face: &Face,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<(), String>;
    fn retire_and_drain(&mut self) -> Result<(), String>;
}

pub struct CrecheEncounterInstallation {
    pub encounter_id: String,
    pub resource_pool: ResourcePoolId,
    pub limits: CrecheLimits,
    pub production_bounds: JoiningProductionBounds,
    pub initial_source: Option<String>,
}

/// Prior ordinary Make artifact plus its explicit owner-reviewed identity.
/// No path, shell, source checkout, toolchain or device authority is inferred.
pub struct CrecheReviewedArtifact {
    pub configuration: CheckedHostConfiguration,
    pub output: SporeOutputKind,
    pub image: HostImage,
    pub image_manifest: Vec<u8>,
    pub artifact: File,
    pub artifact_digest: String,
    pub artifact_bytes: u64,
    pub source_identity: String,
}

/// Private owner-issued provisioning stays in scoped custody, never in a Face.
pub struct CrecheBodyBinding {
    pub biography: BodyBiographyEvidence,
    pub invitation: PortableInvitation,
}

impl Drop for CrecheBodyBinding {
    fn drop(&mut self) {
        self.invitation.secret.fill(0);
    }
}

struct Inputs {
    catalog: MakeCatalog,
    packages: MakePackageSet,
    installation: CrecheEncounterInstallation,
    artifacts: Vec<CrecheReviewedArtifact>,
    resource: ResourceOffer,
    admission_digest: [u8; 32],
}

/// Retained and validated before its exact resource is advertised. Creating a
/// per-Play adapter borrows this factory, so no second concurrent encounter can
/// spend the same single slot through this owner.
pub struct CrecheEncounterFactory {
    inputs: Arc<Inputs>,
}

struct Command {
    show: MaskShow,
    input: FaceInteraction,
}
struct State {
    active: bool,
    retired: bool,
    face: Option<Arc<Face>>,
    acknowledged: Option<MaskShow>,
    command: Option<Command>,
    checked: Option<CheckedHostConfiguration>,
    request: Option<CrecheRequest>,
    started: bool,
    worker: Option<JoinHandle<()>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    result: Option<Result<ProducedJoiningHost<File>, String>>,
    products: Option<ProducedJoiningHost<File>>,
    published: bool,
}
struct Channel {
    state: Mutex<State>,
    custody: Mutex<Box<dyn CrecheMaskShowCustody>>,
    inputs: Arc<Inputs>,
    binding: CrecheBodyBinding,
}
impl Channel {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>, String> {
        self.state
            .lock()
            .map_err(|_| "Crèche mailbox poisoned".into())
    }
}

pub struct CrecheEncounterAdapter<'a> {
    encounter: HostConfigurationEncounter<'a>,
    channel: Arc<Channel>,
    basis: Option<HostConfigurationEncounterBasis>,
    request: Option<CrecheRequest>,
    evidence: Vec<u8>,
    outcome: Option<CrecheOutcome>,
    begun: bool,
    retired: bool,
}

fn clock() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "Crèche wall clock unavailable".to_string())?
        .as_millis()
        .try_into()
        .map_err(|_| "Crèche wall clock exhausted".into())
}

/// Internal admission fingerprint, not a portable catalogue or authority
/// format. Stream the retained immutable metadata without cloning its tree.
fn catalogue_digest(catalog: &MakeCatalog, packages: &MakePackageSet) -> [u8; 32] {
    struct Writer(Sha256);
    impl std::fmt::Write for Writer {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            self.0.update(value.as_bytes());
            Ok(())
        }
    }
    let mut writer = Writer(Sha256::new());
    std::fmt::write(&mut writer, format_args!("{catalog:?}\n{packages:?}"))
        .expect("digest writer cannot fail");
    writer.0.finalize().into()
}

#[cfg(test)]
mod tests;
