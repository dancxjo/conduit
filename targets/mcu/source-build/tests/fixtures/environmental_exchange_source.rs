//! Exact authored environmental Source and finite typed-selection test fixture.
//! Consumers declare `use conduit_mcu_source_build as source_build` (or `crate`
//! in that crate's unit tests) in the parent of this #[path] module.
use super::source_build as build;
use build::composite::{Specialization, Value, ValueReference};
use conduit_core as c;
use conduit_plot as p;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy)]
enum Selection {
    State(&'static str, &'static str),
    Merge(&'static str, &'static str),
    Zip {
        alias: &'static str,
        pair: &'static str,
        left: &'static str,
        left_bytes: u32,
        right: &'static str,
        right_bytes: u32,
        feedback: bool,
    },
}
// Select the proven canonical extent, not the profile's permissive ceiling.
// These are fabrication descriptions; no runtime capacity is enlarged here.
fn exact_bytes(name: &str) -> u32 {
    static EXTENTS: std::sync::OnceLock<std::collections::BTreeMap<&'static str, u32>> =
        std::sync::OnceLock::new();
    EXTENTS.get_or_init(|| {
        let mut startup = p::StartupCatalog::new();
        let mut profiles = p::ProfileCatalog::new();
        build::protocol_catalog::install(&mut startup, &mut profiles).unwrap();
        let types =
            p::prepare_source_types(&p::parse_syntax_document(&library()), &startup).unwrap();
        [
            "BmeProtocolFrame",
            "Ht16k33ProtocolFrame",
            "BmeClockContext",
            "Ht16k33Failure",
            "I2cCorrelatedRequest",
            "BmeProtocolState",
            "BmeProtocolEvent",
            "ClockResult",
            "Ht16k33State",
            "I2cResult",
            "I2cCorrelation",
        ]
        .into_iter()
        .map(|name| {
            let bytes =
                p::maximum_prepared_canonical_value_bytes(types.named_type(name).unwrap()).unwrap();
            (name, bytes)
        })
        .collect()
    })[name]
}
fn selections() -> [Selection; 9] {
    let result_bytes = exact_bytes("I2cResult");
    [
        Selection::State("BmeCell", "BmeProtocolFrame"),
        Selection::State("HtCell", "Ht16k33ProtocolFrame"),
        Selection::Merge("BmeContexts", "BmeClockContext"),
        Selection::Merge("HtErrors", "Ht16k33Failure"),
        Selection::Merge("Requests", "I2cCorrelatedRequest"),
        Selection::Zip {
            alias: "BmeFeedback",
            pair: "BmeFeedbackPair",
            left: "BmeProtocolState",
            left_bytes: exact_bytes("BmeProtocolState"),
            right: "BmeProtocolEvent",
            right_bytes: exact_bytes("BmeProtocolEvent"),
            feedback: true,
        },
        Selection::Zip {
            alias: "BmeClockPairing",
            pair: "BmeClockPair",
            left: "BmeClockContext",
            left_bytes: exact_bytes("BmeClockContext"),
            right: "ClockResult",
            right_bytes: exact_bytes("ClockResult"),
            feedback: false,
        },
        Selection::Zip {
            alias: "HtFeedback",
            pair: "HtFeedbackPair",
            left: "Ht16k33State",
            left_bytes: exact_bytes("Ht16k33State"),
            right: "I2cResult",
            right_bytes: result_bytes,
            feedback: true,
        },
        Selection::Zip {
            alias: "BusPairing",
            pair: "BusPair",
            left: "I2cCorrelation",
            left_bytes: exact_bytes("I2cCorrelation"),
            right: "I2cResult",
            right_bytes: result_bytes,
            feedback: false,
        },
    ]
}
fn request(selection: Selection) -> Specialization<'static> {
    let reference = |type_name, maximum_bytes| ValueReference {
        type_name,
        maximum_bytes,
    };
    match selection {
        Selection::State(_, name) => {
            Specialization::SeededUntilInstance(reference(name, exact_bytes(name)))
        }
        Selection::Merge(_, name) => {
            Specialization::MergeInstance(reference(name, exact_bytes(name)))
        }
        Selection::Zip {
            left,
            left_bytes,
            right,
            right_bytes,
            feedback,
            ..
        } => Specialization::ZipInstance {
            left: reference(left, left_bytes),
            right: reference(right, right_bytes),
            feedback,
        },
    }
}
fn library() -> String {
    let units = [
        include_str!("../../../../../plots/device-protocols/bme280-lifecycle.conduit"),
        include_str!("../../../../../plots/device-protocols/bme280-clock-events.conduit"),
        include_str!("../../../../../plots/device-protocols/main.conduit"),
        include_str!("../../../../../plots/device-protocols/bme280-capture-observation.conduit"),
        include_str!("../../../../../plots/device-protocols/bme280-protocol-frame.conduit"),
        include_str!("../../../../../plots/device-protocols/bme280-exchange.conduit"),
        include_str!("../../../../../plots/device-protocols/ht16k33.conduit"),
        include_str!("../../../../../plots/device-protocols/ht16k33-exchanges.conduit"),
        include_str!("../../../../../plots/device-protocols/i2c-correlated.conduit"),
        include_str!("../../../../../plots/device-protocols/environmental-exchange.conduit"),
    ];
    // Ordinary declaration packaging: deduplicate imports, retain every library
    // body line. No legacy adapters or protocol text substitutions are used.
    let imports = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|l| l.starts_with("with "))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    let bodies = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|l| !l.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{imports}\n{bodies}")
}
fn aliases(source: &str) -> (String, Vec<c::KindId>) {
    let mut startup = p::StartupCatalog::new();
    let mut profiles = p::ProfileCatalog::new();
    build::protocol_catalog::install(&mut startup, &mut profiles).unwrap();
    let types = p::prepare_source_types(&p::parse_syntax_document(source), &startup).unwrap();
    let value = |name, maximum_bytes| {
        let schema = types.named_type(name).unwrap().clone();
        let contract = c::CheckedValueContract::new(
            schema.profile().unwrap().value_kind().clone(),
            maximum_bytes,
            vec![],
        )
        .unwrap();
        Value { schema, contract }
    };
    let mut imports = String::new();
    let mut ids = Vec::new();
    for selection in selections() {
        let (family, alias, pair_alias, values) = match selection {
            Selection::State(alias, name) => (
                "state/seeded/flow/until",
                alias,
                None,
                vec![value(name, exact_bytes(name))],
            ),
            Selection::Merge(alias, name) => (
                "flow/merge/finite",
                alias,
                None,
                vec![value(name, exact_bytes(name))],
            ),
            Selection::Zip {
                alias,
                pair,
                left,
                left_bytes,
                right,
                right_bytes,
                feedback,
            } => (
                if feedback {
                    "flow/zip/feedback"
                } else {
                    "flow/zip/finite"
                },
                alias,
                Some(pair),
                vec![value(left, left_bytes), value(right, right_bytes)],
            ),
        };
        let id = c::typed_kind_instance_id(family, values.iter().map(|v| (&v.schema, &v.contract)))
            .unwrap();
        imports.push_str(&format!("with {} as {alias}\n", id.as_str()));
        if let Some(pair) = pair_alias {
            imports.push_str(&format!("with {}/paired as {pair}\n", id.as_str()));
        }
        ids.push(id);
    }
    (imports, ids)
}

const CALLER: &str = include_str!("environmental-exchange-selected.conduit");

/// Existing fabrication arguments, not runtime authority or a new Source syntax.
pub fn specializations() -> [Specialization<'static>; 9] {
    selections().map(request)
}
pub struct Fixture {
    pub source: String,
    pub source_sha256: String,
    pub typed_receipt: Vec<u8>,
    pub typed_receipt_sha256: String,
    pub kind_ids: Vec<c::KindId>,
}
impl Fixture {
    pub fn compile(&self) -> Result<build::workset::Workset, String> {
        build::workset::compile_captured_specializations(
            self.source.as_bytes(),
            &self.source_sha256,
            &["shared-environment".into()],
            None,
            build::workset::CapturedTypedSelections {
                bytes: &self.typed_receipt,
                sha256: &self.typed_receipt_sha256,
            },
        )
    }
}
pub fn selected_source() -> Fixture {
    let library = library();
    let (imports, kind_ids) = aliases(&library);
    let source = format!("{imports}\n{library}\n{CALLER}");
    let source_sha256 = format!("{:x}", Sha256::digest(source.as_bytes()));
    let reference = |name: &str, maximum: u32| {
        serde_json::json!({
            "type_name":name,"maximum_bytes":maximum,
        })
    };
    let values = selections().map(|selection| match selection {
        Selection::State(_, name) => serde_json::json!({
            "kind":"seeded-until","value":reference(name,exact_bytes(name)),
        }),
        Selection::Merge(_, name) => serde_json::json!({
            "kind":"merge","value":reference(name,exact_bytes(name)),
        }),
        Selection::Zip {
            left,
            left_bytes,
            right,
            right_bytes,
            feedback,
            ..
        } => serde_json::json!({
            "kind":"zip","left":reference(left,left_bytes),
            "right":reference(right,right_bytes),"feedback":feedback,
        }),
    });
    let typed_receipt = serde_json::to_vec(&serde_json::json!({
        "schema":1,"source_sha256":source_sha256,"specializations":values,
    }))
    .unwrap();
    let typed_receipt_sha256 = format!("{:x}", Sha256::digest(&typed_receipt));
    Fixture {
        source,
        source_sha256,
        typed_receipt,
        typed_receipt_sha256,
        kind_ids,
    }
}
