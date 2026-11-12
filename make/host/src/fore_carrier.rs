//! Shared finite initialization field contract for an explicitly selected Fore
//! carrier Base. Describing/checking these fields creates no carrier custody,
//! native initialization, Line availability, resource grant or runtime authority.
use crate::{
    BaseInitializationContract, ConfigurationValue, KindConfigurationField, KindConfigurationRule,
};
pub const BASE_KIND: &str = "line/fore-carrier";
pub fn initialization_contract() -> BaseInitializationContract {
    let count = |key: &str, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(1),
        rule: KindConfigurationRule::U64Range {
            minimum: 1,
            maximum,
        },
    };
    BaseInitializationContract {
        fields: vec![
            KindConfigurationField {
                key: "authorizer".into(),
                default_value: ConfigurationValue::Text("seed-owner".into()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec!["seed-owner".into()],
                },
            },
            count("maximum_lines", 16),
            count("maximum_frame_bytes", 4096),
            count("maximum_payload_bytes", 4096),
            count("maximum_buffered_bytes", 65536),
            count("maximum_in_flight_items", 1),
            count("maximum_frames", 1_000_000),
        ],
    }
}
