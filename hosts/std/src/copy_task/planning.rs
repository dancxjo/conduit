use crate::StdHost;
use conduit_core::{ConnectionBase, Plan, PlanFragment, ProtectedResourceGrant};
use conduit_form::CheckedForm;
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
use std::collections::BTreeMap;

const COPY_FORM_SOURCE: &str = "form 0\n\ncopy-task {\n    copy: file/copy\n    show: presentation/structured-info\n    copy.result -> show.input\n}\n";

#[derive(Debug, Clone)]
pub struct PreparedCopyTask {
    pub form: CheckedForm,
    pub plan: Plan,
    pub fragment: PlanFragment,
}

pub fn prepare_copy_task(
    host: &StdHost,
    grants: &[ProtectedResourceGrant; 2],
) -> Result<PreparedCopyTask, String> {
    let mut catalog = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_copy_file_catalog(&mut catalog)?;
    let form =
        conduit_form::parse(COPY_FORM_SOURCE, &catalog).map_err(|error| error.to_string())?;
    let placements = default_placements(&form, std::slice::from_ref(host.advertisement()))
        .map_err(|error| error.to_string())?;
    let overrides = BTreeMap::new();
    let plan = plan_with_options(
        &form,
        std::slice::from_ref(host.advertisement()),
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &overrides,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: grants,
            line_offers: &[],
        },
    )
    .map_err(|error| error.to_string())?;
    let fragment = plan
        .fragments
        .first()
        .cloned()
        .ok_or_else(|| "copy Plan has no local fragment".to_string())?;
    Ok(PreparedCopyTask {
        form,
        plan,
        fragment,
    })
}
