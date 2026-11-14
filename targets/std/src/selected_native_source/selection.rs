//! Exact native owner selection for either a fragment or its containing Body.
use super::*;
use conduit_body::{BodyPlan, BodyPlayIdentity, Wake};
use conduit_plan_lowering::{lowering::LoweredPlanFragment, selected_operation::*};
enum PlayScope<'a> {
    Fragment(&'a ActivePlayIdentity),
    Body {
        wake: &'a Wake,
        plan: &'a BodyPlan,
        play: &'a BodyPlayIdentity,
    },
}
impl InitializedNativeSource {
    pub(crate) fn matches(&self, gear: &PlannedGear) -> bool {
        let d = &self.definition;
        let offer = &d.base.capabilities[0];
        gear.capability_id == offer.capability_id
            && gear.kind_id == d.kind.kind_id
            && gear.implementation_id == offer.implementation.implementation_id
            && gear.artifact_id == offer.implementation.artifact_id
    }
    pub(crate) fn budget(&self) -> crate::installed_std::factory::BackBudget {
        let d = &self.definition;
        crate::installed_std::factory::BackBudget {
            value_items: 2,
            value_bytes: d.input_bytes.saturating_add(d.output_bytes),
            host_requests: 1,
            sign_items: 16,
            maximum_value_bytes: d.input_bytes.max(d.output_bytes),
        }
    }
    pub(crate) fn prepare<'a>(
        &'a mut self,
        registry: &BaseRegistry,
        host: &HostAdvertisement,
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &ActivePlayIdentity,
    ) -> Result<Option<PreparedNativeSource<'a>>, String> {
        self.prepare_scoped(
            registry,
            host,
            fragment,
            lowered,
            PlayScope::Fragment(active),
        )
    }
    pub(crate) fn prepare_body<'a>(
        &'a mut self,
        registry: &BaseRegistry,
        host: &HostAdvertisement,
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        context: (&Wake, &BodyPlan, &BodyPlayIdentity),
    ) -> Result<Option<PreparedNativeSource<'a>>, String> {
        self.prepare_scoped(
            registry,
            host,
            fragment,
            lowered,
            PlayScope::Body {
                wake: context.0,
                plan: context.1,
                play: context.2,
            },
        )
    }
    pub(crate) fn has_image_backing(&self) -> bool {
        self._image_memory.is_some() && self.backing.is_some()
    }
    pub(crate) fn is_immediate(&self) -> bool {
        matches!(self.provider, NativeProvider::Immediate(_))
    }
    fn prepare_scoped<'a>(
        &'a mut self,
        registry: &BaseRegistry,
        host: &HostAdvertisement,
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        play: PlayScope<'_>,
    ) -> Result<Option<PreparedNativeSource<'a>>, String> {
        let d = &self.definition;
        let mut gears = fragment.placements.iter().filter(|gear| self.matches(gear));
        let Some(gear) = gears.next() else {
            return Ok(None);
        };
        if let NativeProvider::Worker(worker) = &self.provider {
            if !worker.ready_for_play() {
                return Err("native worker is not ready and quiescent for a child Play".into());
            }
        }
        if gears.next().is_some() {
            return Err("native Source slot supports exactly one placement".into());
        }
        if !registry.entries().iter().any(|entry| entry == &d.base)
            || host.host_id != d.grant.host_id
            || host.boot_id != d.grant.boot_id
            || fragment.offer_generation != host.offer_generation
            || gear.base.as_ref()
                != host
                    .bases
                    .iter()
                    .find(|base| base.provider_instance_id == d.base.provider_instance_id)
                    .map(|base| base.binding())
                    .as_ref()
            || gear.resources.len() != 1
            || gear.resources[0].pool_id != d.base.resources[0].pool_id
            || gear.resources[0].class_id != d.base.resources[0].class_id
            || gear.resources[0].units != 1
            || gear.authority.len() != 1
        {
            return Err("native Source selected a stale registry, offer, grant or resource".into());
        }
        let a = &gear.authority[0];
        let g = &d.grant;
        if a.grant_id != g.grant_id
            || a.contract_id != g.contract_id
            || a.host_call_contract_id != g.host_call_contract_id
            || a.subject_kind != g.subject_kind
            || a.host_id != g.host_id
            || a.boot_id != g.boot_id
            || a.capability_id != g.capability_id
        {
            return Err(
                "native Source Plan does not possess the independently installed grant".into(),
            );
        }
        let envelope: CapabilityEnvelopeId = "std/native-source/exact@1".into();
        let scope = BaseCapabilityScope {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            base_instance_id: d.base.provider_instance_id.clone(),
            base_provider_generation: d.base.provider_generation,
            plan_id: match &play {
                PlayScope::Fragment(active) => &active.plan_id,
                PlayScope::Body { plan, .. } => &plan.plan_id,
            }
            .clone(),
            active_play_id: match &play {
                PlayScope::Fragment(active) => &active.active_play_id,
                PlayScope::Body { play, .. } => &play.active_play_id,
            }
            .clone(),
            authority_grant_id: g.grant_id.clone(),
            authority_contract_id: g.contract_id.clone(),
            capability_id: g.capability_id.clone(),
            implementation_id: gear.implementation_id.clone(),
            operation_contract_id: g.host_call_contract_id.clone(),
            subject_kind: g.subject_kind.clone(),
            resource_pool_id: d.base.resources[0].pool_id.clone(),
            resource_generation_id: d.resource_generation.clone(),
            envelope_id: envelope.clone(),
            maximum_parameter_bytes: d.input_bytes,
            maximum_result_bytes: d.output_bytes,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        };
        let authority = BaseCapabilityAuthority {
            grant: g.clone(),
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            envelope_id: envelope,
            maximum_parameter_bytes: d.input_bytes,
            maximum_result_bytes: d.output_bytes,
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
            parameter_bytes: d.input_bytes,
            work_units: 1,
        };
        let mut issuer = [0; 32];
        getrandom::fill(&mut issuer)
            .map_err(|error| format!("native Source issuer entropy: {error}"))?;
        let mut table = BaseCapabilityTable::new(
            host.host_id.clone(),
            host.boot_id.clone(),
            d.base.provider_instance_id.clone(),
            d.base.provider_generation,
            issuer,
            1,
        )
        .map_err(|error| format!("native Source table: {error:?}"))?;
        let handle = table
            .issue(CapabilityIssueRequest { scope, authority })
            .map_err(|error| format!("native Source possession: {error:?}"))?;
        let contract = SelectedOperationContract {
            kind: &d.kind,
            call: g.host_call_contract_id.as_str(),
            input_bytes: d.input_bytes,
            output_bytes: d.output_bytes,
            resource_bytes: d.resource_bytes,
        };
        let node = match play {
            PlayScope::Fragment(active) => bind_selected_configured_operation(
                &table,
                &claim,
                contract,
                &d.configuration,
                SelectedOperationPlan {
                    fragment,
                    lowered,
                    active,
                    placement_id: &gear.placement_id,
                },
            ),
            PlayScope::Body { wake, plan, play } => bind_body_selected_configured_operation(
                &table,
                &claim,
                contract,
                &d.configuration,
                SelectedBodyOperationPlan {
                    wake,
                    plan,
                    play,
                    fragment,
                    lowered,
                    placement_id: &gear.placement_id,
                },
            ),
        }
        .map_err(|error| format!("native Source selection: {error:?}"))?;
        let operation = BoundBaseOperation::bind(table, handle, claim)
            .map_err(|(_, _, _, error)| format!("native Source binding: {error:?}"))?;
        Ok(Some(PreparedNativeSource {
            node,
            operation,
            provider: &mut self.provider,
            output: vec![0; d.output_bytes as usize].into_boxed_slice(),
            next_request: 0,
            pending: None,
            cancelled: false,
            cancellation_started: None,
        }))
    }
}
