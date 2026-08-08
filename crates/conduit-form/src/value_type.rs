use crate::RuntimePortTemporal;
use conduit_core::{kind_id, KindId};

pub(crate) fn canonical_value_kind(source_type: &str) -> KindId {
    match source_type {
        "Text" => kind_id("value/text@1"),
        "Tick" => kind_id("value/tick@1"),
        "Count" => kind_id("value/count@1"),
        exact => kind_id(exact),
    }
}

pub(crate) fn canonical_port_temporal(source: RuntimePortTemporal) -> conduit_core::PortTemporal {
    match source {
        RuntimePortTemporal::Value => conduit_core::PortTemporal::Value,
        RuntimePortTemporal::Flow { closes } => conduit_core::PortTemporal::Flow { closes },
        RuntimePortTemporal::Current => conduit_core::PortTemporal::Current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_text_resolves_without_changing_exact_explicit_kinds() {
        assert_eq!(canonical_value_kind("Text").as_str(), "value/text@1");
        assert_eq!(canonical_value_kind("Tick").as_str(), "value/tick@1");
        assert_eq!(canonical_value_kind("Count").as_str(), "value/count@1");
        assert_eq!(canonical_value_kind("test/value").as_str(), "test/value");
    }
}
