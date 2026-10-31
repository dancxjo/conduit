//! Root-owned checked Source, native offers and one finite capability issuance.
use super::*;
use conduit_environment_runtime::environmental_reading::source_observation::SOURCE_KIND;
use std::collections::BTreeMap;
const SOURCE:&str = "plot sample (\n >> begin: Empty...|\n) {\n source: environment/read-environmental-source()\n publication: environment/publish-environmental-observation()\n begin >> source.request\n source.observation >> publication.value\n}\n";
impl<P: EnvironmentalSourceProvider> UsbSourceRoot<P> {
    pub(super) fn plan(&self) -> Result<Plan, Refusal> {
        self.plan_on(&self.advertisement()?)
    }
    pub(super) fn plan_on(&self, host: &HostAdvertisement) -> Result<Plan, Refusal> {
        if self.provider.is_none() {
            return Err(Refusal::Acquisition);
        }
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profiles = conduit_plot::ProfileCatalog::new();
        conduit_environment_runtime::environmental_source::install_catalog(
            &mut startup,
            &mut profiles,
        )
        .map_err(|_| Refusal::Plan)?;
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(SOURCE),
            &startup,
        )
        .map_err(|_| Refusal::Plan)?;
        let plot = conduit_plot::expand_canonical_plot_for_authoring(&checked, "sample", &profiles)
            .map_err(|_| Refusal::Plan)?;
        let host = host.clone();
        let mut choices = conduit_planner::PlacementChoices {
            by_gear: BTreeMap::new(),
        };
        for gear in &plot.expanded.gears {
            let capability = if gear.kind_id.as_str() == SOURCE_KIND {
                self.identity.capability.clone()
            } else {
                self.identity.publication.clone()
            };
            choices.by_gear.insert(
                gear.gear_id.clone(),
                conduit_planner::PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: capability,
                },
            );
        }
        let hosts = [host];
        let mut fore = BTreeMap::new();
        for binding in &plot.input_bindings {
            fore.insert(
                conduit_planner::ForeBoundaryKey {
                    direction: PortDirection::Input,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 1,
                },
            );
        }
        conduit_planner::plan_expanded_authoring_with_options(
            &plot,
            &hosts,
            &choices,
            &["conduit.base/local@1".into()],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: MAXIMUM_BYTES as u32,
                authority_grants: &[self.grant()],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &fore,
        )
        .map_err(|error| Refusal::Planning(Box::new(error)))
    }
    pub(super) fn grant(&self) -> AuthorityGrant {
        AuthorityGrant {
            grant_id: format!("grant/{}", self.identity.provider_instance_id.as_str()).into(),
            contract_id: AUTHORITY_CONTRACT.into(),
            host_call_contract_id: HOST_CALL.into(),
            subject_kind: SOURCE_KIND.into(),
            host_id: self.identity.host_id.clone(),
            boot_id: self.identity.boot_id.clone(),
            capability_id: self.identity.capability.clone(),
        }
    }
    pub(super) fn issue(
        &self,
        fragment: &PlanFragment,
        issued: &conduit_std_host::IssuedKernelPlay,
        lease: &conduit_std_host::NativeFragmentReservation,
    ) -> Result<
        (
            BaseCapabilityTable,
            BaseCapabilityHandle,
            BaseOperationClaim,
        ),
        Refusal,
    > {
        if !lease.matches_fragment(fragment) {
            return Err(Refusal::Authority);
        }
        self.issue_for_active(fragment, issued.identity())
    }
    pub(super) fn issue_for_active(
        &self,
        fragment: &PlanFragment,
        active: &ActivePlayIdentity,
    ) -> Result<
        (
            BaseCapabilityTable,
            BaseCapabilityHandle,
            BaseOperationClaim,
        ),
        Refusal,
    > {
        if fragment.host_id != self.identity.host_id
            || fragment.boot_id != self.identity.boot_id
            || !verify_plan_fragment(fragment)
            || *active
                != bind_active_play(
                    &fragment.plan_id,
                    &self.identity.host_id,
                    &self.identity.boot_id,
                    active.play_sequence,
                )
        {
            return Err(Refusal::Authority);
        }
        let i = &self.identity;
        let mut table = BaseCapabilityTable::new(
            i.host_id.clone(),
            i.boot_id.clone(),
            i.provider_instance_id.clone(),
            i.provider_generation,
            self.key,
            1,
        )
        .map_err(|_| Refusal::Authority)?;
        let grant = self.grant();
        let scope = BaseCapabilityScope {
            host_id: i.host_id.clone(),
            boot_id: i.boot_id.clone(),
            base_instance_id: i.provider_instance_id.clone(),
            base_provider_generation: i.provider_generation,
            plan_id: fragment.plan_id.clone(),
            active_play_id: active.active_play_id.clone(),
            authority_grant_id: grant.grant_id.clone(),
            authority_contract_id: grant.contract_id.clone(),
            capability_id: self.identity.capability.clone(),
            implementation_id: i.implementation_id.clone(),
            operation_contract_id: HOST_CALL.into(),
            subject_kind: SOURCE_KIND.into(),
            resource_pool_id: i.resource_pool_id.clone(),
            resource_generation_id: i.resource_generation_id.clone(),
            envelope_id: i.envelope_id.clone(),
            maximum_parameter_bytes: 1,
            maximum_result_bytes: MAXIMUM_BYTES as u32,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        };
        let claim = BaseOperationClaim {
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            plan_id: scope.plan_id.clone(),
            active_play_id: scope.active_play_id.clone(),
            implementation_id: scope.implementation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            envelope_id: scope.envelope_id.clone(),
            parameter_bytes: 1,
            work_units: 1,
        };
        let authority = BaseCapabilityAuthority {
            grant,
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            envelope_id: scope.envelope_id.clone(),
            maximum_parameter_bytes: 1,
            maximum_result_bytes: MAXIMUM_BYTES as u32,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        };
        let handle = table
            .issue(CapabilityIssueRequest { scope, authority })
            .map_err(|_| Refusal::Authority)?;
        Ok((table, handle, claim))
    }
}
