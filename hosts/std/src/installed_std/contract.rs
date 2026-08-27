use conduit_core::{CapabilityOffer, ConfigurationEntry, ConfigurationValue};

#[cfg(test)]
pub(crate) use conduit_std_catalog::{
    TEXT_PRESENTATION_CONTRACT_REVISION, TEXT_PRESENTATION_KIND, TEXT_PRESENTATION_VALUE_KIND,
};
pub(crate) use conduit_std_offers::text_presentation_offer as text_offer;
#[cfg(test)]
pub(crate) use conduit_std_offers::TEXT_PRESENTATION_IMPLEMENTATION;
pub(crate) use conduit_text::MAX_TEXT_BYTES;

pub(crate) const TICK_KIND: &str = conduit_time::TICK_KIND;
pub(crate) const TICK_VALUE_KIND: &str = conduit_time::TICK_VALUE_KIND;
pub(super) const TICK_ENCODED_LEN: u32 = conduit_time::TICK_ENCODED_LEN;
pub(crate) const TICK_CONTRACT_REVISION: &str = conduit_time::TICK_CONTRACT_REVISION;
pub(super) use conduit_std_offers::{
    TICK_ARTIFACT, TICK_EXECUTION_PROFILE, TICK_IMPLEMENTATION, TIME_EVERY_ARTIFACT,
    TIME_EVERY_EXECUTION_PROFILE, TIME_EVERY_IMPLEMENTATION,
};
pub(super) use conduit_time::{
    decode_tick, encode_tick, TickConfiguration, TIME_EVERY_CONTRACT_REVISION, TIME_EVERY_COUNT,
    TIME_EVERY_KIND,
};

pub(crate) fn every_offer() -> CapabilityOffer {
    conduit_std_offers::time_every_offer()
}

pub(super) fn parse_every_configuration(
    entries: &[ConfigurationEntry],
) -> Result<TickConfiguration, String> {
    if entries.len() != 1 {
        return Err("time/every requires exactly one planned configuration field".to_string());
    }
    let period_ms = entries
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            ("freq", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or_else(|| "missing or invalid time/every configuration 'freq'".to_string())?;
    Ok(TickConfiguration {
        count: TIME_EVERY_COUNT,
        period_ms,
    })
}

pub(crate) fn tick_offer() -> CapabilityOffer {
    conduit_std_offers::tick_capability_offer()
}

pub(super) fn parse_tick_configuration(
    entries: &[ConfigurationEntry],
) -> Result<TickConfiguration, String> {
    conduit_time::parse_tick_configuration(entries).map_err(|error| error.to_string())
}

#[cfg(test)]
pub(super) fn test_tick_catalog() -> conduit_form::ProfileCatalog {
    use conduit_core::{kind_id, KindContractRevision};
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};

    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(TICK_KIND),
            kind_contract_revision: KindContractRevision::from(TICK_CONTRACT_REVISION),
            inputs: Vec::new(),
            outputs: tick_offer().outputs,
            configuration: vec![
                ConfigurationField {
                    key: "count".to_string(),
                    default_value: ConfigurationValue::U64(4),
                    validation: ConfigurationRule::U64Range {
                        minimum: 0,
                        maximum: conduit_time::MAX_TICK_COUNT,
                    },
                },
                ConfigurationField {
                    key: "period-ms".to_string(),
                    default_value: ConfigurationValue::U64(1_000),
                    validation: ConfigurationRule::U64Range {
                        minimum: 0,
                        maximum: u64::MAX,
                    },
                },
            ],
        })
        .expect("the test catalog has one exact typed tick revision");
    catalog
}
