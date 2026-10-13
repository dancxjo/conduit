//! Original USB owners and optional independently admitted semantic services.
use super::*;
use crate::protocol_source::NativeProtocolPossession;
#[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
pub(crate) struct NativeEthernetPreparation {
    pub ready: crate::ethernet_frame::owned_installation::ReadyOwnedEthernetFrameBase,
    pub issuer: NativeProtocolIssuer,
    pub random_seed: u64,
    pub timestamp: smoltcp::time::Instant,
}
/// Plan and admit three transports with independently issued exact possession.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_native_usb_duplex<P: I2cProvider, C: MonotonicDeadlineProvider>(
    entry: PreparedProtocolEntry,
    session: BodyLifecycleSession,
    mut advertisement: HostAdvertisement,
    ready: crate::arch::ReadyUsbDuplexHost,
    issuer: NativeProtocolIssuer,
    endpoint_issuer: NativeProtocolIssuer,
    write_issuer: NativeProtocolIssuer,
    body_play_sequence: u64,
    work_units: u64,
    additional_remote_sign_items: u16,
    #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))] network: Option<
        NativeEthernetPreparation,
    >,
    storage: Option<NativeStoragePreparation>,
) -> Result<PreparedProtocolBodyPlay<P, C>, Box<NativeProtocolPreparationRefusal>> {
    use NativeProtocolPreparationReason as Reason;
    let mut retained_session = Some(session);
    let prepared = (|| {
        let contract = crate::usb_base::control_contract::ControlContract::prepare()
            .map_err(Reason::Contract)?;
        let endpoint_contract =
            crate::usb_base::endpoint_read_contract::EndpointReadContract::prepare()
                .map_err(Reason::Contract)?;
        let write_contract =
            crate::usb_base::endpoint_write_contract::EndpointWriteContract::prepare()
                .map_err(Reason::Contract)?;
        ready
            .append_to_advertisement(
                &mut advertisement,
                &contract,
                &endpoint_contract,
                &write_contract,
            )
            .map_err(|error| {
                Reason::Body(super::super::body_play::ProtocolBodyRefusal::Call(
                    crate::protocol_host_calls::ProtocolCallRefusal::UsbProtocol(error),
                ))
            })?;
        #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
        if let Some(network) = &network {
            let frame_contract = crate::ethernet_frame::contract::EthernetFrameContract::prepare()
                .map_err(Reason::Contract)?;
            network
                .ready
                .append_to_advertisement(&mut advertisement, &frame_contract)
                .map_err(|error| {
                    Reason::Body(super::super::body_play::ProtocolBodyRefusal::Call(
                        crate::protocol_host_calls::ProtocolCallRefusal::EthernetFrame(error),
                    ))
                })?;
        }
        if let Some(storage) = &storage {
            if storage
                .additional
                .as_ref()
                .is_some_and(|extra| extra.additional.is_some())
            {
                return Err(Reason::Body(
                    super::super::body_play::ProtocolBodyRefusal::Call(
                        crate::protocol_host_calls::ProtocolCallRefusal::InvalidPlan,
                    ),
                ));
            }
            storage.append(&mut advertisement).map_err(|error| {
                Reason::Body(super::super::body_play::ProtocolBodyRefusal::Call(error))
            })?;
        }
        entry
            .publish_pure_backs(&mut advertisement)
            .map_err(Reason::Source)?;
        // Independently projected owners need one canonical combined pool order.
        // Sorting retains duplicate identities for the planner to reject.
        advertisement
            .resources
            .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
        let mut grants = vec![
            issuer.grant().clone(),
            endpoint_issuer.grant().clone(),
            write_issuer.grant().clone(),
        ];
        #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
        if let Some(network) = &network {
            grants.push(network.issuer.grant().clone());
        }
        if let Some(storage) = &storage {
            grants.push(storage.authorize.grant().clone());
            grants.push(storage.settle.grant().clone());
            if let Some(additional) = &storage.additional {
                grants.push(additional.authorize.grant().clone());
                grants.push(additional.settle.grant().clone());
            }
        }
        let hosts = [advertisement];
        let mut placements = entry.placements(&hosts).map_err(Reason::Source)?;
        if let Some(storage) = &storage {
            storage
                .select_placements(&entry.expanded().expanded.gears, &hosts[0], &mut placements)
                .map_err(|error| {
                    Reason::Body(super::super::body_play::ProtocolBodyRefusal::Call(error))
                })?;
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
                    crate::usb_base::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION,
                ),
                work_units,
            )
            .map_err(Reason::Possession)?;
        let write_possession = write_issuer
            .issue(
                plan,
                &active,
                &ImplementationId::from(
                    crate::usb_base::endpoint_write_factory::ENDPOINT_WRITE_IMPLEMENTATION,
                ),
                work_units,
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
        let usb = crate::protocol_play::UsbDuplexAdmission {
            additional_remote_sign_items,
            ready,
            control_table: possession.table,
            control_handle: possession.handle,
            control_claim: possession.claim,
            endpoint_table: endpoint_possession.table,
            endpoint_handle: endpoint_possession.handle,
            endpoint_claim: endpoint_possession.claim,
            write_table: write_possession.table,
            write_handle: write_possession.handle,
            write_claim: write_possession.claim,
        };
        let result = if let Some(mut storage) = storage {
            let maximum_retained_bytes = storage.maximum_retained_bytes;
            let additional = storage
                .additional
                .take()
                .map(|preparation| {
                    let maximum_retained_bytes = preparation.maximum_retained_bytes;
                    let (authorize, settle) =
                        issue_storage_pair(*preparation, plan, &active, work_units)?;
                    Ok::<_, Reason>((maximum_retained_bytes, authorize, settle))
                })
                .transpose()?;
            let (authorize, settle) = issue_storage_pair(storage, plan, &active, work_units)?;
            let scope = |possession: NativeProtocolPossession| {
                crate::protocol_play::StorageScopePossession {
                    table: possession.table,
                    handle: possession.handle,
                    claim: possession.claim,
                }
            };
            let first = crate::protocol_play::StorageUsbAdmission {
                maximum_retained_bytes,
                usb,
                authorize: scope(authorize),
                settle: scope(settle),
            };
            match additional {
                Some((maximum_retained_bytes, authorize, settle)) => {
                    PreparedProtocolBodyPlay::prepare_storage_usb_duplex_two_pairs(
                        artifact,
                        retained_session.take().expect("retained body"),
                        body_play_sequence,
                        first,
                        crate::protocol_host_calls::AdditionalStoragePairAdmission {
                            maximum_retained_bytes,
                            authorize: scope(authorize),
                            settle: scope(settle),
                        },
                    )
                }
                None => PreparedProtocolBodyPlay::prepare_storage_usb_duplex(
                    artifact,
                    retained_session.take().expect("retained body"),
                    body_play_sequence,
                    first,
                ),
            }
        } else {
            #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
            let result = match network {
                Some(network) => {
                    let possession = network
                        .issuer
                        .issue(
                            plan,
                            &active,
                            &ImplementationId::from(
                                crate::ethernet_frame::factory::FRAME_IMPLEMENTATION,
                            ),
                            work_units,
                        )
                        .map_err(Reason::Possession)?;
                    PreparedProtocolBodyPlay::prepare_usb_ethernet(
                        artifact,
                        retained_session.take().expect("retained body"),
                        body_play_sequence,
                        usb,
                        crate::protocol_host_calls::EthernetFrameAdmission {
                            ready: network.ready,
                            table: possession.table,
                            handle: possession.handle,
                            claim: possession.claim,
                            random_seed: network.random_seed,
                            timestamp: network.timestamp,
                        },
                    )
                }
                None => PreparedProtocolBodyPlay::prepare_usb_duplex(
                    artifact,
                    retained_session.take().expect("retained body"),
                    body_play_sequence,
                    usb,
                ),
            };
            #[cfg(not(all(target_arch = "x86_64", feature = "native-owner-network")))]
            let result = PreparedProtocolBodyPlay::prepare_usb_duplex(
                artifact,
                retained_session.take().expect("retained body"),
                body_play_sequence,
                usb,
            );
            result
        };
        match result {
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

fn issue_storage_pair(
    preparation: NativeStoragePreparation,
    plan: &Plan,
    active: &ActivePlayIdentity,
    work_units: u64,
) -> Result<(NativeProtocolPossession, NativeProtocolPossession), NativeProtocolPreparationReason> {
    use crate::storage_base::transaction_factory::{
        AUTHORIZE_IMPLEMENTATION, SETTLE_IMPLEMENTATION,
    };
    let authorize = preparation
        .authorize
        .issue(plan, active, &AUTHORIZE_IMPLEMENTATION.into(), work_units)
        .map_err(NativeProtocolPreparationReason::Possession)?;
    let settle = preparation
        .settle
        .issue(plan, active, &SETTLE_IMPLEMENTATION.into(), work_units)
        .map_err(NativeProtocolPreparationReason::Possession)?;
    Ok((authorize, settle))
}
