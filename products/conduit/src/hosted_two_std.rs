//! Exact two-std-Host realization of one authored text Form over an admitted Line.

mod transport;

use std::{collections::BTreeMap, io::Write};

use conduit_core::{
    bind_active_play, BaseImplementationId, BaseInstanceId, BootId, CapabilityId, FragmentId,
    GearId, HostId, LineAvailability, LineAvailabilitySign, LineContinuation, LineContract,
    LineDuplex, LineId, LineOffer, LineOrdering, LineReliability, LineScope, LineSecurity,
    LineTrafficShape, LinkAuthorityReference, LinkBinding, LinkBindingId, LinkCredentialReference,
    LinkEndpoint, LinkEndpointId, LinkLimits, OfferGeneration, PlanId, SignId,
};
use conduit_planner::{PlacementChoice, PlacementChoices};
use conduit_std_host::{InstalledRemoteFragment, StdHost, StdHostConfig};

use crate::{
    form_source,
    product_execution::{ProductExecutionContext, ProductRuntime},
};

const SOURCE_HOST: &str = "tour/std-source";
const SINK_HOST: &str = "tour/std-sink";
const SOURCE_BOOT: &str = "tour/std-source/boot-1";
const SINK_BOOT: &str = "tour/std-sink/boot-1";
// SessionBinding currently seals the first Play of each exact fragment.
const PLAY_SEQUENCE: u64 = 0;

pub struct HostedTwoHostExecution {
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub expanded_form_id: conduit_core::ExpandedFormId,
    pub plan_id: PlanId,
    pub source_fragment_id: FragmentId,
    pub sink_fragment_id: FragmentId,
    pub source_active_play_id: conduit_core::ActivePlayId,
    pub sink_active_play_id: conduit_core::ActivePlayId,
    pub line_id: LineId,
    pub transferred_values: u32,
    pub result: String,
}

pub fn execute_hosted_two_std_form(
    source: &str,
    output: &mut impl Write,
) -> Result<HostedTwoHostExecution, String> {
    let form = form_source::parse(source)?.expand_entry()?;
    let source_host = host(SOURCE_HOST, SOURCE_BOOT);
    let sink_host = host(SINK_HOST, SINK_BOOT);
    let line_offer = line_offer(&source_host, &sink_host);
    let line_id = line_offer.line_id.clone();
    let context = ProductExecutionContext::new(
        vec![
            source_host.advertisement().clone(),
            sink_host.advertisement().clone(),
        ],
        vec![
            ProductRuntime::std(source_host),
            ProductRuntime::std(sink_host),
        ],
        vec![
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        vec![line_offer],
        Vec::new(),
    )?;
    let plan = context.plan_with_placements(&form, &placements(&form)?)?;
    let source_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == SOURCE_HOST)
        .ok_or_else(|| "two-Host Plan has no source fragment".to_string())?;
    let sink_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == SINK_HOST)
        .ok_or_else(|| "two-Host Plan has no sink fragment".to_string())?;
    let source_advertisement = context
        .advertisements()
        .iter()
        .find(|host| host.host_id.as_str() == SOURCE_HOST)
        .ok_or_else(|| "two-Host source advertisement is missing".to_string())?;
    let sink_advertisement = context
        .advertisements()
        .iter()
        .find(|host| host.host_id.as_str() == SINK_HOST)
        .ok_or_else(|| "two-Host sink advertisement is missing".to_string())?;
    let source_runtime =
        InstalledRemoteFragment::prepare(source_advertisement, source_fragment, PLAY_SEQUENCE)?;
    let sink_runtime =
        InstalledRemoteFragment::prepare(sink_advertisement, sink_fragment, PLAY_SEQUENCE)?;
    let source_endpoint = transport::exact_endpoint(
        &source_runtime,
        conduit_plan_lowering::lowering::RemoteCordDirection::Egress,
    )?;
    let sink_endpoint = transport::exact_endpoint(
        &sink_runtime,
        conduit_plan_lowering::lowering::RemoteCordDirection::Ingress,
    )?;
    let (transfer, sink_output) =
        transport::execute_line(source_runtime, source_endpoint, sink_runtime, sink_endpoint)?;
    output
        .write_all(&sink_output)
        .map_err(|error| error.to_string())?;
    let result = core::str::from_utf8(&transfer.bytes)
        .map_err(|_| "two-Host text is not UTF-8")?
        .to_string();
    Ok(HostedTwoHostExecution {
        source_document_id: form.source_document_id,
        checked_form_id: form.checked_form_id,
        expanded_form_id: form.expanded_form_id,
        plan_id: plan.plan_id.clone(),
        source_fragment_id: source_fragment.fragment_id.clone(),
        sink_fragment_id: sink_fragment.fragment_id.clone(),
        source_active_play_id: bind_active_play(
            &plan.plan_id,
            &source_fragment.host_id,
            &source_fragment.boot_id,
            PLAY_SEQUENCE,
        )
        .active_play_id,
        sink_active_play_id: bind_active_play(
            &plan.plan_id,
            &sink_fragment.host_id,
            &sink_fragment.boot_id,
            PLAY_SEQUENCE,
        )
        .active_play_id,
        line_id,
        transferred_values: 1,
        result,
    })
}

fn line_offer(source: &StdHost, sink: &StdHost) -> LineOffer {
    let line_id = LineId::from(format!(
        "tour-line/{}/{}",
        source.advertisement().host_id.as_str(),
        sink.advertisement().host_id.as_str()
    ));
    let binding_id = LinkBindingId::from(format!("{}/binding", line_id.as_str()));
    LineOffer {
        line_id: line_id.clone(),
        binding: LinkBinding {
            binding_id: binding_id.clone(),
            source: LinkEndpoint {
                host_id: source.advertisement().host_id.clone(),
                boot_id: source.advertisement().boot_id.clone(),
                endpoint_id: LinkEndpointId::from("tour/std-source/egress"),
            },
            sink: LinkEndpoint {
                host_id: sink.advertisement().host_id.clone(),
                boot_id: sink.advertisement().boot_id.clone(),
                endpoint_id: LinkEndpointId::from("tour/std-sink/ingress"),
            },
            base: BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            base_instance_id: BaseInstanceId::from("tour/std-line/in-process-instance"),
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: conduit_text::MAX_TEXT_BYTES,
                maximum_buffered_bytes: conduit_text::MAX_TEXT_BYTES,
                maximum_frame_bytes: 2_048,
            },
        },
        contract: LineContract {
            scope: LineScope::LocalNetwork,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::PlaintextNetwork,
        },
        availability: LineAvailabilitySign {
            line_id,
            binding_id,
            availability: LineAvailability::Ready,
            sign_id: SignId::from("tour/std-line/ready"),
        },
    }
}

fn host(host_id: &str, boot_id: &str) -> StdHost {
    StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from(host_id),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
    })
}

fn placements(form: &conduit_form::ExpandedCanonicalForm) -> Result<PlacementChoices, String> {
    let mut by_gear = BTreeMap::new();
    for gear in &form.gears {
        let (host, capability) = match gear.kind_id.as_str() {
            conduit_text::TEXT_LITERAL_KIND => (SOURCE_HOST, "text-literal-v1"),
            conduit_semantic_catalog::TEXT_PRESENTATION_KIND => (SINK_HOST, "presentation-text-v1"),
            kind => return Err(format!("two-Host Tour does not implement kind '{kind}'")),
        };
        by_gear.insert(
            GearId::from(gear.gear_id.as_str()),
            PlacementChoice {
                host_id: HostId::from(host),
                capability_id: CapabilityId::from(capability),
            },
        );
    }
    Ok(PlacementChoices { by_gear })
}
