//! Selected single-call ownership for a sensor-independent physical source.
//! The ordinary kernel HostCallBack remains the execution Back. No polling loop,
//! USB boot operation, network setup, or Body membership is introduced here.
use conduit_core::{
    self as c, BaseCapabilityHandle, BaseCapabilityTable, BaseOperationClaim, BoundBaseOperation,
};
use conduit_environment_measurement::source_observation::{
    ObservationContract, PreparedObservationCodec, SourceObservation, MAXIMUM_BYTES,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::selected_operation::{
    bind_selected_operation, SelectedCallRefusal, SelectedOperationContract, SelectedOperationPlan,
};

pub const HOST_CALL: &str = "environment/read-environmental-source";
pub const RESOURCE_CLASS: &str = "conduit.environment/source-session@1";
pub const AUTHORITY_CONTRACT: &str = "conduit.environment/source-read@1";
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Binding(SelectedCallRefusal),
    Capability(c::BaseCapabilityRefusal),
    Possession,
    Request,
    Capacity,
    Provider,
    Encoding(c::StructuredInfoRefusal),
}
/// The actual initialized provider owns one exclusive bounded transport session,
/// its checked Host-configuration/build identity and read deadline. It must
/// quiesce on cancellation, supply the producing Host's actual monotonic clock,
/// and retain source-reported clock coordinates without inventing MCU Boot.
/// Its Base-owned SourceContinuity must survive child Plays. A first cached
/// sample establishes a baseline only; bounded acquisition waits for a counter
/// advance. NoNewObservation cannot refresh a Face. Counter/uptime rollback
/// invalidates the physical session and requires cancellation/re-admission.
pub trait EnvironmentalSourceProvider {
    /// Root preparation only, before resource reservation/Play. Native owners
    /// revalidate the same Host/Boot against the current installed inventory.
    fn prepare_for_host(&mut self, _host: &c::HostAdvertisement) -> Result<(), Refusal> {
        Ok(())
    }

    fn observe(&mut self)
        -> impl core::future::Future<Output = Result<SourceObservation, Refusal>>;
}
pub struct EnvironmentalSourceHostCall<P> {
    scope: super::environmental_publication::PublicationScope,
    fragment: c::FragmentId,
    placement: c::PlacementId,
    operation: BoundBaseOperation,
    provider: P,
    node: NodeId,
    next_request: u32,
    codec: PreparedObservationCodec,
}
impl<P: EnvironmentalSourceProvider> EnvironmentalSourceHostCall<P> {
    /// # Safety
    /// Root must retain the actual exclusive initialized provider and checked
    /// configuration independently of received Plan bytes. Its finite transport,
    /// cancellation and memory bounds must already be admitted. A claimed ID or
    /// a diagnostic packet cannot establish possession at this native boundary.
    #[allow(clippy::result_large_err)] // Failed preparation must return every affine owner.
    pub unsafe fn bind_selected(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        provider: P,
        contract: &ObservationContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<
        Self,
        (
            BaseCapabilityTable,
            BaseCapabilityHandle,
            BaseOperationClaim,
            P,
            Refusal,
        ),
    > {
        let prepared = (|| -> Result<_, Refusal> {
            let mut inspections = table.inspections();
            let entry = inspections.next().ok_or(Refusal::Possession)?;
            if inspections.next().is_some()
                || entry.scope.authority_contract_id.as_str() != AUTHORITY_CONTRACT
                || entry.scope.maximum_in_flight != 1
                || entry.scope.maximum_operations == 0
                || entry.scope.maximum_parameter_bytes < 1
                || entry.scope.maximum_result_bytes < MAXIMUM_BYTES as u32
                || claim.parameter_bytes != 1
                || claim.work_units != 1
            {
                return Err(Refusal::Possession);
            }
            drop(inspections);
            let gear = selected
                .fragment
                .placements
                .iter()
                .find(|p| &p.placement_id == selected.placement_id)
                .ok_or(Refusal::Possession)?;
            if gear.resources.len() != 1
                || gear.resources[0].class_id.as_str() != RESOURCE_CLASS
                || gear.resources[0].units != 1
            {
                return Err(Refusal::Possession);
            }
            let kind = contract.kind(false).map_err(Refusal::Encoding)?;
            let scope = super::environmental_publication::PublicationScope {
                host: selected.fragment.host_id.clone(),
                boot: selected.fragment.boot_id.clone(),
                plan: selected.fragment.plan_id.clone(),
                play: selected.active.active_play_id.clone(),
                source: selected.fragment.source_document_id.clone(),
                checked: selected.fragment.checked_plot_id.clone(),
                expanded: selected.fragment.expanded_plot_id.clone(),
            };
            let fragment = selected.fragment.fragment_id.clone();
            let placement = selected.placement_id.clone();
            let node = bind_selected_operation(
                &table,
                &claim,
                SelectedOperationContract {
                    kind: &kind,
                    call: HOST_CALL,
                    input_bytes: 1,
                    output_bytes: MAXIMUM_BYTES as u32,
                    resource_bytes: MAXIMUM_BYTES as u64,
                },
                selected,
            )
            .map_err(Refusal::Binding)?;
            let codec = PreparedObservationCodec::prepare(contract).map_err(Refusal::Encoding)?;
            Ok((scope, fragment, placement, node, codec))
        })();
        let (scope, fragment, placement, node, codec) = match prepared {
            Ok(prepared) => prepared,
            Err(reason) => return Err((table, handle, claim, provider, reason)),
        };
        let operation = match BoundBaseOperation::bind(table, handle, claim) {
            Ok(operation) => operation,
            Err((table, handle, claim, reason)) => {
                return Err((table, handle, claim, provider, Refusal::Capability(reason)))
            }
        };
        Ok(Self {
            scope,
            fragment,
            placement,
            operation,
            provider,
            node,
            next_request: 0,
            codec,
        })
    }
    /// Bind the actual retained provider to its exact verified local Source
    /// edge before constructing a publication endpoint. Metadata alone cannot
    /// construct this witness: this method requires the native bound owner.
    pub fn prepare_publication(
        &self,
        selected: SelectedOperationPlan<'_>,
        maximum_events: u32,
    ) -> Result<super::environmental_publication::EnvironmentalPublicationBack, Refusal> {
        if self.next_request != 0
            || self.operation.inspection().lifecycle != c::CapabilityLifecycle::Issued
            || maximum_events > self.operation.inspection().scope.maximum_operations
        {
            return Err(Refusal::Possession);
        }
        let fragment = selected.fragment;
        if !c::verify_plan_fragment(fragment)
            || conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
                .map_err(|_| Refusal::Possession)?
                != *selected.lowered
            || fragment.fragment_id != self.fragment
            || fragment.host_id != self.scope.host
            || fragment.boot_id != self.scope.boot
            || fragment.plan_id != self.scope.plan
            || fragment.source_document_id != self.scope.source
            || fragment.checked_plot_id != self.scope.checked
            || fragment.expanded_plot_id != self.scope.expanded
            || selected.active.active_play_id != self.scope.play
            || selected.active.host_id != self.scope.host
            || selected.active.boot_id != self.scope.boot
            || selected.active.plan_id != self.scope.plan
            || c::bind_active_play(
                &self.scope.plan,
                &self.scope.host,
                &self.scope.boot,
                selected.active.play_sequence,
            ) != *selected.active
        {
            return Err(Refusal::Possession);
        }
        let contract = ObservationContract::prepare().map_err(Refusal::Encoding)?;
        let kind = contract.kind(true).map_err(Refusal::Encoding)?;
        let sink = fragment
            .placements
            .iter()
            .find(|g| &g.placement_id == selected.placement_id)
            .ok_or(Refusal::Possession)?;
        if sink.kind_id != kind.kind_id
            || sink.kind_contract_revision != kind.kind_contract_revision
            || sink.inputs != kind.inputs
            || sink.outputs != kind.outputs
            || sink.semantic_contract != kind.semantic_contract()
            || !sink.configuration.is_empty()
            || sink.host_id != self.scope.host
            || sink.boot_id != self.scope.boot
            || !sink.host_calls.is_empty()
        {
            return Err(Refusal::Possession);
        }
        check_publication_edge(
            &self.placement,
            selected.placement_id,
            &kind.inputs[0].value_kind,
            &fragment.connections,
        )?;
        super::environmental_publication::EnvironmentalPublicationBack::prepare(
            self.scope.clone(),
            maximum_events,
        )
        .map_err(Refusal::Encoding)
    }
    pub async fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, Refusal> {
        if node != self.node
            || call != HostCallId(0)
            || request != RequestId(self.next_request)
            || !input.is_empty()
        {
            return Err(Refusal::Request);
        }
        if output.len() < MAXIMUM_BYTES {
            return Err(Refusal::Capacity);
        }
        let next = self.next_request.checked_add(1).ok_or(Refusal::Request)?;
        let lease = self.operation.authorize().map_err(Refusal::Capability)?;
        self.next_request = next;
        let pending = Pending {
            operation: &mut self.operation,
            lease: Some(lease),
        };
        let result = match self.provider.observe().await {
            Ok(value) => self
                .codec
                .encode(value)
                .map_err(Refusal::Encoding)
                .map(|bytes| {
                    output[..bytes.len()].copy_from_slice(bytes);
                    bytes.len()
                }),
            Err(reason) => Err(reason),
        };
        pending
            .complete(result.as_ref().copied().unwrap_or(0) as u32)
            .map_err(Refusal::Capability)?;
        result
    }
    /// Return the same initialized source only after its selected operation
    /// has been revoked and quiesced. Continuity and the USB session remain in
    /// P across the next child Play; refusal leaves this retained slot intact.
    pub fn retire_retained(slot: &mut Option<Self>) -> Result<P, Refusal> {
        let owner = slot.as_ref().ok_or(Refusal::Possession)?;
        let inspection = owner.operation.inspection();
        if inspection.lifecycle != c::CapabilityLifecycle::Revoked
            || inspection.in_flight_operations != 0
        {
            return Err(Refusal::Possession);
        }
        Ok(slot.take().expect("retained owner checked").provider)
    }
    pub fn revoke(&mut self) -> Result<(), Refusal> {
        self.operation.revoke().map_err(Refusal::Capability)
    }
}
struct Pending<'a> {
    operation: &'a mut BoundBaseOperation,
    lease: Option<c::BaseOperationLease>,
}
impl Pending<'_> {
    fn complete(mut self, bytes: u32) -> Result<(), c::BaseCapabilityRefusal> {
        self.operation
            .complete(self.lease.as_ref().expect("pending lease").clone(), bytes)?;
        self.lease = None;
        Ok(())
    }
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if self.lease.is_some() {
            let _ = self.operation.revoke();
        }
    }
}

fn check_publication_edge(
    source: &c::PlacementId,
    sink: &c::PlacementId,
    value_kind: &c::KindId,
    connections: &[c::PlannedConnection],
) -> Result<(), Refusal> {
    let mut incoming = connections
        .iter()
        .filter(|edge| &edge.sink_placement_id == sink);
    let edge = incoming.next().ok_or(Refusal::Possession)?;
    if incoming.next().is_some()
        || &edge.source_placement_id != source
        || edge.source_port_id.as_str() != "observation"
        || edge.sink_port_id.as_str() != "value"
        || &edge.value_kind != value_kind
        || edge.selected_line.is_some()
        || !edge.admitted_lines.is_empty()
        || edge.resource.is_some()
        || edge.abnormal_kind.is_some()
        || edge.track != c::ConnectionTrack::Payload
        || edge.temporal != (c::PortTemporal::Flow { closes: true })
        || edge.item_capacity != 1
        || edge.byte_capacity < MAXIMUM_BYTES as u32
    {
        return Err(Refusal::Possession);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    #[test]
    fn authored_source_accepts_bare_empty_trigger_with_the_existing_one_byte_bound() {
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profile = conduit_plot::ProfileCatalog::new();
        install_catalog(&mut startup, &mut profile).unwrap();
        let source = alloc::format!(
            "plot capture (\n >> begin: Empty...| <= 1B\n) {{\n source: {}\n publish: {}\n begin >> source.request\n source.observation >> publish.value\n}}\n",
            conduit_environment_measurement::source_observation::SOURCE_KIND,
            conduit_environment_measurement::source_observation::PUBLICATION_KIND,
        );
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&source),
            &startup,
        )
        .expect("a bare Empty request must connect to the actual environmental Source");
        let authored =
            conduit_plot::expand_canonical_plot_for_authoring(&checked, "capture", &profile)
                .expect("the real Source/publication edge must expand with the same checked Types");
        assert_eq!(authored.input_bindings.len(), 1);
        let source = authored
            .expanded
            .gears
            .iter()
            .find(|gear| {
                gear.kind_id.as_str()
                    == conduit_environment_measurement::source_observation::SOURCE_KIND
            })
            .unwrap();
        // Validate the actual wire input selected by the checked expansion,
        // rather than merely comparing a Kind constant with itself. This is
        // the primitive validation used by the ordinary lowered Fore boundary.
        c::validate_primitive_info(source.inputs[0].value_kind.as_str(), &[])
            .expect("the actual selected Source input must accept a bare Empty value");
    }
    #[test]
    fn actual_source_edge_rejects_substitution_duplicate_or_underreserved_routes() {
        let source: c::PlacementId = "source/test".into();
        let sink: c::PlacementId = "sink/test".into();
        let contract = ObservationContract::prepare().unwrap();
        let kind = contract.kind(true).unwrap();
        let value = kind.inputs[0].value_kind.clone();
        let edge = c::PlannedConnection {
            connection_id: "edge/test".into(),
            source_placement_id: source.clone(),
            source_port_id: "observation".into(),
            sink_placement_id: sink.clone(),
            sink_port_id: "value".into(),
            value_kind: value.clone(),
            resource: None,
            abnormal_kind: None,
            track: c::ConnectionTrack::Payload,
            temporal: c::PortTemporal::Flow { closes: true },
            pressure_policy: Default::default(),
            selected_line: None,
            admitted_lines: vec![],
            item_capacity: 1,
            byte_capacity: MAXIMUM_BYTES as u32,
        };
        assert!(
            check_publication_edge(&source, &sink, &value, core::slice::from_ref(&edge)).is_ok()
        );
        let mut wrong = edge.clone();
        wrong.source_placement_id = "source/other".into();
        assert_eq!(
            check_publication_edge(&source, &sink, &value, &[wrong]),
            Err(Refusal::Possession)
        );
        let mut wrong = edge.clone();
        wrong.byte_capacity -= 1;
        assert_eq!(
            check_publication_edge(&source, &sink, &value, &[wrong]),
            Err(Refusal::Possession)
        );
        assert_eq!(
            check_publication_edge(&source, &sink, &value, &[edge.clone(), edge]),
            Err(Refusal::Possession)
        );
    }
}

/// Make the same checked semantic Kinds available to ordinary Source checking.
/// Catalog entries describe meaning; they install no provider or capability.
pub fn install_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let contract = ObservationContract::prepare()
        .map_err(|reason| alloc::format!("environmental contract: {reason:?}"))?;
    // Expose the same exact observation contract at an authored Plot Fore.
    // Installed collectors need not invent a publication Back to export it.
    startup.insert_value_kind_alias(
        "EnvironmentalObservation",
        contract
            .value_type()
            .profile()
            .map_err(|reason| alloc::format!("environmental Fore: {reason:?}"))?
            .value_kind()
            .clone(),
    )?;
    for publication in [false, true] {
        let kind = contract
            .kind(publication)
            .map_err(|reason| alloc::format!("environmental Kind: {reason:?}"))?;
        let id = kind.kind_id.as_str();
        startup.insert(conduit_plot::KindSignature {
            kind: id.into(),
            startup_parameters: alloc::vec![],
        })?;
        startup.insert_fore(id, kind.checked_front())?;
        profiles
            .insert_kind(kind)
            .map_err(|reason| alloc::format!("environmental profile: {reason:?}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod fore_catalog_tests {
    #[test]
    fn source_observation_can_leave_through_ordinary_typed_fore() {
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profiles = conduit_plot::ProfileCatalog::new();
        super::install_catalog(&mut startup, &mut profiles).unwrap();
        let source = include_str!("../../profiles/source-observation.conduit");
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(source),
            &startup,
        )
        .unwrap();
        let expanded =
            conduit_plot::expand_canonical_plot_for_authoring(&checked, "sample", &profiles)
                .unwrap();
        assert_eq!(expanded.expanded.gears.len(), 1);
        assert_eq!(
            expanded.expanded.gears[0].kind_id.as_str(),
            conduit_environment_measurement::source_observation::SOURCE_KIND
        );
        assert_eq!(expanded.input_bindings.len(), 1);
        assert_eq!(expanded.output_bindings.len(), 1);
    }
}
