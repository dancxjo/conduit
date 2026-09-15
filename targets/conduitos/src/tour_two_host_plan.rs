//! Exact two-Host native realization of the shared `hello-across` Tour Form.

use alloc::{collections::BTreeMap, format, vec::Vec};
use conduit_core::{
    ActivePlayIdentity, BaseImplementationId, BaseInstanceId, CapabilityId, GearId,
    LineAvailability, LineAvailabilitySign, LineContinuation, LineContract, LineDuplex, LineId,
    LineOffer, LineOrdering, LineReliability, LineScope, LineSecurity, LineTrafficShape,
    LinkAuthorityReference, LinkBinding, LinkBindingId, LinkCredentialReference, LinkEndpoint,
    LinkEndpointId, LinkLimits, Plan, SignId, bind_active_play,
};
use conduit_plan_lowering::lowering::{RemoteCordDirection, lower_plan_fragment};
use conduit_planner::{PlacementChoice, PlacementChoices, PlanningOptions};

use crate::{
    identity::{BootIdentities, derive_tour_peer},
    offer::HostOffer,
    ordinary_plan::PreparationError,
};

const FORM_NAME: &str = "hello-across";
const LINE_BASE: &str = "conduit.base/conduitos-process-line@1";
const PLAY_SEQUENCE: u64 = 0;

pub struct PreparedTourTwoHostPlan {
    pub plan: Plan,
    pub source_fragment_id: conduit_core::FragmentId,
    pub sink_fragment_id: conduit_core::FragmentId,
    pub source_active: ActivePlayIdentity,
    pub sink_active: ActivePlayIdentity,
    pub line_id: LineId,
    pub(crate) source: crate::tour_two_host_kernel::SourceKernel,
    pub(crate) sink: crate::tour_two_host_kernel::SinkKernel,
}

pub fn prepare(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedTourTwoHostPlan, PreparationError> {
    let source =
        conduit_tour_model::tour_stage_source(3, 0).map_err(|_| PreparationError::FormRejected)?;
    let form = crate::ordinary_form::checked_expanded_text_form_named(&source, FORM_NAME)?;
    if form.gears.len() != 2 || form.connections.len() != 1 {
        return Err(PreparationError::FormRejected);
    }
    let peer_identities = derive_tour_peer(identities);
    let peer_offer = HostOffer::new(
        &peer_identities,
        build_id,
        offer.cpu_features,
        offer.runtime_arena_bytes,
    );
    let source_advertisement = crate::ordinary_plan::advertisement(identities, offer, build_id)?;
    let sink_advertisement =
        crate::ordinary_plan::advertisement(&peer_identities, &peer_offer, build_id)?;
    let hosts = [source_advertisement, sink_advertisement];
    let line_offer = line_offer(&hosts[0], &hosts[1]);
    let line_id = line_offer.line_id.clone();
    let placements = placements(&form, &hosts)?;
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &form,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from(LINE_BASE),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_text::MAX_TEXT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[line_offer],
        },
    )
    .map_err(|_| PreparationError::PlanRejected)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 2 {
        return Err(PreparationError::PlanRejected);
    }
    let source_fragment = fragment_for_kind(&plan, conduit_text::TEXT_LITERAL_KIND)?;
    let sink_fragment = fragment_for_kind(&plan, conduit_semantic_catalog::TEXT_PRESENTATION_KIND)?;
    if source_fragment.host_id == sink_fragment.host_id
        || source_fragment.boot_id == sink_fragment.boot_id
    {
        return Err(PreparationError::PlanRejected);
    }
    let source_lowered =
        lower_plan_fragment(source_fragment).map_err(|_| PreparationError::LoweringRejected)?;
    let sink_lowered =
        lower_plan_fragment(sink_fragment).map_err(|_| PreparationError::LoweringRejected)?;
    exact_endpoint(&source_lowered, RemoteCordDirection::Egress)?;
    exact_endpoint(&sink_lowered, RemoteCordDirection::Ingress)?;
    let source_kernel = crate::tour_two_host_kernel::SourceKernel::prepare(
        source_fragment,
        &source_lowered,
        "hello across one Cord",
    )
    .map_err(|_| PreparationError::KernelRejected)?;
    let sink_kernel =
        crate::tour_two_host_kernel::SinkKernel::prepare(sink_fragment, &sink_lowered)
            .map_err(|_| PreparationError::KernelRejected)?;
    Ok(PreparedTourTwoHostPlan {
        source_fragment_id: source_fragment.fragment_id.clone(),
        sink_fragment_id: sink_fragment.fragment_id.clone(),
        source_active: bind_active_play(
            &plan.plan_id,
            &source_fragment.host_id,
            &source_fragment.boot_id,
            PLAY_SEQUENCE,
        ),
        sink_active: bind_active_play(
            &plan.plan_id,
            &sink_fragment.host_id,
            &sink_fragment.boot_id,
            PLAY_SEQUENCE,
        ),
        line_id,
        source: source_kernel,
        sink: sink_kernel,
        plan,
    })
}

fn placements(
    form: &conduit_form::ExpandedCanonicalForm,
    hosts: &[conduit_core::HostAdvertisement; 2],
) -> Result<PlacementChoices, PreparationError> {
    let mut by_gear = BTreeMap::new();
    for gear in &form.gears {
        let (host, capability) = match gear.kind_id.as_str() {
            conduit_text::TEXT_LITERAL_KIND => (&hosts[0], "conduitos/text-literal@1"),
            conduit_semantic_catalog::TEXT_PRESENTATION_KIND => {
                (&hosts[1], "conduitos/presentation-text@1")
            }
            _ => return Err(PreparationError::PlacementRejected),
        };
        by_gear.insert(
            GearId::from(gear.gear_id.as_str()),
            PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: CapabilityId::from(capability),
            },
        );
    }
    Ok(PlacementChoices { by_gear })
}

fn fragment_for_kind<'a>(
    plan: &'a Plan,
    kind: &str,
) -> Result<&'a conduit_core::PlanFragment, PreparationError> {
    plan.fragments
        .iter()
        .find(|fragment| {
            fragment
                .placements
                .iter()
                .any(|placement| placement.kind_id.as_str() == kind)
        })
        .ok_or(PreparationError::PlanRejected)
}

fn exact_endpoint(
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    direction: RemoteCordDirection,
) -> Result<conduit_kernel::RemoteEndpointId, PreparationError> {
    let matches = lowered
        .remote_endpoints
        .iter()
        .filter(|endpoint| endpoint.direction == direction)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [endpoint] => Ok(endpoint.endpoint),
        _ => Err(PreparationError::LoweringRejected),
    }
}

fn line_offer(
    source: &conduit_core::HostAdvertisement,
    sink: &conduit_core::HostAdvertisement,
) -> LineOffer {
    let line_id = LineId::from(format!(
        "tour/native-line/{}/{}",
        source.host_id.as_str(),
        sink.host_id.as_str()
    ));
    let binding_id = LinkBindingId::from(format!("{}/binding", line_id.as_str()));
    LineOffer {
        line_id: line_id.clone(),
        binding: LinkBinding {
            binding_id: binding_id.clone(),
            source: LinkEndpoint {
                host_id: source.host_id.clone(),
                boot_id: source.boot_id.clone(),
                endpoint_id: LinkEndpointId::from("tour/native-source/egress"),
            },
            sink: LinkEndpoint {
                host_id: sink.host_id.clone(),
                boot_id: sink.boot_id.clone(),
                endpoint_id: LinkEndpointId::from("tour/native-sink/ingress"),
            },
            base: BaseImplementationId::from(LINE_BASE),
            base_instance_id: BaseInstanceId::from(format!("{}/instance", line_id.as_str())),
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: conduit_text::MAX_TEXT_BYTES,
                maximum_buffered_bytes: conduit_text::MAX_TEXT_BYTES,
                maximum_frame_bytes: conduit_text::MAX_TEXT_BYTES,
            },
        },
        contract: LineContract {
            scope: LineScope::Process,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::Simplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::ProcessBoundary,
        },
        availability: LineAvailabilitySign {
            line_id,
            binding_id,
            availability: LineAvailability::Ready,
            sign_id: SignId::from("tour/native-line/ready"),
        },
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;
    use crate::{
        machine::{BaseError, SerialBase},
        offer::CpuFeatures,
    };

    struct Serial(Vec<Vec<u8>>);

    impl SerialBase for Serial {
        fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
            self.0.push(bytes.into());
            Ok(())
        }

        fn presentation_count(&self) -> u32 {
            self.0.len() as u32
        }
    }

    #[test]
    fn shared_form_runs_as_two_distinct_native_fragment_plays_over_one_line() {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            256 * 1024,
        );
        let mut prepared = prepare(&identities, &offer, "build").unwrap();
        assert!(conduit_core::verify_plan(&prepared.plan));
        assert_eq!(prepared.plan.fragments.len(), 2);
        assert_ne!(prepared.source_fragment_id, prepared.sink_fragment_id);
        assert_ne!(
            prepared.source_active.active_play_id,
            prepared.sink_active.active_play_id
        );
        assert_eq!(
            prepared
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.connections)
                .filter(|connection| connection
                    .selected_line
                    .as_ref()
                    .is_some_and(|line| line.line_id == prepared.line_id))
                .count(),
            2,
        );
        let mut serial = Serial(Vec::new());
        let evidence = crate::tour_two_host::run(&mut prepared, &mut serial).unwrap();
        assert_eq!(serial.0, [b"hello across one Cord".to_vec()]);
        assert_eq!(evidence.run.logical_operations, 2);
        assert_eq!(evidence.run.serial_presentations, 1);
        let multi_host = evidence.multi_host.unwrap();
        assert_eq!(
            multi_host.source_fragment_id,
            prepared.source_fragment_id.as_str()
        );
        assert_eq!(
            multi_host.sink_fragment_id,
            prepared.sink_fragment_id.as_str()
        );
        assert_ne!(
            multi_host.source_active_play_id,
            multi_host.sink_active_play_id
        );
        assert_eq!(multi_host.line_id, prepared.line_id.as_str());
        assert_eq!(multi_host.transferred_values, 1);
    }
}
