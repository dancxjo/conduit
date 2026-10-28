//! Native possession enters ordinary Source planning and the canonical Body.
use super::{NativeProtocolIssuer, PreparedProtocolBodyPlay, PreparedProtocolEntry};
use crate::{
    digital_base::{DigitalProvider, contract::DigitalContract, installation::ReadyDigitalBase},
    monotonic_clock::{
        contract::MonotonicClockContract, installation::ReadyClockBase,
        owner::MonotonicDeadlineProvider,
    },
    protocol_play::{
        ClockAdmission, DigitalAdmission, NoI2cProvider, PeripheralAdmission, SpiAdmission,
    },
    spi_base::{SpiProvider, contract::SpiContract, installation::ReadySpiBase},
};
use alloc::{boxed::Box, collections::BTreeMap, vec, vec::Vec};
use conduit_body::BodyLifecycleSession;
use conduit_core::*;
use conduit_planner::PlanningOptions;

/// Exact checked gear selection belongs to native realization, never Source.
pub struct NativeDigitalOwner {
    pub gear: GearId,
    pub ready: ReadyDigitalBase<Box<dyn DigitalProvider>>,
    pub issuer: NativeProtocolIssuer,
}
pub struct NativePeripheralOwners<C> {
    pub spi: ReadySpiBase<Box<dyn SpiProvider>>,
    pub spi_issuer: NativeProtocolIssuer,
    pub digital: Vec<NativeDigitalOwner>,
    pub clock: ReadyClockBase<C>,
    pub clock_issuer: NativeProtocolIssuer,
}

#[derive(Debug)]
pub enum NativePeripheralReason {
    Selection,
    Contract(StructuredInfoRefusal),
    Spi(crate::spi_base::installation::SpiInstallationRefusal),
    Digital(crate::digital_base::installation::DigitalInstallationRefusal),
    Clock(crate::monotonic_clock::installation::ClockInstallationRefusal),
    Source(super::ProtocolSourceRefusal),
    Possession(super::NativeProtocolIssueRefusal),
    Lifecycle(conduit_body::BodyLifecycleSessionError),
    Body(super::ProtocolBodyRefusal),
}
#[derive(Debug)]
pub struct NativePeripheralRefusal {
    pub session: BodyLifecycleSession,
    pub reason: NativePeripheralReason,
}

/// No transaction or clock polling occurs during preparation. Every digital
/// contact is independently possessed and matched to an exact checked gear.
pub fn prepare_native_peripheral<C: MonotonicDeadlineProvider>(
    entry: PreparedProtocolEntry,
    session: BodyLifecycleSession,
    mut advertisement: HostAdvertisement,
    owners: NativePeripheralOwners<C>,
    body_play_sequence: u64,
) -> Result<PreparedProtocolBodyPlay<NoI2cProvider, C>, Box<NativePeripheralRefusal>> {
    use NativePeripheralReason as Error;
    let mut retained = Some(session);
    let result = (|| {
        if owners.digital.is_empty() || owners.digital.len() > 64 {
            return Err(Error::Selection);
        }
        owners
            .spi
            .append_to_advertisement(
                &mut advertisement,
                &SpiContract::prepare().map_err(Error::Contract)?,
            )
            .map_err(Error::Spi)?;
        let digital_contract = DigitalContract::prepare().map_err(Error::Contract)?;
        for owner in &owners.digital {
            owner
                .ready
                .append_to_advertisement(&mut advertisement, &digital_contract)
                .map_err(Error::Digital)?;
        }
        owners
            .clock
            .append_to_advertisement(
                &mut advertisement,
                &MonotonicClockContract::prepare().map_err(Error::Contract)?,
            )
            .map_err(Error::Clock)?;
        entry
            .publish_pure_backs(&mut advertisement)
            .map_err(Error::Source)?;
        // Independently appended owners do not prescribe the planner's
        // canonical pool order. Sorting retains every duplicate for refusal.
        advertisement
            .resources
            .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
        let mut grants = vec![
            owners.spi_issuer.grant().clone(),
            owners.clock_issuer.grant().clone(),
        ];
        grants.extend(
            owners
                .digital
                .iter()
                .map(|owner| owner.issuer.grant().clone()),
        );
        let hosts = [advertisement];
        let mut placements = entry.placements(&hosts).map_err(Error::Source)?;
        let mut selected = alloc::collections::BTreeSet::new();
        for owner in &owners.digital {
            if !selected.insert(owner.gear.clone())
                || !entry.expanded().expanded.gears.iter().any(|gear| {
                    gear.gear_id == owner.gear && gear.kind_id == digital_contract.kind().kind_id
                })
            {
                return Err(Error::Selection);
            }
            let choice = placements
                .by_gear
                .get_mut(&owner.gear)
                .ok_or(Error::Selection)?;
            choice.host_id = hosts[0].host_id.clone();
            choice.capability_id = owner.issuer.grant().capability_id.clone();
        }
        let empty = BTreeMap::new();
        let artifact = entry
            .plan(
                &hosts,
                &placements,
                &[BaseImplementationId::from("conduit.base/local@1")],
                PlanningOptions {
                    connection_bases: &empty,
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: 4096,
                    authority_grants: &grants,
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
            .map_err(Error::Source)?;
        let plan = &artifact.artifact().definition().internal_plan;
        let active = bind_active_play(&plan.plan_id, &hosts[0].host_id, &hosts[0].boot_id, 0);
        let spi_impl = ImplementationId::from(crate::spi_base::installation::SPI_IMPLEMENTATION);
        let spi_placement = plan.fragments[0]
            .placements
            .iter()
            .find(|gear| gear.implementation_id == spi_impl)
            .ok_or(Error::Selection)?
            .placement_id
            .clone();
        let spi = owners
            .spi_issuer
            .issue(plan, &active, &spi_impl, 1)
            .map_err(Error::Possession)?;
        let mut digital = Vec::with_capacity(owners.digital.len());
        for owner in owners.digital {
            let placement = plan.fragments[0]
                .placements
                .iter()
                .find(|gear| gear.gear_id == owner.gear)
                .ok_or(Error::Selection)?
                .placement_id
                .clone();
            let possession = owner
                .issuer
                .issue(
                    plan,
                    &active,
                    &ImplementationId::from(
                        crate::digital_base::installation::DIGITAL_IMPLEMENTATION,
                    ),
                    1,
                )
                .map_err(Error::Possession)?;
            digital.push(DigitalAdmission {
                placement,
                ready: owner.ready,
                table: possession.table,
                handle: possession.handle,
                claim: possession.claim,
            });
        }
        let clock = owners
            .clock_issuer
            .issue(
                plan,
                &active,
                &ImplementationId::from(crate::monotonic_clock::installation::CLOCK_IMPLEMENTATION),
                crate::monotonic_clock::owner::MAXIMUM_POLL_STEPS,
            )
            .map_err(Error::Possession)?;
        retained
            .as_mut()
            .expect("retained biography")
            .propose(
                vec![artifact.body_partition()],
                &hosts[0].host_id,
                &hosts[0].boot_id,
            )
            .map_err(Error::Lifecycle)?;
        match PreparedProtocolBodyPlay::prepare_peripheral(
            artifact,
            retained.take().expect("retained biography"),
            body_play_sequence,
            PeripheralAdmission {
                spi: Some(SpiAdmission {
                    placement: spi_placement,
                    ready: owners.spi,
                    table: spi.table,
                    handle: spi.handle,
                    claim: spi.claim,
                }),
                digital,
            },
            ClockAdmission {
                ready: owners.clock,
                table: clock.table,
                handle: clock.handle,
                claim: clock.claim,
            },
        ) {
            Ok(play) => Ok(play),
            Err(refusal) => {
                retained = Some(refusal.session);
                Err(Error::Body(refusal.reason))
            }
        }
    })();
    result.map_err(|reason| {
        Box::new(NativePeripheralRefusal {
            session: retained.expect("refused biography"),
            reason,
        })
    })
}

#[cfg(test)]
mod tests;
