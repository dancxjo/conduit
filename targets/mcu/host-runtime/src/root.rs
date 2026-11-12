//! Small shared MCU Root control custody; native execution remains separately admitted.
use crate::{
    member_return::RetainedReturn,
    membership::SpawnAdmission,
    seed::Seed,
    transport::{DocumentTransport, MessageTransport},
};
use conduit_body::{
    HostCommandInput, HostExecutionDelegation, HostPreparationDelegation, MembershipCredential,
    NativePreparationContext,
};
use conduit_core::{AuthorityGrant, HostAdvertisement, ResourceAdmissionOwner};
use ed25519_dalek::SigningKey;
extern crate alloc;
use alloc::vec::Vec;

pub struct Configuration {
    pub image_id: &'static str,
    pub configuration_id: &'static str,
    pub friendly_label: &'static str,
    pub preparation: Option<&'static HostPreparationDelegation>,
    pub execution: Option<&'static HostExecutionDelegation>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Scope,
    NotMember,
    RetainedInput,
    Freshness,
    Carrier,
    Request,
    Membership,
    MemberReturn(crate::member_return::Refusal),
    Ingress(crate::ingress::Refusal),
    Preparation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Inspect,
    Spawn,
    Return,
    Prepare,
    #[cfg(feature = "execution")]
    InstallContinuation,
}
/// Select a signed continuation artifact through the existing framed channel.
pub const CONTINUATION_INSTALL_REQUEST: &[u8] = b"conduit.host/install-continuation@1";

pub struct Root<'s, const N: usize> {
    pub(crate) seed: Seed<'s>,
    pub(crate) resources: ResourceAdmissionOwner,
    pub(crate) key: SigningKey,
    pub(crate) receipt: Option<MembershipCredential>,
    pub(crate) input: HostCommandInput<N>,
    pub(crate) freshness: u64,
    pub(crate) config: Configuration,
    receipt_digest: Option<[u8; 32]>,
    #[cfg(feature = "execution")]
    continuation_attempted: bool,
}
/// Caller-funded Root storage. In-place initialization avoids temporarily copying
/// the fixed command arena onto the MCU stack. Drop preserves ordinary custody.
pub struct RootStorage<'s, const N: usize> {
    value: core::mem::MaybeUninit<Root<'s, N>>,
    initialized: bool,
}
impl<'s, const N: usize> RootStorage<'s, N> {
    pub const fn new() -> Self {
        Self {
            value: core::mem::MaybeUninit::uninit(),
            initialized: false,
        }
    }
    pub fn initialize(
        &mut self,
        seed: Seed<'s>,
        actual: HostAdvertisement,
        config: Configuration,
    ) -> Result<&mut Root<'s, N>, Refusal> {
        if self.initialized
            || actual.protocol_version != conduit_core::PROTOCOL_VERSION
            || actual.offer_generation.0 == 0
            || actual.profile.as_str() != config.configuration_id
            || !seed.matches_native(
                actual.host_id.as_str(),
                config.image_id,
                config.configuration_id,
            )
        {
            return Err(Refusal::Scope);
        }
        let key = SigningKey::from_bytes(seed.admission.secret());
        let resources = ResourceAdmissionOwner::new(actual);
        let target = self.value.as_mut_ptr();
        // SAFETY: original uninitialized owned storage; every field is written
        // exactly once before publication, and Drop only observes complete state.
        unsafe {
            core::ptr::addr_of_mut!((*target).seed).write(seed);
            core::ptr::addr_of_mut!((*target).resources).write(resources);
            core::ptr::addr_of_mut!((*target).key).write(key);
            core::ptr::addr_of_mut!((*target).receipt).write(None);
            let input = &mut *core::ptr::addr_of_mut!((*target).input)
                .cast::<core::mem::MaybeUninit<HostCommandInput<N>>>();
            HostCommandInput::initialize_in_place(input);
            core::ptr::addr_of_mut!((*target).freshness).write(0);
            core::ptr::addr_of_mut!((*target).config).write(config);
            core::ptr::addr_of_mut!((*target).receipt_digest).write(None);
            #[cfg(feature = "execution")]
            core::ptr::addr_of_mut!((*target).continuation_attempted).write(false);
            self.initialized = true;
            Ok(self.value.assume_init_mut())
        }
    }
}
impl<const N: usize> Default for RootStorage<'_, N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const N: usize> Drop for RootStorage<'_, N> {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: initialized was set only after every field was published.
            unsafe { self.value.assume_init_drop() };
        }
    }
}

/// Disjoint borrowed original custody, not copies or serializable grants.
/// The native execution bridge must prepare the complete workset and atomically
/// admit it against this exact ledger before binding providers or starting.
pub struct ExecutionParts<'a, 's, const N: usize> {
    pub bootstrap: &'a conduit_body::MemberBootstrap<'s>,
    pub member: &'a MembershipCredential,
    pub resources: &'a mut ResourceAdmissionOwner,
    pub workspace: &'a mut HostCommandInput<N>,
    pub configuration: &'a Configuration,
}
impl<'s, const N: usize> Root<'s, N> {
    pub fn new(
        seed: Seed<'s>,
        actual: HostAdvertisement,
        config: Configuration,
    ) -> Result<Self, Refusal> {
        if actual.protocol_version != conduit_core::PROTOCOL_VERSION
            || actual.offer_generation.0 == 0
            || actual.profile.as_str() != config.configuration_id
            || !seed.matches_native(
                actual.host_id.as_str(),
                config.image_id,
                config.configuration_id,
            )
        {
            return Err(Refusal::Scope);
        }
        let key = SigningKey::from_bytes(seed.admission.secret());
        Ok(Self {
            seed,
            resources: ResourceAdmissionOwner::new(actual),
            key,
            receipt: None,
            input: HostCommandInput::new(),
            freshness: 0,
            config,
            receipt_digest: None,
            #[cfg(feature = "execution")]
            continuation_attempted: false,
        })
    }
    pub fn actual(&self) -> &HostAdvertisement {
        self.resources.host()
    }
    pub fn receipt_digest(&self) -> Option<[u8; 32]> {
        self.receipt_digest
    }
    /// Call after a cancelled/timed-out receive. Never clears retained custody.
    pub fn clear_unretained_input(&mut self) {
        if let Ok(bytes) = self.input.writable() {
            bytes.fill(0);
        }
    }
    pub fn has_current_member(&self) -> bool {
        self.receipt.is_some()
    }
    pub fn retained_input(&self) -> bool {
        self.input.is_retained()
    }
    pub fn configuration(&self) -> &Configuration {
        &self.config
    }
    /// Read-only return evidence may name the actual retained prior-Boot
    /// credential. It is not current membership for issuance, admission or Start.
    pub fn return_context(&self, now_millis: u64) -> Result<NativePreparationContext<'_>, Refusal> {
        let member = self
            .receipt
            .as_ref()
            .or_else(|| self.seed.admission.credential())
            .ok_or(Refusal::NotMember)?;
        Ok(NativePreparationContext {
            bootstrap: &self.seed,
            member,
            actual: self.resources.host(),
            image_id: self.config.image_id,
            configuration_id: self.config.configuration_id,
            now_millis,
        })
    }
    pub fn execution_parts(&mut self) -> Result<ExecutionParts<'_, 's, N>, Refusal> {
        let member = self.receipt.as_ref().ok_or(Refusal::NotMember)?;
        Ok(ExecutionParts {
            bootstrap: &self.seed,
            member,
            resources: &mut self.resources,
            workspace: &mut self.input,
            configuration: &self.config,
        })
    }
    pub async fn receive_request<T: MessageTransport>(
        &mut self,
        link: &mut T,
    ) -> Result<Request, Refusal> {
        let bytes = link
            .receive(self.input.writable().map_err(|_| Refusal::RetainedInput)?)
            .await
            .map_err(|_| Refusal::Carrier)?;
        if bytes == b"?" {
            Ok(Request::Inspect)
        } else if bytes == conduit_body::PICO_SPAWN_REQUEST {
            Ok(Request::Spawn)
        } else if bytes == conduit_body::PICO_MEMBER_RETURN_IMAGES_REQUEST {
            Ok(Request::Return)
        } else if bytes == conduit_body::HOST_PREPARATION_REQUEST {
            Ok(Request::Prepare)
        } else {
            #[cfg(feature = "execution")]
            if bytes == CONTINUATION_INSTALL_REQUEST {
                if self.config.execution.and_then(|p| p.continuation).is_none() {
                    return Err(Refusal::Preparation);
                }
                return Ok(Request::InstallContinuation);
            }
            Err(Refusal::Request)
        }
    }
    pub async fn spawn<T: MessageTransport>(
        &mut self,
        link: &mut T,
        output: &mut [u8],
    ) -> Result<(), Refusal> {
        self.freshness = self.freshness.checked_add(1).ok_or(Refusal::Freshness)?;
        let accepted = SpawnAdmission {
            bootstrap: &self.seed,
            actual: self.resources.host(),
            image_id: self.config.image_id,
            key: &self.key,
            current: self.receipt.as_ref(),
            freshness: self.freshness,
            friendly_label: self.config.friendly_label,
        }
        .exchange(
            link,
            self.input.writable().map_err(|_| Refusal::RetainedInput)?,
            output,
        )
        .await
        .map_err(|_| Refusal::Membership)?;
        self.receipt = Some(accepted.credential);
        self.receipt_digest = Some(accepted.receipt_digest);
        Ok(())
    }
    pub async fn return_member<'i, T, I, G>(
        &mut self,
        link: &mut T,
        images: I,
        grants: G,
        output: &mut [u8],
    ) -> Result<(), Refusal>
    where
        T: DocumentTransport,
        I: Iterator<Item = &'i [u8]> + Clone,
        G: FnOnce() -> Result<Vec<AuthorityGrant>, crate::member_return::Refusal>,
    {
        if !self.resources.admissions().is_empty() || !self.resources.assignments().is_empty() {
            return Err(Refusal::Scope);
        }
        let retained = self
            .receipt
            .as_ref()
            .or_else(|| self.seed.admission.credential())
            .ok_or(Refusal::NotMember)?;
        let accepted = RetainedReturn {
            bootstrap: &self.seed,
            actual: self.resources.host(),
            image_id: self.config.image_id,
            key: &self.key,
            retained,
        }
        .exchange(
            link,
            images,
            grants,
            self.input.writable().map_err(|_| Refusal::RetainedInput)?,
            output,
        )
        .await
        .map_err(Refusal::MemberReturn)?;
        self.receipt = Some(accepted.credential);
        self.receipt_digest = Some(accepted.receipt_digest);
        Ok(())
    }
    /// The platform wraps this one future in the existing 10s deadline. Nonce
    /// entropy is supplied by its real RNG; the retained Boot counter adds replay
    /// separation across every interrupted receive attempt.
    pub async fn receive_preparation<T: DocumentTransport>(
        &mut self,
        link: &mut T,
        mut nonce: [u8; 32],
        now: impl Fn() -> u64,
    ) -> Result<(), Refusal> {
        if self.input.is_retained() {
            return Err(Refusal::RetainedInput);
        }
        if !self.resources.admissions().is_empty() {
            return Err(Refusal::Scope);
        }
        let member = self.receipt.as_ref().ok_or(Refusal::NotMember)?;
        let policy = self.config.preparation.ok_or(Refusal::Preparation)?;
        self.freshness = self.freshness.checked_add(1).ok_or(Refusal::Freshness)?;
        nonce[..8].copy_from_slice(&self.freshness.to_le_bytes());
        let context = NativePreparationContext {
            bootstrap: &self.seed,
            member,
            actual: self.resources.host(),
            image_id: self.config.image_id,
            configuration_id: self.config.configuration_id,
            now_millis: now(),
        };
        crate::ingress::receive(link, &mut self.input, policy, context, nonce, now)
            .await
            .map_err(Refusal::Ingress)
    }
}

#[cfg(feature = "execution")]
#[path = "root_continuation.rs"]
mod continuation;
#[cfg(feature = "execution")]
pub use continuation::{
    ContinuationAcceptanceRefusal, ContinuationParts, InstalledContinuationAcceptance,
    PreparedContinuationInstall,
};

#[path = "root_control.rs"]
mod control;
pub use control::MemberControl;

#[cfg(feature = "execution")]
#[path = "root_fore_carrier.rs"]
mod fore_carrier;
#[cfg(feature = "execution")]
pub use fore_carrier::{
    external_fore_scope_commitment, fore_carrier_scope_commitment, BoundForeCarrierKernels,
    BoundForeSessions, ForeCarrierBounds, ForeCarrierKernels, ForeCarrierRefusal, ForeIoActor,
    ForeIoProgress, ForeIoRefusal, NativeForeCarrierSelection, PreparedForeCarrier,
    PreparedForeSessions,
};
