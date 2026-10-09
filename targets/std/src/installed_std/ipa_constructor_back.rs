//! IPA parsing runs before Play; the existing structured Value Back emits once.
use super::{
    back::{BackBudget, BackFactory, InstalledBack},
    structured_values_back::StructuredLiteralBack,
};
use conduit_core::PlannedGear;
use conduit_kernel::{HostedValueStore, ValueStorage};
use conduit_speech::ipa_constructors::{self, IpaConstructor, PreparedIpaValue};
#[cfg(test)]
#[path = "ipa_constructor_back_tests.rs"]
mod tests;

pub(super) static PHONE: BackFactory = factory(IpaConstructor::Phone);
pub(super) static PHONETIC: BackFactory = factory(IpaConstructor::Phonetic);
pub(super) static PHONEME: BackFactory = factory(IpaConstructor::Phoneme);
pub(super) static PHONEMIC: BackFactory = factory(IpaConstructor::Phonemic);
const fn factory(constructor: IpaConstructor) -> BackFactory {
    BackFactory {
        implementation_id: constructor.implementation(),
        budget,
        prepare,
    }
}
fn admitted(placement: &PlannedGear) -> Result<PreparedIpaValue, String> {
    let constructor =
        IpaConstructor::from_kind(placement.kind_id.as_str()).ok_or("unknown IPA constructor")?;
    let expected = ipa_constructors::offer(constructor);
    if placement.kind_contract_revision != expected.kind_contract_revision
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.limits != expected.limits
        || placement.semantic_contract
            != ipa_constructors::contract(constructor).semantic_contract()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || placement.base.is_some()
        || !placement.realization_properties.is_empty()
        || !placement.realization_characteristics.is_empty()
        || !placement.pool_references.is_empty()
        || !placement.terminal_transductions.is_empty()
    {
        return Err("IPA constructor differs from its exact installed offer".into());
    }
    let value = ipa_constructors::prepare_configuration(constructor, &placement.configuration)
        .map_err(|e| format!("IPA constructor preparation: {e:?}"))?;
    if value.value_kind() != &placement.outputs[0].value_kind {
        return Err("IPA constructor output Type differs".into());
    }
    Ok(value)
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let value = admitted(placement)?;
    let maximum = u32::try_from(value.bytes().len()).map_err(|_| "IPA value exceeds storage")?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: maximum.saturating_mul(2),
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: maximum,
    })
}
fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    let prepared = admitted(placement)?;
    let value = values
        .store(prepared.bytes())
        .map_err(|e| format!("store prepared IPA: {e:?}"))?;
    Ok(InstalledBack::StructuredLiteral(
        StructuredLiteralBack::prepared(value),
    ))
}
