//! Preparation-only exact-schema registry for generic finite Value repetition.
use alloc::{boxed::Box, collections::BTreeMap, format, string::String, sync::Arc};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub struct PreparedValueRepeat {
    value: CheckedValueContract,
    schema: StructuredInfoType,
    offer: CapabilityOffer,
}
impl PreparedValueRepeat {
    /// Host supplies its exact offered realization; no target identity is invented here.
    pub fn new(
        value: CheckedValueContract,
        schema: StructuredInfoType,
        offer: CapabilityOffer,
    ) -> Result<Self, String> {
        let kind = match offer.kind_id.as_str() {
            crate::VALUE_REPEAT_KIND => crate::value_repeat_semantic_contract(&value, &schema),
            crate::VALUE_REPEAT_CAPACITY2_KIND => {
                crate::value_repeat_capacity2_semantic_contract(&value, &schema)
            }
            _ => return Err("unsupported finite repeat semantic identity".into()),
        }
        .map_err(String::from)?;
        super::exact_offer::validate(kind, &offer)?;
        Ok(Self {
            value,
            schema,
            offer,
        })
    }
    pub fn offer(&self) -> &CapabilityOffer {
        &self.offer
    }
    pub fn schema(&self) -> &StructuredInfoType {
        &self.schema
    }
}
struct Selected {
    placement: PlannedGear,
    profile: Arc<PreparedValueRepeat>,
    count: u16,
}
pub struct ValueRepeatOperationFactory {
    implementation: ImplementationId,
    selected: BTreeMap<PlacementId, Selected>,
}
fn count(gear: &PlannedGear, offer: &CapabilityOffer) -> Result<u16, String> {
    if gear.kind_id != offer.kind_id
        || gear.kind_contract_revision != offer.kind_contract_revision
        || gear.execution_profile_id != offer.implementation.execution_profile_id
        || gear.capability_id != offer.capability_id
        || gear.implementation_id != offer.implementation.implementation_id
        || gear.artifact_id != offer.implementation.artifact_id
        || gear.inputs != offer.inputs
        || gear.outputs != offer.outputs
        || gear.limits != offer.limits
        || gear.semantic_contract != offer.semantic_contract
        || !gear.host_calls.is_empty()
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
        || gear.base.is_some()
        || !gear.realization_characteristics.is_empty()
        || !gear.realization_properties.is_empty()
        || !gear.pool_references.is_empty()
        || !gear.terminal_transductions.is_empty()
    {
        return Err("finite repeat placement differs from its exact offered profile".into());
    }
    let [configuration] = gear.configuration.as_slice() else {
        return Err("finite repeat needs exact count configuration".into());
    };
    let ConfigurationValue::U64(count) = configuration.value else {
        return Err("finite repeat count is not Count".into());
    };
    if configuration.key != "count"
        || count == 0
        || count > u64::from(crate::VALUE_REPEAT_MAXIMUM_COUNT)
    {
        return Err("finite repeat count is outside its admitted bound".into());
    }
    Ok(count as u16)
}
impl ValueRepeatOperationFactory {
    pub fn for_plan(
        plan: &Plan,
        implementation: &ImplementationId,
        profiles: &[Arc<PreparedValueRepeat>],
    ) -> Result<Self, String> {
        if !verify_plan(plan) {
            return Err("invalid sealed Plan".into());
        }
        let mut selected = BTreeMap::new();
        let mut counts = BTreeMap::new();
        for gear in plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|gear| gear.implementation_id == *implementation)
        {
            let mut matches = profiles.iter().filter_map(|profile| {
                count(gear, profile.offer())
                    .ok()
                    .map(|count| (profile, count))
            });
            let (profile, count) = matches
                .next()
                .ok_or("no exact checked finite repeat profile")?;
            if matches.next().is_some() {
                return Err("ambiguous finite repeat profile".into());
            }
            let instances = counts
                .entry(profile.offer().capability_id.clone())
                .or_insert(0u16);
            if *instances >= profile.offer().limits.max_active_instances {
                return Err("finite repeat selected instance capacity exceeded".into());
            }
            *instances += 1;
            if selected
                .insert(
                    gear.placement_id.clone(),
                    Selected {
                        placement: gear.clone(),
                        profile: profile.clone(),
                        count,
                    },
                )
                .is_some()
            {
                return Err("duplicate finite repeat placement".into());
            }
        }
        Ok(Self {
            implementation: implementation.clone(),
            selected,
        })
    }
    fn selected(&self, gear: &PlannedGear) -> Result<&Selected, String> {
        let selected = self
            .selected
            .get(&gear.placement_id)
            .ok_or("no selected finite repeat placement")?;
        if gear != &selected.placement {
            return Err("finite repeat placement changed after admission".into());
        }
        Ok(selected)
    }
}
impl KernelOperationFactory for ValueRepeatOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let selected = self.selected(gear)?;
        Ok(KernelOperationBudget {
            value_items: 2,
            value_bytes: gear.limits.max_queue_bytes,
            maximum_value_bytes: selected.profile.value.maximum_bytes,
            host_requests: 0,
            sign_items: 16,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        let selected = self.selected(gear)?;
        Ok(Box::new(
            conduit_data::ValueRepeatBack::prepare_with_schema(
                &selected.profile.value,
                &selected.profile.schema,
                selected.count,
                crate::VALUE_REPEAT_MAXIMUM_COUNT,
            )
            .map_err(|error| format!("finite repeat preparation: {error:?}"))?,
        ))
    }
}
