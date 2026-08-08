use crate::{
    default_placements_unvalidated, plan_validated_form, PlacementChoices, PlannerError,
    PlanningOptions,
};
use conduit_core::{
    ConnectionProvider, HostAdvertisement, Plan, DEFAULT_CONNECTION_BYTE_CAPACITY,
    DEFAULT_CONNECTION_ITEM_CAPACITY,
};
use conduit_form::{CheckedForm, ExpandedCanonicalForm};
use std::collections::BTreeMap;

pub fn default_expanded_placements(
    form: &ExpandedCanonicalForm,
    realm: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    form.validate_expansion()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    default_placements_unvalidated(&form.operations, realm)
}

pub fn plan_expanded_canonical(
    form: &ExpandedCanonicalForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
) -> Result<Plan, PlannerError> {
    plan_expanded_canonical_with_options(
        form,
        realm,
        placements,
        providers,
        PlanningOptions {
            connection_providers: &BTreeMap::new(),
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[],
            protected_resource_grants: &[],
            link_bindings: &[],
        },
    )
}

pub fn plan_expanded_canonical_with_options(
    form: &ExpandedCanonicalForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    form.validate_expansion()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    let planning_form = CheckedForm {
        source_document_id: form.source_document_id.clone(),
        checked_form_id: form.checked_form_id.clone(),
        expanded_form_id: form.expanded_form_id.clone(),
        name: form.name.clone(),
        operations: form.operations.clone(),
        connections: form.connections.clone(),
        exports: Vec::new(),
        nested_forms: Vec::new(),
    };
    plan_validated_form(&planning_form, realm, placements, providers, options)
}
