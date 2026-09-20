//! Exact finite session machines for one selected dynamic shared-pool member.
//!
//! Selection remains owned by the common pool kernel. This module only
//! instantiates the semantic port sessions already named by validated
//! selection evidence and admitted by the immutable Plan.

use conduit_core::{
    HostId, PlacementId, Plan, PoolMemberSessionDirection, PoolSelectionDisposition,
    PoolSelectionEvidence, PortId,
};
use conduit_wire::{SessionBinding, SessionMachine, SessionRole};

const MAXIMUM_POOL_MEMBER_SESSIONS: usize = 16;

pub struct PoolMemberSession {
    pub direction: PoolMemberSessionDirection,
    pub port_id: PortId,
    binding: SessionBinding,
    machine: SessionMachine,
}

impl PoolMemberSession {
    pub fn binding(&self) -> &SessionBinding {
        &self.binding
    }

    pub fn machine(&self) -> &SessionMachine {
        &self.machine
    }

    pub fn machine_mut(&mut self) -> &mut SessionMachine {
        &mut self.machine
    }
}

pub struct PoolMemberSessions {
    selection: PoolSelectionEvidence,
    sessions: Vec<PoolMemberSession>,
}

impl PoolMemberSessions {
    pub fn prepare(
        plan: &Plan,
        selection: PoolSelectionEvidence,
        consumer_placement_id: &PlacementId,
        local_host_id: &HostId,
    ) -> Result<Self, String> {
        selection
            .validate(plan)
            .map_err(|error| format!("validate pool member selection: {error:?}"))?;
        if selection.disposition != PoolSelectionDisposition::Selected {
            return Err("pool member sessions require one selected realization".into());
        }
        let pool = plan
            .fragments
            .first()
            .and_then(|fragment| {
                fragment
                    .shared_pools
                    .iter()
                    .find(|pool| pool.pool_id == selection.pool_id)
            })
            .ok_or_else(|| "selected shared pool is absent from the Plan".to_string())?;
        let realization = pool
            .realization_envelope
            .get(usize::from(selection.selected_realization.ok_or_else(
                || "selected pool realization is absent".to_string(),
            )?))
            .ok_or_else(|| "selected pool realization is outside the Plan".to_string())?;
        if &realization.host_id != local_host_id {
            return Err("selected pool realization belongs to another Host".into());
        }
        let session_count = pool
            .member_front
            .inputs()
            .len()
            .checked_add(pool.member_front.outputs().len())
            .ok_or_else(|| "pool member session count overflowed".to_string())?;
        if session_count == 0 || session_count > MAXIMUM_POOL_MEMBER_SESSIONS {
            return Err("pool member front exceeds the finite std session bound".into());
        }

        let mut sessions = Vec::with_capacity(session_count);
        for (direction, ports, role) in [
            (
                PoolMemberSessionDirection::Input,
                pool.member_front.inputs(),
                SessionRole::Sink,
            ),
            (
                PoolMemberSessionDirection::Output,
                pool.member_front.outputs(),
                SessionRole::Source,
            ),
        ] {
            for port in ports {
                let binding = SessionBinding::from_selected_pool_operation(
                    plan,
                    &selection,
                    consumer_placement_id,
                    direction,
                    &port.port_id,
                )
                .map_err(|error| format!("bind selected pool member port: {error:?}"))?;
                let machine = SessionMachine::new(binding.clone(), role)
                    .map_err(|error| format!("prepare selected pool member session: {error:?}"))?;
                sessions.push(PoolMemberSession {
                    direction,
                    port_id: port.port_id.clone(),
                    binding,
                    machine,
                });
            }
        }
        Ok(Self {
            selection,
            sessions,
        })
    }

    pub fn selection(&self) -> &PoolSelectionEvidence {
        &self.selection
    }

    pub fn iter(&self) -> impl Iterator<Item = &PoolMemberSession> {
        self.sessions.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut PoolMemberSession> {
        self.sessions.iter_mut()
    }

    pub fn session_for_binding_mut(
        &mut self,
        binding: &conduit_wire::SessionIdentity<'_>,
    ) -> Option<&mut PoolMemberSession> {
        self.sessions
            .iter_mut()
            .find(|session| session.binding.identity() == *binding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        mandatory_sign_storage_requirement, seal_plan, AdmittedLine, ArtifactId, AuthorityGrantId,
        BaseImplementationId, BaseInstanceId, BootId, BoundLink, CancellationPolicy, CapabilityId,
        CapabilityLimits, CheckedFront, ConfigurationEntry, ExecutionProfileId, ExpandedFormId,
        ExpectedSign, ExpectedTerminal, FormIdentity, FragmentId, GearId, ImplementationId,
        KindContractRevision, KindId, LineContinuation, LineContract, LineDuplex, LineId,
        LineOrdering, LineReliability, LineScope, LineSecurity, LineTrafficShape,
        LinkAuthorityReference, LinkBindingId, LinkCredentialReference, LinkEndpoint,
        LinkEndpointId, LinkLimits, OfferGeneration, PlanFragment, PlanId, PlannedGear,
        PlannedSharedPool, PoolDeclarationId, PoolMemberLimits, PoolOperationId,
        PoolRealizationEnvelope, PoolSelectionEvidence, PortDescriptor, PortDirection,
        PortTemporal, SharedPoolId, SharedPoolSelectionPolicy, SignId, SourceDocumentId,
        TerminalPolicy,
    };

    fn endpoint(host: &str, boot: &str, endpoint: &str) -> LinkEndpoint {
        LinkEndpoint {
            host_id: HostId::from(host),
            boot_id: conduit_core::BootId::from(boot),
            endpoint_id: LinkEndpointId::from(endpoint),
        }
    }

    fn line(id: &str, source: LinkEndpoint, sink: LinkEndpoint) -> AdmittedLine {
        AdmittedLine {
            line_id: LineId::from(id),
            binding: BoundLink {
                binding_id: LinkBindingId::from(format!("{id}/binding")),
                source,
                sink,
                base: BaseImplementationId::from("conduit.base/test-session@1"),
                base_instance_id: BaseInstanceId::from(format!("{id}/instance")),
                credential: LinkCredentialReference::None,
                authority: LinkAuthorityReference::ProcessOwned,
                limits: LinkLimits {
                    maximum_in_flight_items: 1,
                    maximum_payload_bytes: 64,
                    maximum_buffered_bytes: 64,
                    maximum_frame_bytes: 4_096,
                },
            },
            contract: LineContract {
                scope: LineScope::LocalNetwork,
                traffic_shape: LineTrafficShape::Message,
                duplex: LineDuplex::FullDuplex,
                ordering: LineOrdering::Ordered,
                reliability: LineReliability::Reliable,
                continuation: LineContinuation::None,
                security: LineSecurity::AuthenticatedEncrypted,
            },
        }
    }

    fn fragment(
        host: &str,
        boot: &str,
        placements: Vec<PlannedGear>,
        pool: PlannedSharedPool,
    ) -> PlanFragment {
        let expected_sign = if placements.is_empty() {
            vec![
                ExpectedSign::PlanFragmentReceived,
                ExpectedSign::PlanTerminal,
            ]
        } else {
            vec![
                ExpectedSign::PlanFragmentReceived,
                ExpectedSign::PlacementPrepared(placements[0].placement_id.clone()),
                ExpectedSign::PlacementTerminal(placements[0].placement_id.clone()),
                ExpectedSign::PlanTerminal,
            ]
        };
        let expected_terminals = placements
            .iter()
            .map(|placement| ExpectedTerminal::PlacementCompleted(placement.placement_id.clone()))
            .chain(core::iter::once(ExpectedTerminal::PlanCompleted))
            .collect();
        PlanFragment {
            plan_id: PlanId::from(""),
            fragment_id: FragmentId::from(""),
            source_document_id: SourceDocumentId::from("source/pool-session"),
            checked_form_id: conduit_core::CheckedFormId::from("checked/pool-session"),
            expanded_form_id: ExpandedFormId::from("expanded/pool-session"),
            completion_policy: conduit_core::PlanCompletionPolicy::Live,
            realization_backs: vec![],
            host_id: HostId::from(host),
            boot_id: BootId::from(boot),
            offer_generation: OfferGeneration(1),
            startup_order: placements
                .iter()
                .map(|placement| placement.placement_id.clone())
                .collect(),
            placements,
            execution_regions: vec![],
            execution_fusions: vec![],
            states: vec![],
            connections: vec![],
            shared_pools: vec![pool],
            startup_dependencies: vec![],
            cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
            terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
            expected_terminals,
            sign_storage_budget: mandatory_sign_storage_requirement(&expected_sign).unwrap(),
            expected_sign,
            plan_fragments: vec![],
        }
    }

    fn exact_plan() -> (Plan, PoolSelectionEvidence, PlacementId) {
        let consumer_id = PlacementId::from("placement/client");
        let input = PortDescriptor {
            port_id: PortId::from("prompt"),
            value_kind: KindId::from("value/text"),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        };
        let output = PortDescriptor {
            port_id: PortId::from("text"),
            value_kind: KindId::from("value/text"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        };
        let pool_id = SharedPoolId::from("pool/workers");
        let request = line(
            "line/request",
            endpoint("host/client", "boot/client", "client/request"),
            endpoint("host/worker", "boot/worker", "worker/request"),
        );
        let result = line(
            "line/result",
            endpoint("host/worker", "boot/worker", "worker/result"),
            endpoint("host/client", "boot/client", "client/result"),
        );
        let pool = PlannedSharedPool {
            pool_id: pool_id.clone(),
            declaration_id: PoolDeclarationId::from("declaration/workers"),
            member_front: CheckedFront::new(vec![], vec![input], vec![output], None),
            maximum_members: 1,
            member_limits: PoolMemberLimits {
                queue_item_capacity: 1,
                queue_byte_capacity: 64,
                sign_item_capacity: 8,
                sign_byte_capacity: 1_024,
            },
            member_sessions_required: true,
            realization_envelope: vec![PoolRealizationEnvelope {
                host_id: HostId::from("host/worker"),
                boot_id: BootId::from("boot/worker"),
                offer_generation: OfferGeneration(1),
                capability_id: CapabilityId::from("capability/worker"),
                implementation_id: ImplementationId::from("implementation/worker"),
                artifact_id: ArtifactId::from("artifact/worker"),
                member_capacity: 1,
                resources: vec![],
                admitted_lines: vec![request, result],
            }],
            selection_policy:
                SharedPoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
            admission_authority: AuthorityGrantId::from("grant/workers"),
            consumers: vec![consumer_id.clone()],
        };
        let consumer = PlannedGear {
            placement_id: consumer_id.clone(),
            gear_id: GearId::from("client"),
            kind_id: KindId::from("flow/pool-observe"),
            kind_contract_revision: KindContractRevision::from("flow/pool-observe@1"),
            execution_profile_id: ExecutionProfileId::from("test@1"),
            configuration: Vec::<ConfigurationEntry>::new(),
            host_id: HostId::from("host/client"),
            boot_id: BootId::from("boot/client"),
            offer_generation: OfferGeneration(1),
            capability_id: CapabilityId::from("capability/client"),
            implementation_id: ImplementationId::from("implementation/client"),
            artifact_id: ArtifactId::from("artifact/client"),
            realization_characteristics: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 64,
            },
            inputs: vec![],
            outputs: vec![],
            host_operations: vec![],
            resources: vec![],
            authority: vec![],
            pool_references: vec![pool_id.clone()],
        };
        let plan = seal_plan(
            FormIdentity {
                source_document_id: SourceDocumentId::from("source/pool-session"),
                checked_form_id: conduit_core::CheckedFormId::from("checked/pool-session"),
                expanded_form_id: ExpandedFormId::from("expanded/pool-session"),
            },
            vec![
                fragment("host/client", "boot/client", vec![consumer], pool.clone()),
                fragment("host/worker", "boot/worker", vec![], pool),
            ],
        );
        let selection = PoolSelectionEvidence {
            plan_id: plan.plan_id.clone(),
            pool_id,
            operation_id: PoolOperationId::from("request/1"),
            selected_realization: Some(0),
            observation_sign_ids: vec![SignId::from("sign/provider/1")],
            disposition: PoolSelectionDisposition::Selected,
            sign_id: SignId::from("sign/selection/1"),
        };
        (plan, selection, consumer_id)
    }

    #[test]
    fn selected_worker_prepares_only_its_exact_finite_port_sessions() {
        let (plan, selection, consumer) = exact_plan();
        assert!(conduit_core::verify_plan(&plan));
        let sessions = PoolMemberSessions::prepare(
            &plan,
            selection.clone(),
            &consumer,
            &HostId::from("host/worker"),
        )
        .unwrap();
        assert_eq!(sessions.iter().count(), 2);
        assert_eq!(sessions.selection(), &selection);
        assert!(sessions.iter().any(|session| {
            session.direction == PoolMemberSessionDirection::Input
                && session.port_id.as_str() == "prompt"
        }));
        assert!(sessions.iter().any(|session| {
            session.direction == PoolMemberSessionDirection::Output
                && session.port_id.as_str() == "text"
        }));
        assert!(PoolMemberSessions::prepare(
            &plan,
            selection,
            &consumer,
            &HostId::from("host/unsealed"),
        )
        .is_err());
    }
}
