//! Finite inert metadata for exact generic Back preparation.
use super::ProtocolValue;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProtocolSpecialization {
    Singleton {
        value: ProtocolValue,
    },
    SeededFlow {
        value: ProtocolValue,
    },
    SeededUntil {
        value: ProtocolValue,
    },
    SeededUntilInstance {
        value: ProtocolValue,
    },
    FeedbackZipInstance {
        left: ProtocolValue,
        right: ProtocolValue,
    },
    Concat {
        value: ProtocolValue,
    },
    Merge {
        value: ProtocolValue,
    },
    MergeInstance {
        value: ProtocolValue,
    },
    Zip {
        left: ProtocolValue,
        right: ProtocolValue,
    },
    FeedbackZip {
        left: ProtocolValue,
        right: ProtocolValue,
    },
}
