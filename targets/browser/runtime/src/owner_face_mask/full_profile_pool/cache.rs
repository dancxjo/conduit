//! Immutable local Boot witness, not a retained duplicate Host advertisement.
use super::*;
use conduit_core::{OfferGeneration, PROTOCOL_VERSION};
use conduit_face::{PlannedMaskPlot, RemoteMaskHostBasis};
use sha2::{Digest, Sha256};
use std::io::{Error, Write};
const MAX_PLAN_BYTES: usize = 256 * 1024;

pub(crate) struct Evidence {
    basis: RemoteMaskHostBasis,
    canonical: Box<[u8]>,
    template: PlannedMaskPlot,
    charged: usize,
}
impl Evidence {
    pub(crate) fn basis(&self) -> &RemoteMaskHostBasis {
        &self.basis
    }
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub(crate) fn template(&self) -> &PlannedMaskPlot {
        &self.template
    }
    pub(crate) fn charged_bytes(&self) -> usize {
        self.charged
    }
    pub(crate) fn offer_generation(&self) -> OfferGeneration {
        self.basis.offer_generation
    }
}

pub(super) fn capture(
    host: &HostAdvertisement,
    bytes: &[u8],
    available: usize,
) -> Result<Evidence, String> {
    if bytes.is_empty()
        || bytes.len() > conduit_body::MAX_CANDIDATE_ADVERTISEMENT_BYTES as usize
        || host.protocol_version != PROTOCOL_VERSION
        || host.offer_generation.0 == 0
        || host.host_id.as_str().len() > 128
        || host.boot_id.as_str().len() > 128
        || host.profile.as_str().len() > 128
    {
        return Err("Mask Boot witness extent or protocol".into());
    }
    if host
        .resources
        .windows(2)
        .any(|w| w[0].pool_id >= w[1].pool_id)
        || host
            .capabilities
            .windows(2)
            .any(|w| w[0].capability_id >= w[1].capability_id)
    {
        return Err("Mask Boot witness inventory is not canonical".into());
    }
    if host
        .capabilities
        .iter()
        .filter(|c| c.capability_id.as_str() == profile::GENERIC_CAPABILITY)
        .count()
        != 1
        || !host.capabilities.contains(&profile::offer())
    {
        return Err("Mask Boot witness offer missing".into());
    }
    let mut pools = host
        .resources
        .iter()
        .filter(|r| r.class_id.as_str() == profile::MEMORY_CLASS);
    let pool = pools.next().ok_or("Mask Boot witness pool missing")?;
    if pools.next().is_some()
        || pool.pool_id.as_str() != profile::MEMORY_POOL
        || pool.capacity_units != profile::PREPARATION_BYTES * profile::PREPARATION_SLOTS as u32
        || host
            .resources
            .iter()
            .any(|r| r.pool_id == pool.pool_id && r.class_id != pool.class_id)
    {
        return Err("Mask Boot witness pool mismatch".into());
    }
    let face_pools = host
        .resources
        .iter()
        .filter(|r| r.class_id.as_str() == conduit_core::FACE_RESOURCE_CLASS);
    if face_pools.count() != 1
        || host
            .resources
            .iter()
            .filter(|r| {
                r.class_id.as_str() == profile::MEMORY_CLASS
                    || r.class_id.as_str() == conduit_core::FACE_RESOURCE_CLASS
            })
            .any(|r| {
                r.pool_id.as_str().len() > 128
                    || r.pool_id == pool.pool_id && r.class_id != pool.class_id
                    || r.compute.is_some()
                    || r.content.is_some()
            })
    {
        return Err("Mask Boot template resource shape".into());
    }
    // Verify that the bytes being retained are the actual canonical local
    // snapshot, without allocating a second full serialization buffer.
    verify_bytes(host, bytes)?;
    let mut template = profile::template_plan(host)?;
    let plan_bytes = profile::bounded_json_bytes(&template.plan, MAX_PLAN_BYTES)?;
    let plan_charge =
        PlannedMaskPlot::generic_json_plan_preparation_bytes(&plan_bytes).map_err(str::to_owned)?;
    let mask_strings = mask_strings(&template);
    let retained = std::mem::size_of::<Evidence>()
        + 2 * std::mem::size_of::<usize>()
        + host.host_id.as_str().len()
        + host.boot_id.as_str().len()
        + 64
        + bytes.len()
        + plan_charge
            .checked_sub(std::mem::size_of::<conduit_core::Plan>())
            .ok_or("Mask Boot template bound")?
        + mask_strings;
    // Named overlapping owners after template construction: exact canonical
    // witness, retained template, bounded raw Plan output and parser scratch.
    let known_peak = retained
        .checked_add(plan_bytes.capacity())
        .and_then(|n| n.checked_add(2 * MAX_PLAN_BYTES + 8))
        .ok_or("Mask Boot cache extent")?;
    if known_peak > available {
        return Err("Mask Boot cache preparation pressure".into());
    }
    // Planner-created Vecs can retain spare capacity beyond serde's growth
    // model. Re-admit the exact generic raw Plan before retaining that model;
    // source identities and every selected fact remain byte-for-byte equal.
    let normalized: conduit_core::Plan =
        serde_json::from_slice(&plan_bytes).map_err(|_| "Mask Boot template decode")?;
    if normalized != template.plan {
        return Err("Mask Boot template normalization mismatch".into());
    }
    template.plan = normalized;
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    const DIGITS: &[u8] = b"0123456789abcdef";
    for byte in digest {
        hex.push(DIGITS[(byte >> 4) as usize] as char);
        hex.push(DIGITS[(byte & 15) as usize] as char);
    }
    Ok(Evidence {
        basis: RemoteMaskHostBasis {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            offer_digest: hex,
        },
        canonical: bytes.to_vec().into_boxed_slice(),
        template,
        charged: retained,
    })
}
pub(super) fn verify_bytes(host: &HostAdvertisement, bytes: &[u8]) -> Result<(), String> {
    let mut equal = Equal { bytes, at: 0 };
    serde_json::to_writer(&mut equal, host)
        .map_err(|_| "Mask Boot witness differs from local advertisement")?;
    if equal.at != bytes.len() {
        return Err("Mask Boot witness trailing bytes".into());
    }
    Ok(())
}
fn mask_strings(p: &PlannedMaskPlot) -> usize {
    let m = &p.mask;
    let mut bytes = m.plot_name.capacity()
        + m.plot_identity.source_document_id.0.capacity()
        + m.plot_identity.checked_plot_id.0.capacity()
        + m.plot_identity.expanded_plot_id.0.capacity();
    for b in [&m.face_input, &m.interaction_output, &m.show_output] {
        bytes +=
            b.front_port_id.0.capacity() + b.gear_id.0.capacity() + b.gear_port_id.0.capacity();
    }
    bytes
}
struct Equal<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Write for Equal<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self
            .at
            .checked_add(bytes.len())
            .ok_or_else(|| Error::other("snapshot extent"))?;
        if self.bytes.get(self.at..end) != Some(bytes) {
            return Err(Error::other("snapshot mismatch"));
        }
        self.at = end;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
