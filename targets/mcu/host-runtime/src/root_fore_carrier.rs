//! Root's explicit native carrier loan; preparation creates no session or IO.
use super::*;
use crate::{
    execution_workset::{PreparedLocalKernels, StartedEvidence},
    transport::{Carrier, Link},
};
use conduit_body as b;
use conduit_core as c;
#[path = "root_fore_carrier/check.rs"]
mod check;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForeCarrierRefusal {
    MissingSelection,
    Scope,
    Bounds,
    Attribution,
    Workset,
    Fragment,
}
/// Finite installed serial-carrier ceilings; these public numbers grant nothing.
#[derive(Clone, Copy)]
pub struct ForeCarrierBounds {
    pub maximum_lines: u8,
    pub maximum_frame_bytes: u32,
    pub maximum_payload_bytes: u32,
    pub maximum_buffered_bytes: usize,
    pub maximum_in_flight_items: u16,
    pub maximum_frames: u32,
}
struct Installation<'p> {
    image: &'p str,
    configuration: &'p str,
    peer: &'p b::FederationPeer,
    profile: c::AssignedLineForeProfile<'p>,
    lines: &'p [c::AdmittedLine],
    session: &'p str,
    session_contract: &'p str,
    bounds: ForeCarrierBounds,
}
/// Affine original native carrier/backing custody plus immutable installation.
/// No safe metadata constructor, Clone, serialization or raw IO extraction exists.
/// ```compile_fail
/// use conduit_mcu_host_runtime::{root::NativeForeCarrierSelection,transport::{Carrier,Link}};
/// fn overlap<'a,'p,C:Carrier+'a>(link:&'a mut Link<C>,
///     install:impl FnOnce(&'a mut Link<C>)->NativeForeCarrierSelection<'a,'p,C>) {
///     let selected=install(link);
///     link.reset(); // The original loan remains exclusively held.
///     drop(selected);
/// }
/// ```
pub struct NativeForeCarrierSelection<'c, 'p, C: Carrier> {
    _link: &'c mut Link<C>,
    backing: &'c mut [u8],
    installed: Installation<'p>,
}
impl<'c, 'p, C: Carrier> NativeForeCarrierSelection<'c, 'p, C> {
    /// Explicit native installation, not permission inferred from a received Plan.
    /// # Safety
    /// Trusted Root composition owns THIS initialized physical serial Link and
    /// THIS complete finite backing. The immutable image/configuration, peer,
    /// exact Line/Fore/session pins and limits are independently checked installed
    /// Host selection; none may originate from received preparation metadata.
    /// The native owner has independently established this carrier-to-peer
    /// physical custody mapping. This function cannot attest it from bytes and
    /// claims neither peer authentication nor network confidentiality. The loan
    /// must remain exclusive through the eventual ordinary workset retirement.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_native_custody(
        link: &'c mut Link<C>,
        backing: &'c mut [u8],
        image: &'p str,
        configuration: &'p str,
        peer: &'p b::FederationPeer,
        profile: c::AssignedLineForeProfile<'p>,
        lines: &'p [c::AdmittedLine],
        session: &'p str,
        session_contract: &'p str,
        bounds: ForeCarrierBounds,
    ) -> Result<Self, ForeCarrierRefusal> {
        check::bounds(bounds, backing.len(), profile.lines.len())?;
        if lines.len() != profile.lines.len() {
            return Err(ForeCarrierRefusal::Bounds);
        }
        for text in [image, configuration, session, session_contract] {
            if text.is_empty() || text.len() > b::MAX_LIFECYCLE_ID_BYTES {
                return Err(ForeCarrierRefusal::Scope);
            }
        }
        Ok(Self {
            _link: link,
            backing,
            installed: Installation {
                image,
                configuration,
                peer,
                profile,
                lines,
                session,
                session_contract,
                bounds,
            },
        })
    }
}
/// Descriptive reproducible full scope, not authority. Transcript is SHA256 of
/// "conduit.root/fore-carrier-scope@1\0", child-document SHA256[32], image and
/// configuration (each u32-LE length then UTF-8), then bounds in this order:
/// lines u8, frame u32-LE, payload u32-LE, backing u64-LE, in-flight u16-LE,
/// frames u32-LE. Every input is public independently selected installation.
pub fn fore_carrier_scope_commitment(
    child_sha256: &[u8; 32],
    image: &str,
    configuration: &str,
    bounds: ForeCarrierBounds,
) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"conduit.root/fore-carrier-scope@1\0");
    hash.update(child_sha256);
    for text in [image, configuration] {
        hash.update((text.len() as u32).to_le_bytes());
        hash.update(text.as_bytes());
    }
    hash.update([bounds.maximum_lines]);
    hash.update(bounds.maximum_frame_bytes.to_le_bytes());
    hash.update(bounds.maximum_payload_bytes.to_le_bytes());
    hash.update((bounds.maximum_buffered_bytes as u64).to_le_bytes());
    hash.update(bounds.maximum_in_flight_items.to_le_bytes());
    hash.update(bounds.maximum_frames.to_le_bytes());
    hash.finalize().into()
}
/// Descriptive per-Fore transcript, never a grant. Exact bytes are SHA256 of
/// "conduit.root/external-fore-carrier@1\0" || full scope commitment[32] ||
/// original public command custody transcript[32] || exact Line/Fore row[198].
/// Linux can reproduce both commitments; no secret input is included.
pub fn external_fore_scope_commitment(
    full_scope: &[u8; 32],
    custody: &[u8; 32],
    line: &c::AssignedLineForeRequirement,
) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"conduit.root/external-fore-carrier@1\0");
    hash.update(full_scope);
    hash.update(custody);
    hash.update(line.payload());
    hash.finalize().into()
}
/// Prepared original loan. No receive/send, carrier extraction or standalone
/// bind/Start exists; attach it to the original workset's prepared kernel group.
pub struct PreparedForeCarrier<'c, 'p, C: Carrier> {
    selection: NativeForeCarrierSelection<'c, 'p, C>,
    scope: [u8; 32],
    custody: [u8; 32],
}
impl<'c, 'p, C: Carrier> PreparedForeCarrier<'c, 'p, C> {
    pub fn scope_digest(&self) -> &[u8; 32] {
        &self.scope
    }
    pub fn fore_scope_digest(&self, endpoint: u16, cord: u16) -> Option<[u8; 32]> {
        let line = self
            .selection
            .installed
            .profile
            .lines
            .iter()
            .find(|line| line.fore().endpoint() == endpoint && line.fore().cord() == cord)?;
        Some(external_fore_scope_commitment(
            &self.scope,
            &self.custody,
            line,
        ))
    }

    pub fn attach<T: PreparedLocalKernels>(self, kernels: T) -> ForeCarrierKernels<'c, 'p, C, T> {
        ForeCarrierKernels {
            kernels,
            carrier: self,
        }
    }
}
/// Same ordinary PreparedLocalKernels admission/Start path, with finite original
/// borrowed carrier backing charged alongside the original prepared kernels.
pub struct ForeCarrierKernels<'c, 'p, C: Carrier, T: PreparedLocalKernels> {
    kernels: T,
    carrier: PreparedForeCarrier<'c, 'p, C>,
}
/// Bound only by the original workset Start. Raw carrier remains private until
/// the typed runtime actor is wired; no public IO/extraction entrance exists.
pub struct BoundForeCarrierKernels<'c, 'p, C: Carrier, T> {
    kernels: T,
    _carrier: PreparedForeCarrier<'c, 'p, C>,
}
impl<C: Carrier, T> BoundForeCarrierKernels<'_, '_, C, T> {
    pub fn kernels_mut(&mut self) -> &mut T {
        &mut self.kernels
    }
    pub fn scope_digest(&self) -> &[u8; 32] {
        &self._carrier.scope
    }
    pub fn fore_scope_digest(&self, endpoint: u16, cord: u16) -> Option<[u8; 32]> {
        self._carrier.fore_scope_digest(endpoint, cord)
    }

    /// Required again by the future typed actor before use; a different original
    /// attempt, even with identical payload bytes, cannot authorize this loan.
    pub fn matches_started(&self, started: &StartedEvidence) -> bool {
        started.custody_identity() == &self._carrier.custody
    }
}
// SAFETY: both original owners are transferred once by the SAME existing Start.
// No IO/allocation occurs in bind; the carrier stays private. Original kernels'
// unsafe contract still requires the factory's exact approved fragment set.
unsafe impl<'c, 'p, C: Carrier, T: PreparedLocalKernels> PreparedLocalKernels
    for ForeCarrierKernels<'c, 'p, C, T>
{
    type Bound = BoundForeCarrierKernels<'c, 'p, C, T::Bound>;
    fn retained_backing_bytes(&self) -> Option<usize> {
        let wrapper = core::mem::size_of::<Self>().checked_sub(core::mem::size_of::<T>())?;
        self.kernels
            .retained_backing_bytes()?
            .checked_add(wrapper)?
            .checked_add(self.carrier.selection.backing.len())
    }
    unsafe fn bind(self) -> Self::Bound {
        // SAFETY: caller satisfies the unchanged original workset admission/Start.
        let kernels = unsafe { self.kernels.bind() };
        BoundForeCarrierKernels {
            kernels,
            _carrier: self.carrier,
        }
    }
}
impl<'s, const N: usize> Root<'s, N> {
    /// Recheck this Root's original online whole-set command. Returning a ticket
    /// borrows only the selected carrier/backing and immutable installation.
    pub fn prepare_fore_carrier<'c, 'p, C: Carrier, const F: usize, const PORTS: usize>(
        &self,
        now: u64,
        selection: Option<NativeForeCarrierSelection<'c, 'p, C>>,
        approved: &b::DelegatedBodyFragment<'_>,
        fore: &b::ApprovedBodyForePorts<PORTS>,
        profile: b::AssignedBodyLineForePreparationProfile<'_>,
    ) -> Result<PreparedForeCarrier<'c, 'p, C>, ForeCarrierRefusal> {
        let member = self.receipt.as_ref().ok_or(ForeCarrierRefusal::Scope)?;
        let context = b::NativePreparationContext {
            bootstrap: &self.seed,
            member,
            actual: self.resources.host(),
            image_id: self.config.image_id,
            configuration_id: self.config.configuration_id,
            now_millis: now,
        };
        check::prepare::<C, N, F, PORTS>(
            &self.input,
            context,
            &self.config,
            selection,
            approved,
            fore,
            profile,
        )
    }
}
impl<const N: usize> ExecutionParts<'_, '_, N> {
    /// Same Root gate over its original disjoint loans, usable inside the real
    /// prepared-workset factory without reborrowing or reconstructing Root.
    pub fn prepare_fore_carrier<'c, 'p, C: Carrier, const F: usize, const PORTS: usize>(
        &self,
        now: u64,
        selection: Option<NativeForeCarrierSelection<'c, 'p, C>>,
        approved: &b::DelegatedBodyFragment<'_>,
        fore: &b::ApprovedBodyForePorts<PORTS>,
        profile: b::AssignedBodyLineForePreparationProfile<'_>,
    ) -> Result<PreparedForeCarrier<'c, 'p, C>, ForeCarrierRefusal> {
        let context = b::NativePreparationContext {
            bootstrap: self.bootstrap,
            member: self.member,
            actual: self.resources.host(),
            image_id: self.configuration.image_id,
            configuration_id: self.configuration.configuration_id,
            now_millis: now,
        };
        check::prepare::<C, N, F, PORTS>(
            self.workspace,
            context,
            self.configuration,
            selection,
            approved,
            fore,
            profile,
        )
    }
}
#[cfg(test)]
#[path = "root_fore_carrier/tests.rs"]
mod tests;
