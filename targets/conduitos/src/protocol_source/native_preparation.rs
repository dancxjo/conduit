//! Production composition of retained Source and independently admitted native owners.
use super::{
    NativeProtocolIssueRefusal, NativeProtocolIssuer, PreparedProtocolBodyPlay,
    PreparedProtocolEntry, ProtocolBodyRefusal, ProtocolSourceRefusal,
};
use crate::{
    i2c_base::{
        I2cProvider,
        contract::I2cContract,
        installation::{I2C_IMPLEMENTATION, I2cInstallationRefusal, ReadyI2cBase},
    },
    monotonic_clock::{
        contract::MonotonicClockContract,
        installation::{CLOCK_IMPLEMENTATION, ClockInstallationRefusal, ReadyClockBase},
        owner::MonotonicDeadlineProvider,
    },
    protocol_play::{ClockAdmission, I2cAdmission},
};
use alloc::{boxed::Box, collections::BTreeMap, vec};
use conduit_body::{BodyLifecycleSession, BodyLifecycleSessionError};
use conduit_core::*;
use conduit_planner::PlanningOptions;

/// Possession enters from the native Root, never from Source or planner metadata.
pub struct NativeProtocolOwners<P, C> {
    pub bus: ReadyI2cBase<P>,
    pub clock: ReadyClockBase<C>,
    pub bus_issuer: NativeProtocolIssuer,
    pub clock_issuer: NativeProtocolIssuer,
}

pub struct NativeProtocolPreparationLimits {
    pub body_play_sequence: u64,
    pub bus_work_units: u64,
    pub clock_work_units: u64,
}

#[derive(Debug)]
pub enum NativeProtocolPreparationReason {
    Contract(StructuredInfoRefusal),
    Source(ProtocolSourceRefusal),
    Bus(I2cInstallationRefusal),
    Clock(ClockInstallationRefusal),
    Possession(NativeProtocolIssueRefusal),
    Lifecycle(BodyLifecycleSessionError),
    Body(ProtocolBodyRefusal),
}

#[derive(Debug)]
pub struct NativeProtocolPreparationRefusal {
    pub session: BodyLifecycleSession,
    pub reason: NativeProtocolPreparationReason,
}

/// Publish actual ready owners, plan the retained entry, issue exact possession,
/// and propose the complete canonical body before preparing its only kernel.
/// The body must already contain this reviewed Source resident and current Host
/// membership. This function creates neither review nor resource authority and
/// performs no controller transaction, clock poll or Play start.
pub fn prepare_native_protocol<P: I2cProvider, C: MonotonicDeadlineProvider>(
    entry: PreparedProtocolEntry,
    session: BodyLifecycleSession,
    mut advertisement: HostAdvertisement,
    owners: NativeProtocolOwners<P, C>,
    limits: NativeProtocolPreparationLimits,
) -> Result<PreparedProtocolBodyPlay<P, C>, Box<NativeProtocolPreparationRefusal>> {
    use NativeProtocolPreparationReason as Reason;
    let mut retained_session = Some(session);
    let prepared = (|| {
        let i2c = I2cContract::prepare().map_err(Reason::Contract)?;
        let clock = MonotonicClockContract::prepare().map_err(Reason::Contract)?;
        owners
            .bus
            .append_to_advertisement(&mut advertisement, &i2c)
            .map_err(Reason::Bus)?;
        owners
            .clock
            .append_to_advertisement(&mut advertisement, &clock)
            .map_err(Reason::Clock)?;
        entry
            .publish_pure_backs(&mut advertisement)
            .map_err(Reason::Source)?;
        let grants = [
            owners.bus_issuer.grant().clone(),
            owners.clock_issuer.grant().clone(),
        ];
        let hosts = [advertisement];
        let placements = entry.placements(&hosts).map_err(Reason::Source)?;
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
                    connection_byte_capacity: 512,
                    authority_grants: &grants,
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
            .map_err(Reason::Source)?;
        let plan = &artifact.artifact().definition().internal_plan;
        // The composite kernel prepares one fragment Play with sequence zero.
        // Its identity remains distinct from the aggregate Body Play identity.
        let active = bind_active_play(&plan.plan_id, &hosts[0].host_id, &hosts[0].boot_id, 0);
        let bus = owners
            .bus_issuer
            .issue(
                plan,
                &active,
                &ImplementationId::from(I2C_IMPLEMENTATION),
                limits.bus_work_units,
            )
            .map_err(Reason::Possession)?;
        let clock = owners
            .clock_issuer
            .issue(
                plan,
                &active,
                &ImplementationId::from(CLOCK_IMPLEMENTATION),
                limits.clock_work_units,
            )
            .map_err(Reason::Possession)?;
        retained_session
            .as_mut()
            .expect("retained body")
            .propose(
                vec![artifact.body_partition()],
                &hosts[0].host_id,
                &hosts[0].boot_id,
            )
            .map_err(Reason::Lifecycle)?;
        match PreparedProtocolBodyPlay::prepare(
            artifact,
            retained_session.take().expect("retained body"),
            limits.body_play_sequence,
            I2cAdmission {
                ready: owners.bus,
                table: bus.table,
                handle: bus.handle,
                claim: bus.claim,
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
                retained_session = Some(refusal.session);
                Err(Reason::Body(refusal.reason))
            }
        }
    })();
    prepared.map_err(|reason| {
        Box::new(NativeProtocolPreparationRefusal {
            session: retained_session.expect("refused body retained"),
            reason,
        })
    })
}
