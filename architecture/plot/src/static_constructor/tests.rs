use super::*;
use crate::glyph_notation_test_support::fixture;
use conduit_core::{kind_id, StructuredFieldValue};
use core::cell::Cell;

struct Owner {
    kind: Kind,
    ty: StructuredInfoType,
    value: StructuredInfoValue,
    calls: Cell<usize>,
}
impl StaticValueConstructor for Owner {
    type Refusal = ();
    fn contract(&self) -> Kind {
        self.kind.clone()
    }
    fn result_type(&self) -> StructuredInfoType {
        self.ty.clone()
    }
    fn prepare_configuration(&self, _: &[ConfigurationEntry]) -> Result<StructuredInfoValue, ()> {
        self.calls.set(self.calls.get() + 1);
        Ok(self.value.clone())
    }
}

fn value(ty: &StructuredInfoType) -> StructuredInfoValue {
    if let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
        return StructuredInfoValue::nominal(ty.clone(), value(representation)).unwrap();
    }
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("{ty:?}")
    };
    StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new(
            "payload",
            StructuredInfoValue::leaf(fields[0].value_type().clone(), b"x".to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
}

fn owner(profile: &ProfileCatalog, family: &crate::TypedLiteralFamily) -> Owner {
    let branch = &family.branches[0];
    Owner {
        kind: profile
            .canonical_kind(&branch.constructor_kind)
            .unwrap()
            .clone(),
        ty: branch.result_type.clone(),
        value: value(&branch.result_type),
        calls: Cell::new(0),
    }
}

#[test]
fn exact_installed_contract_is_required_before_executing_owner_preparation() {
    let (startup, profile, family) = fixture();
    let owner = owner(&profile, &family);
    let prepared = prepare_static_constructor(&owner, &startup, &profile, &[]).unwrap();
    assert_eq!(owner.calls.get(), 1);
    assert_eq!(prepared.value(), &owner.value);
    assert!(prepared.configuration().is_empty());
    assert!(matches!(
        prepare_static_constructor(&owner, &startup, &ProfileCatalog::new(), &[]),
        Err(StaticConstructorRefusal::Contract)
    ));
    assert!(matches!(
        prepare_static_constructor(&owner, &StartupCatalog::new(), &profile, &[]),
        Err(StaticConstructorRefusal::Contract)
    ));
    let extra = [ConfigurationEntry {
        key: "unknown".into(),
        value: ConfigurationValue::Bool(true),
    }];
    assert!(matches!(
        prepare_static_constructor(&owner, &startup, &profile, &extra),
        Err(StaticConstructorRefusal::Configuration)
    ));
    assert_eq!(owner.calls.get(), 1);
}

#[test]
fn stale_owner_revision_and_foreign_result_never_gain_a_receipt() {
    let (startup, profile, family) = fixture();
    let mut owner = owner(&profile, &family);
    owner.kind.kind_contract_revision = KindIdentity::from("fixture/literal@0");
    assert!(matches!(
        prepare_static_constructor(&owner, &startup, &profile, &[]),
        Err(StaticConstructorRefusal::Contract)
    ));
    assert_eq!(owner.calls.get(), 0);
    owner.kind.kind_contract_revision = family.branches[0].constructor_revision.clone();
    owner.value = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/text")).unwrap(),
        b"x".to_vec(),
    )
    .unwrap();
    assert!(matches!(
        prepare_static_constructor(&owner, &startup, &profile, &[]),
        Err(StaticConstructorRefusal::OutputType)
    ));
    assert_eq!(owner.calls.get(), 1);
}

#[test]
fn combined_configuration_pressure_refuses_before_owner_work_or_retention() {
    let (mut startup, mut profile, family) = fixture();
    let mut owner = owner(&profile, &family);
    owner.kind.kind_id = kind_id("fixture/configured-literal");
    owner.kind.startup_parameters = vec![conduit_core::FrontStartupParameter {
        name: "payload".into(),
        value_type: kind_id("value/text"),
        has_default: false,
    }];
    owner.kind.configuration = vec![conduit_core::KindConfigurationField {
        key: "payload".into(),
        default_value: ConfigurationValue::Text(String::new()),
        rule: conduit_core::KindConfigurationRule::TextBytes {
            maximum: MAXIMUM_CONFIGURATION_BYTES as u32,
        },
    }];
    profile.insert_kind(owner.kind.clone()).unwrap();
    startup
        .insert(crate::KindSignature {
            kind: "fixture/configured-literal".into(),
            startup_parameters: vec![crate::StartupParameterSignature {
                name: "payload".into(),
                value_type: "Text".into(),
                default: None,
            }],
        })
        .unwrap();
    let exact = [ConfigurationEntry {
        key: "payload".into(),
        value: ConfigurationValue::Text("x".repeat(MAXIMUM_CONFIGURATION_BYTES - "payload".len())),
    }];
    prepare_static_constructor(&owner, &startup, &profile, &exact).unwrap();
    assert_eq!(owner.calls.get(), 1);
    let exceeds = [ConfigurationEntry {
        key: "payload".into(),
        value: ConfigurationValue::Text("x".repeat(MAXIMUM_CONFIGURATION_BYTES)),
    }];
    assert!(matches!(
        prepare_static_constructor(&owner, &startup, &profile, &exceeds),
        Err(StaticConstructorRefusal::ConfigurationLimit)
    ));
    assert_eq!(owner.calls.get(), 1);
}
