//! Root-owned checked Source, native offers and one finite capability issuance.
use super::*;
use conduit_environment_runtime::environmental_reading::source_observation::SOURCE_KIND;
use std::collections::BTreeMap;
const SOURCE:&str = "plot sample (\n >> begin: Empty...|\n) {\n source: environment/read-environmental-source()\n publication: environment/publish-environmental-observation()\n begin >> source.request\n source.observation >> publication.value\n}\n";
impl<P: EnvironmentalSourceProvider> UsbSourceRoot<P> {
    pub(super) fn plan(&self) -> Result<Plan, Refusal> {
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
        let mut host = self.host.clone();
        let kind = ObservationContract::prepare()
            .map_err(|_| Refusal::Plan)?
            .kind(true)
            .map_err(|_| Refusal::Plan)?;
        host.capabilities.push(
            BackOfferBuilder::new(
                kind,
                Back {
                    capability_id: "linux/environmental-publication@1".into(),
                    execution_profile_id: PROFILE.into(),
                    implementation_id: "linux/environmental-publication@1".into(),
                    artifact_id: "native/environmental-publication@1".into(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build(),
        );
        let hosts = [host];
        let choices = conduit_planner::default_expanded_placements(&plot.expanded, &hosts)
            .map_err(|_| Refusal::Plan)?;
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
        .map_err(|_| Refusal::Plan)
    }
    fn grant(&self) -> AuthorityGrant {
        AuthorityGrant {
            grant_id: format!("grant/{}", self.identity.provider_instance_id.as_str()).into(),
            contract_id: AUTHORITY_CONTRACT.into(),
            host_call_contract_id: HOST_CALL.into(),
            subject_kind: SOURCE_KIND.into(),
            host_id: self.host.host_id.clone(),
            boot_id: self.host.boot_id.clone(),
            capability_id: CAPABILITY.into(),
        }
    }
    pub(super) fn issue(
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
        if fragment.host_id != self.host.host_id
            || fragment.boot_id != self.host.boot_id
            || !verify_plan_fragment(fragment)
            || *active
                != bind_active_play(
                    &fragment.plan_id,
                    &self.host.host_id,
                    &self.host.boot_id,
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
            capability_id: CAPABILITY.into(),
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
