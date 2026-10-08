//! Pure parser entry preparation retains complete Source checking while
//! avoiding the declaration-only packaging pass for zero specializations.
#[path = "common/parser_kernel.rs"]
mod kernel;
use conduitos::protocol_source::{
    PreparedProtocolEntry, ProtocolSourcePackage, MAXIMUM_SOURCE_BYTES, PACKAGE_SCHEMA,
};

const IDENTITY: &str = "plot identity (\n >> input: Boolean...|\n output: Boolean...| >>\n) = (.)";
const NATIVE: &str = "type Positive = {\n value: U64\n where .value > 0\n}\nplot identity (\n >> input: Positive...|\n output: Positive...| >>\n) = (.)";

#[test]
fn pure_packaging_preserves_exact_artifact_and_checked_expansion() {
    for source in [IDENTITY, NATIVE] {
        let compiled = ProtocolSourcePackage::compile(source.into(), &[]).unwrap();
        let direct = ProtocolSourcePackage {
            schema: PACKAGE_SCHEMA.into(),
            source: source.into(),
            specializations: Vec::new(),
        };
        let compiled = serde_json::to_vec(&compiled).unwrap();
        let direct = serde_json::to_vec(&direct).unwrap();
        assert_eq!(compiled, direct);
        let old = PreparedProtocolEntry::prepare(&compiled, "identity").unwrap();
        let new = PreparedProtocolEntry::prepare(&direct, "identity").unwrap();
        assert_eq!(old.package_digest(), new.package_digest());
        assert_eq!(old.artifact_id(), new.artifact_id());
        assert_eq!(old.expanded(), new.expanded());
        assert_eq!(old.resident(), new.resident());
        // Exercise the changed helper, including publication of checked Backs.
        let _ = kernel::Blueprint::prepare(source.into(), "identity");
    }
}

#[test]
fn pure_helper_still_refuses_bad_declarations_laws_and_plots() {
    let bad_declaration = format!("type Broken = {{\n value: U64\n value: U64\n}}\n{IDENTITY}");
    let bad_law = format!("type Broken = {{\n value: U64\n where .value\n}}\n{IDENTITY}");
    let bad_plot = "plot identity (\n >> input: Boolean...|\n output: Text...| >>\n) = (.)";
    for source in [bad_declaration.as_str(), bad_law.as_str(), bad_plot] {
        assert!(std::panic::catch_unwind(|| {
            kernel::Blueprint::prepare(source.into(), "identity")
        })
        .is_err());
    }
}

#[test]
fn pure_helper_keeps_source_and_entry_bounds_and_missing_entry_refusal() {
    for source in [String::new(), "x".repeat(MAXIMUM_SOURCE_BYTES + 1)] {
        assert!(
            std::panic::catch_unwind(|| { kernel::Blueprint::prepare(source, "identity") })
                .is_err()
        );
    }
    for entry in [
        String::new(),
        "absent".into(),
        "x".repeat(MAXIMUM_SOURCE_BYTES + 1),
    ] {
        assert!(std::panic::catch_unwind(|| {
            kernel::Blueprint::prepare(IDENTITY.into(), &entry)
        })
        .is_err());
    }
}
