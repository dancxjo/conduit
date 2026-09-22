//! Installed configuration for the bounded Boolean-field summary.

use super::back::{BackBudget, BackFactory, InstalledBack};
use super::json_backs::{budget, JsonBack};
use conduit_core::{ConfigurationValue, PlannedGear};

pub(super) static JSON_BOOLEAN_SUMMARY_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::JSON_BOOLEAN_SUMMARY_STD_IMPLEMENTATION,
    budget: summary_budget,
    prepare,
};

pub(super) fn field_configuration(placement: &PlannedGear) -> Result<&str, String> {
    if placement.configuration.len() != 1 || placement.configuration[0].key != "field" {
        return Err("JSON Boolean summary requires exactly one field configuration".into());
    }
    match &placement.configuration[0].value {
        ConfigurationValue::Text(field)
            if !field.is_empty() && field.len() <= conduit_web::JSON_MAXIMUM_KEY_BYTES =>
        {
            Ok(field)
        }
        _ => Err("JSON Boolean summary field is not a nonempty bounded name".into()),
    }
}

fn summary_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    field_configuration(placement)?;
    budget(
        placement,
        conduit_std_offers::json_boolean_summary_std_offer(),
    )
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    summary_budget(placement)?;
    Ok(InstalledBack::Json(JsonBack::new()))
}
