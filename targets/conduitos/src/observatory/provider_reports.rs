//! Exact initialized provider provenance for ordinary Observatory reports.
use super::ExportError;
use crate::{identity::hex, machine::BaseKind, offer::HostOffer};
use alloc::{format, vec::Vec};
use conduit_core::{HostAdvertisement, HostBaseId, HostBaseKindId};
use conduit_observatory::{BaseReport, FramebufferBasis, OperationalState};

pub(crate) fn fixed_base_reports(
    offer: &HostOffer<'_>,
    advertisement: &HostAdvertisement,
) -> Result<Vec<BaseReport>, ExportError> {
    offer
        .bases
        .iter()
        .map(|base| {
            let advertised_pair = advertisement.bases.iter().find(|advertised| {
                base.kind == BaseKind::Timer
                    && advertised.base_id.as_str() == hex(&base.id)
                    && advertised.implementation_id.as_str()
                        == crate::ordinary_base::TIMER_PROVIDER_IMPLEMENTATION
            });
            let instance = if let Some(advertised) = advertised_pair {
                let pair = crate::ordinary_base::timer_clock_provider(offer)
                    .map_err(|_| ExportError::InvalidSnapshot)?;
                // Derive current provenance from Root's providers; never adopt an
                // arbitrary advertised identity or conceal a stale provider epoch.
                if advertised.provider_instance_id.as_str() != hex(&pair.provider_instance_id)
                    || advertised.provider_generation != pair.provider_generation
                    || advertised.mechanism_family.as_str() != BaseKind::Timer.as_str()
                {
                    return Err(ExportError::InvalidSnapshot);
                }
                pair.provider_instance_id
            } else {
                base.provider_instance_id
            };
            Ok(BaseReport {
                host_id: advertisement.host_id.clone(),
                boot_id: advertisement.boot_id.clone(),
                base_id: HostBaseId::from(hex(&base.id)),
                provider_instance_id: conduit_core::BaseInstanceId::from(hex(&instance)),
                provider_generation: base.provider_generation,
                kind_id: HostBaseKindId::from(format!("conduitos.base/{}@1", base.kind.as_str())),
                implementation_id: None,
                enforcement_class: None,
                lifecycle: None,
                state: OperationalState::Available,
                capacity_units: u64::from(base.capacity),
            })
        })
        .collect()
}

pub(crate) fn append_framebuffer_base(
    bases: &mut Vec<BaseReport>,
    host_id: &conduit_core::HostId,
    boot_id: &conduit_core::BootId,
    framebuffer: Option<&FramebufferBasis>,
) -> Result<(), ExportError> {
    let Some(framebuffer) = framebuffer else {
        return Ok(());
    };
    let capacity_units = u64::from(framebuffer.pitch_bytes)
        .checked_mul(u64::from(framebuffer.height))
        .ok_or(ExportError::InvalidSnapshot)?;
    bases.push(BaseReport {
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        base_id: framebuffer.base_id.clone(),
        provider_instance_id: conduit_core::BaseInstanceId::from(format!(
            "{}/provider/1",
            framebuffer.base_id.as_str()
        )),
        provider_generation: 1,
        kind_id: HostBaseKindId::from("conduitos.base/framebuffer@1"),
        implementation_id: None,
        enforcement_class: None,
        lifecycle: None,
        state: OperationalState::Available,
        capacity_units,
    });
    Ok(())
}

pub(crate) fn append_advertised_bases(
    bases: &mut Vec<BaseReport>,
    advertisement: &conduit_core::HostAdvertisement,
) {
    for advertised in &advertisement.bases {
        let resource_capacity = advertisement
            .resources
            .iter()
            .filter(|resource| advertised.resource_pool_ids.contains(&resource.pool_id))
            .map(|resource| u64::from(resource.capacity_units))
            .sum::<u64>();
        let mut report = BaseReport {
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            base_id: advertised.base_id.clone(),
            provider_instance_id: advertised.provider_instance_id.clone(),
            provider_generation: advertised.provider_generation,
            kind_id: advertised.mechanism_family.clone(),
            implementation_id: Some(advertised.implementation_id.clone()),
            enforcement_class: Some(advertised.enforcement_class),
            lifecycle: Some(advertised.lifecycle),
            state: OperationalState::Available,
            capacity_units: resource_capacity
                .max(advertised.capability_ids.len() as u64)
                .max(1),
        };
        // Enrich the same fixed Root provider instead of reporting it twice.
        // A contradictory provider epoch remains a duplicate for validation to refuse.
        if let Some(existing) = bases.iter_mut().find(|base| {
            base.host_id == report.host_id
                && base.boot_id == report.boot_id
                && base.base_id == report.base_id
                && base.provider_instance_id == report.provider_instance_id
                && base.provider_generation == report.provider_generation
        }) {
            report.capacity_units = existing.capacity_units;
            *existing = report;
        } else {
            bases.push(report);
        }
    }
}

#[cfg(test)]
#[path = "provider_report_tests.rs"]
mod tests;
