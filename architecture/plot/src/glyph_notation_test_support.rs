//! Shared exact owner contract for registry, Source shipment and scanner tests.
use crate::*;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindIdentity, KindSemanticLaw, KindTerminalBehavior,
    PortDescriptor, PortDirection, PortTemporal,
};

pub(crate) fn fixture() -> (StartupCatalog, crate::ProfileCatalog, TypedLiteralFamily) {
    let document =
        crate::parse_syntax_document("type FixtureLiteral = {\n    payload: Text <= 64B\n}\n");
    let checked = crate::check_syntax_document(&document, &StartupCatalog::new()).unwrap();
    let ty = &checked.native_types[0];
    let mut startup = StartupCatalog::new();
    startup
        .insert_checked_native_type("FixtureLiteral", ty)
        .unwrap();
    startup
        .insert(KindSignature {
            kind: "fixture/literal".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut profile = crate::ProfileCatalog::new();
    profile
        .insert_kind(Kind {
            kind_id: kind_id("fixture/literal"),
            kind_contract_revision: KindIdentity::from("fixture/literal@1"),
            startup_parameters: vec![],
            shorthand: None,
            inputs: vec![],
            outputs: vec![PortDescriptor {
                port_id: port_id("value"),
                value_kind: ty.value_type.profile().unwrap().value_kind().clone(),
                direction: PortDirection::Output,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            }],
            configuration: vec![],
            semantic_laws: vec![KindSemanticLaw::Terminal(KindTerminalBehavior::EmitsOnce)],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 16384,
            },
        })
        .unwrap();
    let family = TypedLiteralFamily {
        revision: "fixture/notation@1".into(),
        origin: TypedLiteralFamilyOrigin {
            package_content_digest: [7; 32],
            module_path: "fixture/notation".into(),
            source_document_id: document.source_document_id().clone(),
        },
        branches: vec![TypedLiteralBranch {
            delimiter: TypedLiteralDelimiter::Slash,
            lexical_policy: TypedLiteralLexicalPolicy::RawUnicode,
            parser_contract: "fixture/parser@1".into(),
            constructor_kind: kind_id("fixture/literal"),
            constructor_revision: KindIdentity::from("fixture/literal@1"),
            result_type: ty.value_type.clone(),
            maximum_payload_bytes: 64,
        }],
    };
    (startup, profile, family)
}

pub(crate) const MANIFEST: &str =
    "pack fixture/glyph (\n version = 1.0.0\n) {\n ship notation\n}\n";
pub(crate) const SOURCE: &str = "# Original π / IPA owner metadata\nglyph notation notation = {\n revision: \"fixture/notation@1\",\n branches: [{ delimiter: \"slash\", lexical-policy: \"raw-unicode\", parser: \"fixture/parser@1\", constructor: \"fixture/literal\", constructor-revision: \"fixture/literal@1\", result: \"FixtureLiteral\", maximum-payload-bytes: 64 }]\n}\n";
