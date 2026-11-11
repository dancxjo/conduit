//! Bounded local projection instantiation, not planning or execution approval.
//! Make must supply an independently approved structural digest. Neither this
//! borrowed representation nor its newly encoded identities grants authority.
use crate::*;
#[path = "projection_template_binding.rs"]
mod binding;
pub use binding::{
    assigned_local_fore_connection, FreshAssignedFore, FreshAssignedHeader, FreshAssignedNative,
};
use binding::{external, matching_fore, static_binding};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionTemplateRefusal {
    Projection(AssignedPlanRefusal),
    StructuralDigest,
    UnsupportedNetwork,
    Bounds,
    Bindings,
    FreshScope,
}
impl From<AssignedPlanRefusal> for ProjectionTemplateRefusal {
    fn from(e: AssignedPlanRefusal) -> Self {
        Self::Projection(e)
    }
}
/// Checks envelope/framing/counts and the exact immutable digest, not the full
/// Source/profile or possession. Full ordinary projection validation is still
/// mandatory before preparation/admission/Start.
pub struct AssignedLocalProjectionTemplate<'a> {
    bytes: &'a [u8],
}
/// Hash all static bytes; normalize only typed fresh header/native/Fore fields.
/// Make uses this on its checked projection; receiving untrusted bytes and
/// trusting their own returned hash supplies no provenance.
pub fn assigned_local_projection_template_digest(
    bytes: &[u8],
    maxima: AssignedPlanMaxima,
) -> Result<[u8; 32], ProjectionTemplateRefusal> {
    let records = AssignedPlanRecords::new(bytes)?;
    let schema = crate::read_u16(bytes, 8)?;
    if !matches!(
        schema,
        ASSIGNED_NATIVE_PLAN_SCHEMA | ASSIGNED_FORE_NATIVE_PLAN_SCHEMA
    ) {
        return Err(AssignedPlanRefusal::WrongSchema.into());
    }
    if bytes.len() > usize::from(maxima.encoded_bytes)
        || crate::read_u16(bytes, 12)? > maxima.runtime_state_bytes
    {
        return Err(ProjectionTemplateRefusal::Bounds);
    }
    if crate::sha256::digest(&bytes[125..]) != bytes[93..125] {
        return Err(AssignedPlanRefusal::DigestMismatch.into());
    }
    let mut hash = crate::sha256::State::new();
    hash.update(b"conduit.assigned/local-projection-template@1\0");
    hash.update(&bytes[..16]);
    hash.update(&bytes[80..93]);
    let mut counts = [0u8; ASSIGNED_PLAN_COUNT_KINDS];
    let mut fore_count = 0;
    let mut external_count = 0;
    for record in records {
        let record = record?;
        let index = usize::from(record.tag - 1);
        counts[index] = counts[index]
            .checked_add(1)
            .ok_or(ProjectionTemplateRefusal::Bounds)?;
        if counts[index] > maxima.counts[index] {
            return Err(ProjectionTemplateRefusal::Bounds);
        }
        let mut scratch = [0; ASSIGNED_NATIVE_CALL_BYTES];
        let payload = if record.tag == ASSIGNED_HOST_CALL {
            if let Some(native) = assigned_native_binding(record.payload)? {
                let prefix = record.payload[..46]
                    .try_into()
                    .map_err(|_| ProjectionTemplateRefusal::Bindings)?;
                let n = encode_assigned_native_binding(
                    prefix,
                    Some(&static_binding(native)),
                    &mut scratch,
                )?;
                &scratch[..n]
            } else {
                record.payload
            }
        } else if record.tag == ASSIGNED_REMOTE_ENDPOINT {
            if schema != ASSIGNED_FORE_NATIVE_PLAN_SCHEMA || record.payload.len() != 53 {
                return Err(ProjectionTemplateRefusal::UnsupportedNetwork);
            }
            fore_count += 1;
            if fore_count > 16 {
                return Err(ProjectionTemplateRefusal::Bounds);
            }
            record.payload
        } else if external(record) {
            if schema != ASSIGNED_FORE_NATIVE_PLAN_SCHEMA {
                return Err(ProjectionTemplateRefusal::UnsupportedNetwork);
            }
            matching_fore(bytes, record.payload)?;
            external_count += 1;
            scratch[..42].copy_from_slice(record.payload);
            scratch[2..18].fill(0);
            &scratch[..42]
        } else {
            record.payload
        };
        hash.update(&[record.tag]);
        hash.update(&(payload.len() as u16).to_le_bytes());
        hash.update(payload);
    }
    if counts != bytes[80..93] || fore_count != external_count {
        return Err(AssignedPlanRefusal::MalformedRecord.into());
    }
    Ok(hash.finish())
}
impl<'a> AssignedLocalProjectionTemplate<'a> {
    pub fn check(
        bytes: &'a [u8],
        digest: &[u8; 32],
        maxima: AssignedPlanMaxima,
    ) -> Result<Self, ProjectionTemplateRefusal> {
        if &assigned_local_projection_template_digest(bytes, maxima)? != digest {
            return Err(ProjectionTemplateRefusal::StructuralDigest);
        }
        Ok(Self { bytes })
    }
    /// Preflight every typed slot and complete destination before publication.
    /// The entire operation is synchronous/allocation-free. Only after all
    /// refusals are discharged are bytes written; no default calibration Boot,
    /// missing native owner or omitted Fore slot is reused.
    pub fn instantiate(
        &self,
        header: FreshAssignedHeader,
        native: &[FreshAssignedNative],
        fore: &[FreshAssignedFore<'_>],
        output: &mut [u8],
    ) -> Result<usize, ProjectionTemplateRefusal> {
        use ProjectionTemplateRefusal as E;
        let zero = AssignedIdentity([0; 16]);
        if [header.plan, header.fragment, header.host, header.boot].contains(&zero)
            || header.boot == crate::read_identity(self.bytes, 64)?
            || header.plan == crate::read_identity(self.bytes, 16)?
            || header.fragment == crate::read_identity(self.bytes, 32)?
        {
            return Err(E::FreshScope);
        }
        if output.len() < self.bytes.len() || native.len() > 255 || fore.len() > 16 {
            return Err(E::Bounds);
        }
        // Every supplied slot must occur exactly once in the parsed template;
        // total counts alone would miss duplicate records plus an unused slot.
        for slot in native {
            let mut count = 0;
            for record in AssignedPlanRecords::new(self.bytes)? {
                let record = record?;
                if record.tag == ASSIGNED_HOST_CALL
                    && assigned_native_binding(record.payload)?.is_some()
                    && crate::read_u16(record.payload, 0)? == slot.node
                    && crate::read_u16(record.payload, 2)? == slot.call
                    && crate::read_identity(record.payload, 4)? == slot.contract
                {
                    count += 1;
                }
            }
            if count != 1 {
                return Err(E::Bindings);
            }
        }
        for slot in fore {
            let mut count = 0;
            for record in AssignedPlanRecords::new(self.bytes)? {
                let record = record?;
                if external(record) && crate::read_u16(record.payload, 0)? == slot.cord {
                    count += 1;
                }
            }
            if count != 1 {
                return Err(E::Bindings);
            }
        }
        let mut natives = 0;
        let mut fores = 0;
        for record in AssignedPlanRecords::new(self.bytes)? {
            let record = record?;
            if record.tag == ASSIGNED_HOST_CALL {
                if let Some(old) = assigned_native_binding(record.payload)? {
                    let slot = native_slot(record.payload, native)?;
                    if static_binding(slot.binding) != static_binding(old)
                        || slot.binding.host != header.host
                        || slot.binding.boot != header.boot
                        || slot.binding.instance == zero
                        || slot.binding.grant == zero
                        || slot.binding.pool == zero
                    {
                        return Err(E::Bindings);
                    }
                    let mut scratch = [0; 326];
                    encode_assigned_native_binding(
                        record.payload[..46].try_into().map_err(|_| E::Bindings)?,
                        Some(&slot.binding),
                        &mut scratch,
                    )?;
                    natives += 1;
                }
            } else if external(record) {
                fore_slot(self.bytes, record.payload, fore)?;
                fores += 1;
            }
        }
        if natives != native.len() || fores != fore.len() {
            return Err(E::Bindings);
        }
        output[..self.bytes.len()].copy_from_slice(self.bytes);
        for (offset, id) in [
            (16, header.plan),
            (32, header.fragment),
            (48, header.host),
            (64, header.boot),
        ] {
            output[offset..offset + 16].copy_from_slice(&id.0);
        }
        let mut at = ASSIGNED_PLAN_HEADER_BYTES;
        for record in AssignedPlanRecords::new(self.bytes).expect("checked immutable envelope") {
            let record = record.expect("checked immutable records");
            let start = at + 3;
            if record.tag == ASSIGNED_HOST_CALL
                && assigned_native_binding(record.payload)
                    .expect("checked native")
                    .is_some()
            {
                let slot = native_slot(record.payload, native).expect("preflight native slot");
                encode_assigned_native_binding(
                    record.payload[..46].try_into().expect("prefix"),
                    Some(&slot.binding),
                    &mut output[start..start + record.payload.len()],
                )
                .expect("preflight encoding");
            } else if external(record) {
                let slot =
                    fore_slot(self.bytes, record.payload, fore).expect("preflight Fore slot");
                let id = assigned_local_fore_connection(
                    slot.direction,
                    slot.front_port,
                    slot.placement,
                    slot.gear_port,
                );
                output[start + 2..start + 18].copy_from_slice(&id.0);
            }
            at = start + record.payload.len();
        }
        let digest = crate::sha256::digest(&output[125..self.bytes.len()]);
        output[93..125].copy_from_slice(&digest);
        Ok(self.bytes.len())
    }
}
fn native_slot<'a>(
    payload: &[u8],
    slots: &'a [FreshAssignedNative],
) -> Result<&'a FreshAssignedNative, ProjectionTemplateRefusal> {
    let node = crate::read_u16(payload, 0)?;
    let call = crate::read_u16(payload, 2)?;
    let abi = crate::read_identity(payload, 4)?;
    let mut matches = slots
        .iter()
        .filter(|s| s.node == node && s.call == call && s.contract == abi);
    let slot = matches.next().ok_or(ProjectionTemplateRefusal::Bindings)?;
    if matches.next().is_some() || slot.binding.call != abi {
        return Err(ProjectionTemplateRefusal::Bindings);
    }
    Ok(slot)
}
fn fore_slot<'a>(
    bytes: &[u8],
    payload: &[u8],
    slots: &'a [FreshAssignedFore<'a>],
) -> Result<&'a FreshAssignedFore<'a>, ProjectionTemplateRefusal> {
    let record = matching_fore(bytes, payload)?;
    let ordinal = crate::read_u16(payload, 0)?;
    let mut matches = slots.iter().filter(|s| s.cord == ordinal);
    let slot = matches.next().ok_or(ProjectionTemplateRefusal::Bindings)?;
    let input = payload[18] == 1;
    let local = if input { 24 } else { 19 };
    // Canonical Fore names include placement but no Boot. An unchanged checked
    // placement preserves its connection; header/native checks enforce freshness.
    if matches.next().is_some()
        || slot.endpoint != crate::read_u16(&record, 0)?
        || slot.direction
            != if input {
                AssignedForeDirection::Input
            } else {
                AssignedForeDirection::Output
            }
        || slot.local_node != crate::read_u16(payload, local)?
        || slot.local_port != crate::read_u16(payload, local + 2)?
        || AssignedIdentity::from_text(slot.front_port) != crate::read_identity(&record, 5)?
        || [slot.front_port, slot.placement, slot.gear_port]
            .iter()
            .any(|s| s.is_empty() || s.len() > 4096)
    {
        return Err(ProjectionTemplateRefusal::Bindings);
    }
    Ok(slot)
}
#[cfg(test)]
#[path = "projection_template_tests.rs"]
mod tests;
