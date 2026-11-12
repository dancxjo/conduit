//! Explicit representation pins for Line-bound external Fore, never possession.
//!
//! Schema 8 retains the 125-byte envelope and tag-10 inventory. Local records
//! are [0, Fore ABI (53 bytes)]. Selected-Line records are [1, Fore ABI (53),
//! Line/local Host/local Boot/peer Host/peer Boot/session/session contract
//! identities (7 * 16), selected-Line canonical contract commitment (32)].
//! Schemas 4–7 never interpret these tagged variants. Body membership,
//! original authenticated Plan/Play containment and actual session/channel
//! possession remain separate Root checks; this format does not establish them.
use crate::*;

/// Opt-in native Fore projection. Schemas 4–7 keep their original meanings.
/// Runtime table writers and carrier admission do not support this schema yet.
pub const ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA: u16 = 8;
pub const ASSIGNED_LINE_FORE_BYTES: usize = 198;
pub const ASSIGNED_LINE_FORE_LOCAL_BYTES: usize = 54;

/// Trusted Root pins for one selected Line, including the exact existing Fore
/// ABI, Host/Boot peers, session identity/profile and full selected-Line contract
/// commitment (limits, endpoints and realization facts). Preparing these bytes
/// grants no Line, session, membership or permission to call the Fore. Root must
/// derive the session/profile and canonical contract commitment from original
/// verified material; these opaque pins do not verify or attest that material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssignedLineForeRequirement {
    fore: AssignedForeRequirement,
    binding: AssignedRemoteBinding,
    payload: [u8; ASSIGNED_LINE_FORE_BYTES],
}
impl AssignedLineForeRequirement {
    pub const fn new(
        fore: AssignedForeRequirement,
        binding: AssignedRemoteBinding,
        session: AssignedIdentity,
        session_contract: AssignedIdentity,
        selected_line_contract_digest: [u8; 32],
    ) -> Self {
        let mut payload = [0; ASSIGNED_LINE_FORE_BYTES];
        payload[0] = 1;
        let mut i = 0;
        while i < ASSIGNED_LOCAL_FORE_BYTES {
            payload[1 + i] = fore.payload()[i];
            i += 1;
        }
        let ids = [
            binding.line,
            binding.local_host,
            binding.local_boot,
            binding.peer_host,
            binding.peer_boot,
            session,
            session_contract,
        ];
        let mut id = 0;
        while id < ids.len() {
            i = 0;
            while i < 16 {
                payload[54 + id * 16 + i] = ids[id].0[i];
                i += 1;
            }
            id += 1;
        }
        i = 0;
        while i < 32 {
            payload[166 + i] = selected_line_contract_digest[i];
            i += 1;
        }
        Self {
            fore,
            binding,
            payload,
        }
    }
    pub fn payload(&self) -> &[u8; ASSIGNED_LINE_FORE_BYTES] {
        &self.payload
    }
}

/// Immutable expected records from trusted Root preparation. Received records
/// cannot add a Line or replace these independently selected expectations.
#[derive(Clone, Copy)]
pub struct AssignedLineForeProfile<'a> {
    pub local: &'a [AssignedForeRequirement],
    pub lines: &'a [AssignedLineForeRequirement],
}

/// Schema-8 local Fore variant; not a schema-7 record or network permission.
pub fn assigned_line_fore_local_payload(
    fore: &AssignedForeRequirement,
) -> [u8; ASSIGNED_LINE_FORE_LOCAL_BYTES] {
    let mut result = [0; ASSIGNED_LINE_FORE_LOCAL_BYTES];
    result[1..].copy_from_slice(fore.payload());
    result
}

/// Validate representation and exact trusted pins only. Success is not carrier
/// installation, executable kernel support, live availability or admission.
pub fn decode_assigned_plan_with_line_fore(
    bytes: &[u8],
    maxima: AssignedPlanMaxima,
    requirements: AssignedPlanRequirements<'_>,
    computations: AssignedComputationProfile<'_>,
    profile: AssignedLineForeProfile<'_>,
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    crate::validation::decode_line_fore(bytes, maxima, requirements, computations, profile)
}

pub(crate) struct Validation<'a> {
    profile: AssignedLineForeProfile<'a>,
    local_seen: [bool; 16],
    lines_seen: [bool; 16],
}
impl<'a> Validation<'a> {
    pub(crate) fn new(
        profile: AssignedLineForeProfile<'a>,
        required: AssignedPlanRequirements<'_>,
    ) -> Result<Self, AssignedPlanRefusal> {
        if profile
            .local
            .len()
            .checked_add(profile.lines.len())
            .is_none_or(|n| n > 16)
        {
            return Err(AssignedPlanRefusal::MalformedRecord);
        }
        let all = || {
            profile
                .local
                .iter()
                .chain(profile.lines.iter().map(|line| &line.fore))
        };
        for (i, fore) in all().enumerate() {
            if all()
                .take(i)
                .any(|prior| prior.endpoint() == fore.endpoint() || prior.cord() == fore.cord())
            {
                return Err(AssignedPlanRefusal::MalformedRecord);
            }
        }
        for line in profile.lines {
            if line.binding.local_host != required.host
                || line.binding.local_boot != required.boot
                || !required.remote_bindings.contains(&line.binding)
            {
                return Err(AssignedPlanRefusal::StaleRemoteEndpoint);
            }
        }
        if required
            .remote_bindings
            .iter()
            .enumerate()
            .any(|(i, binding)| {
                required.remote_bindings[..i].contains(binding)
                    || !profile.lines.iter().any(|line| &line.binding == binding)
            })
        {
            return Err(AssignedPlanRefusal::StaleRemoteEndpoint);
        }
        Ok(Self {
            profile,
            local_seen: [false; 16],
            lines_seen: [false; 16],
        })
    }
    pub(crate) fn record(
        &mut self,
        payload: &[u8],
    ) -> Result<Option<AssignedRemoteBinding>, AssignedPlanRefusal> {
        if payload[0] == 0 {
            let i = self
                .profile
                .local
                .iter()
                .enumerate()
                .find(|(i, fore)| {
                    !self.local_seen[*i] && fore.payload().as_slice() == &payload[1..]
                })
                .map(|(i, _)| i)
                .ok_or(AssignedPlanRefusal::UnknownForeEndpoint)?;
            self.local_seen[i] = true;
            Ok(None)
        } else {
            let i = self
                .profile
                .lines
                .iter()
                .enumerate()
                .find(|(i, line)| !self.lines_seen[*i] && line.payload().as_slice() == payload)
                .map(|(i, _)| i)
                .ok_or(AssignedPlanRefusal::StaleRemoteEndpoint)?;
            self.lines_seen[i] = true;
            Ok(Some(self.profile.lines[i].binding))
        }
    }
    pub(crate) fn finish(&self) -> Result<(), AssignedPlanRefusal> {
        if self.local_seen[..self.profile.local.len()].contains(&false)
            || self.lines_seen[..self.profile.lines.len()].contains(&false)
        {
            Err(AssignedPlanRefusal::MissingForeEndpoint)
        } else {
            Ok(())
        }
    }
}
