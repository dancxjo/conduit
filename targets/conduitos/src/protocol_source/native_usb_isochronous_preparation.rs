//! Compose actual USB ownership through ordinary Source planning and Body admission.
use super::native_preparation::{
    NativeProtocolPreparationReason, NativeProtocolPreparationRefusal,
};
use super::{NativeProtocolIssuer, PreparedProtocolBodyPlay, PreparedProtocolEntry};
use crate::{i2c_base::I2cProvider, monotonic_clock::owner::MonotonicDeadlineProvider};
use alloc::{boxed::Box, collections::BTreeMap, vec};
use conduit_body::BodyLifecycleSession;
use conduit_core::*;
use conduit_planner::PlanningOptions;

/// Plan and admit both transports with independently issued exact possession.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_native_usb_isochronous_protocol<
    P: I2cProvider,
    C: MonotonicDeadlineProvider,
>(
    entry: PreparedProtocolEntry,
    session: BodyLifecycleSession,
    mut advertisement: HostAdvertisement,
    ready: crate::arch::ReadyUsbIsochronousHost,
    issuer: NativeProtocolIssuer,
    endpoint_issuer: NativeProtocolIssuer,
    body_play_sequence: u64,
    work_units: u64,
    additional_remote_sign_items: u16,
) -> Result<PreparedProtocolBodyPlay<P, C>, Box<NativeProtocolPreparationRefusal>> {
    use NativeProtocolPreparationReason as Reason;
    let mut retained_session = Some(session);
    let prepared = (|| {
        let contract = crate::usb_base::control_contract::ControlContract::prepare()
            .map_err(Reason::Contract)?;
        let endpoint_contract =
            crate::usb_base::isochronous::contract::IsochronousOutContract::prepare()
                .map_err(Reason::Contract)?;
        ready
            .append_to_advertisement(&mut advertisement, &contract, &endpoint_contract)
            .map_err(|error| {
                Reason::Body(super::body_play::ProtocolBodyRefusal::Call(
                    crate::protocol_host_calls::ProtocolCallRefusal::UsbProtocol(error),
                ))
            })?;
        entry
            .publish_pure_backs(&mut advertisement)
            .map_err(Reason::Source)?;
        super::preparation_stage("iso-advertised");
        let grants = [issuer.grant().clone(), endpoint_issuer.grant().clone()];
        let hosts = [advertisement];
        let placements = entry.placements(&hosts).map_err(Reason::Source)?;
        super::preparation_stage("iso-placements");
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
        super::preparation_stage("iso-planned");
        let plan = &artifact.artifact().definition().internal_plan;
        // The composite kernel prepares one fragment Play with sequence zero.
        // Its identity remains distinct from the aggregate Body Play identity.
        let active = bind_active_play(&plan.plan_id, &hosts[0].host_id, &hosts[0].boot_id, 0);
        let possession = issuer
            .issue(
                plan,
                &active,
                &ImplementationId::from(crate::usb_base::control_factory::CONTROL_IMPLEMENTATION),
                work_units,
            )
            .map_err(Reason::Possession)?;
        let endpoint_possession = endpoint_issuer
            .issue(
                plan,
                &active,
                &ImplementationId::from(
                    crate::usb_base::isochronous::factory::ISOCHRONOUS_OUT_IMPLEMENTATION,
                ),
                crate::usb_base::isochronous::contract::ISOCHRONOUS_OUT_MAXIMUM_PACKETS as u64,
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
        super::preparation_stage("iso-proposed");
        match PreparedProtocolBodyPlay::prepare_usb_isochronous(
            artifact,
            retained_session.take().expect("retained body"),
            body_play_sequence,
            crate::protocol_play::UsbIsochronousAdmission {
                additional_remote_sign_items,
                ready,
                control_table: possession.table,
                control_handle: possession.handle,
                control_claim: possession.claim,
                endpoint_table: endpoint_possession.table,
                endpoint_handle: endpoint_possession.handle,
                endpoint_claim: endpoint_possession.claim,
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
