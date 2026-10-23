//! Canonical numeric policy is enforced on actual values, not resource claims.
use conduit_ai::{ModelTensorIoRefusal as R, ModelTensorNumericPolicy as Policy, *};
use conduit_core::{
    BoundedResourceRef, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity,
};
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn signature(element: TensorElement, scalar: bool, policy: Policy) -> ModelSignature {
    let axes = if scalar {
        vec![]
    } else {
        vec![ModelAxisConstraint::new(
            ModelDimensionConstraint::bounded(4, 1).unwrap(),
            TensorAxisRole::Feature,
        )
        .unwrap()]
    };
    let constraint =
        ModelTensorConstraint::from_parts(vec![element], axes, 4 * element.byte_width(), policy)
            .unwrap();
    let port = |name: &str| {
        ModelPortConstraint::from_parts(
            name.into(),
            "data/tensor@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::tensor(constraint.clone()).unwrap(),
        )
        .unwrap()
    };
    ModelSignature::from_parts(
        "test/explicit-numeric-policy@1".into(),
        1,
        vec![ModelOperation::Infer],
        vec![port("input")],
        vec![port("output")],
    )
    .unwrap()
}
fn bundle(name: &str, element: TensorElement, scalar: bool, bytes: &[u8]) -> NamedTensorBundle {
    let (dimensions, axes) = if scalar {
        (vec![], vec![])
    } else {
        (
            vec![bytes.len() as u64 / element.byte_width()],
            vec![TensorAxis {
                role: TensorAxisRole::Feature,
                identity: None,
                unit: None,
            }],
        )
    };
    NamedTensorBundle::from_values(vec![NamedTensor {
        name: name.into(),
        value: TensorValue {
            element,
            dimensions: BoundedSequence::try_from_iter(dimensions).unwrap(),
            axes: BoundedSequence::try_from_iter(axes).unwrap(),
            content_digest: tensor_content_digest(bytes),
            backing: TensorBacking::Inline(BoundedBytes::new(bytes).unwrap()),
        },
    }])
    .unwrap()
}
fn check(
    element: TensorElement,
    scalar: bool,
    policy: Policy,
    bytes: &[u8],
    expected: Result<(), R>,
) {
    let owner = signature(element, scalar, policy);
    let prepared = PreparedModelTensorIo::prepare(&owner).unwrap();
    let reader = PreparedNamedTensorBundleView::prepare().unwrap();
    let input = bundle("input", element, scalar, bytes);
    let output = bundle("output", element, scalar, bytes);
    assert_eq!(owner.validate_named_inputs(&input), expected);
    assert_eq!(owner.validate_named_outputs(&output), expected);
    let input_bytes = input.encode().unwrap();
    let output_bytes = output.clone().encode().unwrap();
    let original = (input_bytes.clone(), output_bytes.clone());
    let iv = reader.read(&input_bytes).unwrap();
    let ov = reader.read(&output_bytes).unwrap();
    let (results, storage) = allocation_probe::observe(|| {
        (
            prepared.validate_inputs(&iv),
            prepared.validate_outputs(&ov),
            prepared.validate_owned_outputs(&output),
        )
    });
    assert_eq!(results, (expected.clone(), expected.clone(), expected));
    assert_eq!((storage.allocations, storage.reallocations), (0, 0));
    assert_eq!((input_bytes, output_bytes), original);
}
#[test]
fn finite_policy_accepts_signed_zero_subnormals_extremes_and_integer_scalars() {
    let finite_f32 = [0_u32, 0x8000_0000, 1, 0x7f7f_ffff];
    for bits in finite_f32 {
        check(
            TensorElement::F32,
            true,
            Policy::FiniteOnly,
            &bits.to_le_bytes(),
            Ok(()),
        );
    }
    let bytes = finite_f32
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    check(
        TensorElement::F32,
        false,
        Policy::FiniteOnly,
        &bytes,
        Ok(()),
    );
    let bits = [0_u64, 0x8000_0000_0000_0000, 1, 0x7fef_ffff_ffff_ffff];
    for value in bits {
        check(
            TensorElement::F64,
            true,
            Policy::FiniteOnly,
            &value.to_le_bytes(),
            Ok(()),
        );
    }
    let bytes = bits
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    check(
        TensorElement::F64,
        false,
        Policy::FiniteOnly,
        &bytes,
        Ok(()),
    );
    check(
        TensorElement::I64,
        true,
        Policy::FiniteOnly,
        &i64::MIN.to_le_bytes(),
        Ok(()),
    );
}
#[test]
fn nonfinite_owned_and_borrowed_inputs_outputs_refuse_but_ieee_preserves_bits() {
    for bits in [0x7f80_0000_u32, 0xff80_0000, 0x7fc0_0001, 0x7f80_0001] {
        let bytes = bits.to_le_bytes();
        check(
            TensorElement::F32,
            true,
            Policy::FiniteOnly,
            &bytes,
            Err(R::NonFinite),
        );
        check(TensorElement::F32, true, Policy::Ieee, &bytes, Ok(()));
        let tail = [1_f32.to_le_bytes(), bytes].concat();
        check(
            TensorElement::F32,
            false,
            Policy::FiniteOnly,
            &tail,
            Err(R::NonFinite),
        );
    }
    for bits in [
        0x7ff0_0000_0000_0000_u64,
        0xfff0_0000_0000_0000,
        0x7ff8_0000_0000_0001,
        0x7ff0_0000_0000_0001,
    ] {
        let bytes = bits.to_le_bytes();
        check(
            TensorElement::F64,
            true,
            Policy::FiniteOnly,
            &bytes,
            Err(R::NonFinite),
        );
        check(TensorElement::F64, true, Policy::Ieee, &bytes, Ok(()));
        let tail = [1_f64.to_le_bytes(), bytes].concat();
        check(
            TensorElement::F64,
            false,
            Policy::FiniteOnly,
            &tail,
            Err(R::NonFinite),
        );
    }
}
#[test]
fn numeric_policy_is_required_canonical_information_and_changes_signature_identity() {
    let ieee = signature(TensorElement::F32, false, Policy::Ieee);
    let finite = signature(TensorElement::F32, false, Policy::FiniteOnly);
    assert_ne!(
        ieee.semantic_digest().unwrap(),
        finite.semantic_digest().unwrap()
    );
    let encoded = finite.clone().encode().unwrap();
    assert_eq!(ModelSignature::decode(&encoded).unwrap(), finite);
    let text = format!("{finite:?}");
    assert!(text.contains("FiniteOnly"));
}
#[test]
fn floating_resource_descriptor_does_not_prove_finite_contents_or_grant_access() {
    for element in [TensorElement::F32, TensorElement::F64] {
        let mut input = bundle(
            "input",
            element,
            true,
            &vec![0; element.byte_width() as usize],
        );
        // Only the immutable descriptor is presented; no resolver or Host call exists here.
        let mut value = input.get()[0].clone();
        let tensor = &mut value.value;
        tensor.backing = TensorBacking::Resource(BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(tensor.content_digest),
            content_profile: tensor.resource_profile().into(),
            access_class: "test/unread-floating-tensor".into(),
            extent: ResourceExtent {
                bytes: element.byte_width(),
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([9; 32]),
                expires_at: None,
            },
        });
        input = NamedTensorBundle::from_values(vec![value.clone()]).unwrap();
        value.name = "output".into();
        let output = NamedTensorBundle::from_values(vec![value]).unwrap();
        let bytes = input.clone().encode().unwrap();
        let ob = output.clone().encode().unwrap();
        let reader = PreparedNamedTensorBundleView::prepare().unwrap();
        let iv = reader.read(&bytes).unwrap();
        let ov = reader.read(&ob).unwrap();
        for policy in [Policy::Ieee, Policy::FiniteOnly] {
            let owner = signature(element, true, policy);
            let expected = if policy == Policy::FiniteOnly {
                Err(R::NumericPolicyUnverifiable)
            } else {
                Ok(())
            };
            assert_eq!(owner.validate_named_inputs(&input), expected);
            assert_eq!(owner.validate_named_outputs(&output), expected);
            let prepared = PreparedModelTensorIo::prepare(&owner).unwrap();
            let (results, storage) = allocation_probe::observe(|| {
                (
                    prepared.validate_inputs(&iv),
                    prepared.validate_outputs(&ov),
                )
            });
            assert_eq!(results, (expected.clone(), expected));
            assert_eq!((storage.allocations, storage.reallocations), (0, 0));
        }
    }
}

#[test]
fn relation_evidence_cannot_bypass_the_same_canonical_numeric_policy() {
    for policy in [Policy::Ieee, Policy::FiniteOnly] {
        let owner = signature(TensorElement::F32, true, policy);
        let variables = |name: &str| {
            RelationVariableIdentities::new(
                BoundedSequence::try_from_iter([
                    RelationVariableIdentity::new(name.into()).unwrap()
                ])
                .unwrap(),
            )
            .unwrap()
        };
        let value = owner.inputs.get()[0].value.clone();
        let relation = ModelRelationSignature::from_parts(
            "test/numeric-relation".into(),
            1,
            owner.semantic_digest().unwrap(),
            ["input", "output"]
                .into_iter()
                .map(|name| {
                    RelationVariable::from_parts(name.into(), "data/tensor@1".into(), value.clone())
                        .unwrap()
                })
                .collect(),
            vec![SupportedRelationQuery::new(
                variables("input"),
                variables("output"),
                RelationQueryMode::InferPosterior,
                RelationResultProfile::Deterministic,
                1,
                16,
            )
            .unwrap()],
        )
        .unwrap();
        let artifact = ModelArtifact {
            architecture_profile: "test/numeric-relation".into(),
            format_profile: "model/reference".into(),
            precision_profile: "f32".into(),
            state_schema_version: 1,
            signature_identity: owner.semantic_digest().unwrap(),
            content: BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest([1; 32]),
                content_profile: "model/reference".into(),
                access_class: "test/read-only".into(),
                extent: ResourceExtent {
                    bytes: 4,
                    items: None,
                },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest([2; 32]),
                    expires_at: None,
                },
            },
        };
        let tensor = bundle(
            "input",
            TensorElement::F32,
            true,
            &f32::INFINITY.to_le_bytes(),
        )
        .get()[0]
            .value
            .clone();
        let query = RelationQuery::from_parts(
            [3; 32],
            artifact.content_identity(),
            None,
            relation.semantic_digest().unwrap(),
            vec![RelationEvidence::from_parts(
                "input".into(),
                RelationValue::tensor(tensor).unwrap(),
            )
            .unwrap()],
            vec!["output".into()],
            RelationQueryMode::InferPosterior,
            RelationResultProfile::Deterministic,
            RandomnessProfile::Deterministic,
            1,
            16,
        )
        .unwrap();
        let actual = relation.realize(
            &artifact,
            &query,
            HostRelationTerminal::NoResult(RelationTerminal::Cancelled),
        );
        if policy == Policy::FiniteOnly {
            assert_eq!(actual, Err(RelationRefusal::InvalidValue));
        } else {
            assert_eq!(
                actual,
                Ok(RelationQueryOutcome::NoResult(RelationTerminal::Cancelled))
            );
        }
    }
}
