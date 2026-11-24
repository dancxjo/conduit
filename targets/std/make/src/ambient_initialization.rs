//! Checked declaration for the hosted Ambient observation provider.
use conduit_host_make::{
    BaseInitializationContract, ConfigurationValue, KindConfigurationField, KindConfigurationRule,
};

pub(super) fn contract() -> BaseInitializationContract {
    let text = |key: &str, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text("".into()),
        rule: KindConfigurationRule::DataLabel { maximum },
    };
    BaseInitializationContract {
        fields: vec![
            text("timezone", 128),
            text("weather_endpoint", 128),
            text("source_pool", 128),
            text("memory_pool", 128),
            text("provider_access", 32),
            text("photo_source", 32),
        ],
    }
}

pub(super) fn wallpaper_contract() -> BaseInitializationContract {
    let mut contract = contract();
    contract
        .fields
        .retain(|field| field.key != "weather_endpoint" && field.key != "timezone");
    contract
}
