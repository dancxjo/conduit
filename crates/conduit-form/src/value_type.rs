use conduit_core::{kind_id, KindId};

pub(crate) fn canonical_value_kind(source_type: &str) -> KindId {
    match source_type {
        "Text" => kind_id("value/text@1"),
        exact => kind_id(exact),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_text_resolves_without_changing_exact_explicit_kinds() {
        assert_eq!(canonical_value_kind("Text").as_str(), "value/text@1");
        assert_eq!(canonical_value_kind("test/value").as_str(), "test/value");
    }
}
