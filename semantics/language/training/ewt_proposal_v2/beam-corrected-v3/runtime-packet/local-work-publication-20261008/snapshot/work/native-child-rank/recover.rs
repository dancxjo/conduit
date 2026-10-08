use conduit_plot::rust_binding::{generate_rust_bindings,RustBindingOptions,NativeBindingRefusal};use conduit_plot::rust_binding::semantic_core as conduit_core;use conduit_plot::CheckedNativeType;fn main(){let mut types=Vec::new();
static LanguageAnalysisRevisionId_PREPARED_NATIVE_LAWS: &[&[u8]] = &[
];
{let value_contracts={
        vec![
            conduit_plot::NativeTypeValueContract { representation_path: "".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
        ]
    };let invariants:Result<Vec<conduit_plot::PortableExpressionProgram>,NativeBindingRefusal>=(||{
        let mut laws = Vec::with_capacity(LanguageAnalysisRevisionId_PREPARED_NATIVE_LAWS.len());
        for encoded in LanguageAnalysisRevisionId_PREPARED_NATIVE_LAWS {
            laws.push(conduit_plot::PortableExpressionProgram::from_canonical_bytes(encoded).map_err(NativeBindingRefusal::InvalidInvariantProgram)?);
        }
        Ok(laws)
    })();types.push(CheckedNativeType{name:"LanguageAnalysisRevisionId".into(),identity:conduit_core::kind_id("type/LanguageAnalysisRevisionId@f38dc8ff7ca7b74eb7527be7d748f7078b84b87acb1d733249b4a2afe79f2926"),value_type:conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!("/home/dancxjo/conduit-4907-native-reuse/work/native-child-rank/native_binding_bytes_21.bin")).unwrap(),value_contracts,invariants:invariants.unwrap()});}
static LanguageParserBasis_PREPARED_NATIVE_LAWS: &[&[u8]] = &[
];
{let value_contracts={
        vec![
            conduit_plot::NativeTypeValueContract { representation_path: ".text".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".source_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".analysis_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
        ]
    };let invariants:Result<Vec<conduit_plot::PortableExpressionProgram>,NativeBindingRefusal>=(||{
        let mut laws = Vec::with_capacity(LanguageParserBasis_PREPARED_NATIVE_LAWS.len());
        for encoded in LanguageParserBasis_PREPARED_NATIVE_LAWS {
            laws.push(conduit_plot::PortableExpressionProgram::from_canonical_bytes(encoded).map_err(NativeBindingRefusal::InvalidInvariantProgram)?);
        }
        Ok(laws)
    })();types.push(CheckedNativeType{name:"LanguageParserBasis".into(),identity:conduit_core::kind_id("type/LanguageParserBasis@dc752c17047d61af9281fccb2cfd561b7b0fb116579e5396c51860ead6dba7e4"),value_type:conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!("/home/dancxjo/conduit-4907-native-reuse/work/native-child-rank/native_binding_bytes_103.bin")).unwrap(),value_contracts,invariants:invariants.unwrap()});}
static LanguageParserRelation_PREPARED_NATIVE_LAWS: &[&[u8]] = &[
];
{let value_contracts={
        vec![
            conduit_plot::NativeTypeValueContract { representation_path: ".subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
        ]
    };let invariants:Result<Vec<conduit_plot::PortableExpressionProgram>,NativeBindingRefusal>=(||{
        let mut laws = Vec::with_capacity(LanguageParserRelation_PREPARED_NATIVE_LAWS.len());
        for encoded in LanguageParserRelation_PREPARED_NATIVE_LAWS {
            laws.push(conduit_plot::PortableExpressionProgram::from_canonical_bytes(encoded).map_err(NativeBindingRefusal::InvalidInvariantProgram)?);
        }
        Ok(laws)
    })();types.push(CheckedNativeType{name:"LanguageParserRelation".into(),identity:conduit_core::kind_id("type/LanguageParserRelation@c9d64c56db2aa40e5ff56bcade30f884753be23364e24138d3e66fb5fa2485f6"),value_type:conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!("/home/dancxjo/conduit-4907-native-reuse/work/native-child-rank/native_binding_bytes_102.bin")).unwrap(),value_contracts,invariants:invariants.unwrap()});}
static LanguageParserSubtype_PREPARED_NATIVE_LAWS: &[&[u8]] = &[
];
{let value_contracts={
        vec![
            conduit_plot::NativeTypeValueContract { representation_path: "".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
        ]
    };let invariants:Result<Vec<conduit_plot::PortableExpressionProgram>,NativeBindingRefusal>=(||{
        let mut laws = Vec::with_capacity(LanguageParserSubtype_PREPARED_NATIVE_LAWS.len());
        for encoded in LanguageParserSubtype_PREPARED_NATIVE_LAWS {
            laws.push(conduit_plot::PortableExpressionProgram::from_canonical_bytes(encoded).map_err(NativeBindingRefusal::InvalidInvariantProgram)?);
        }
        Ok(laws)
    })();types.push(CheckedNativeType{name:"LanguageParserSubtype".into(),identity:conduit_core::kind_id("type/LanguageParserSubtype@3a59ad54c8e19001575142e2145e9aba256728c49f35644f1a938075ba91c645"),value_type:conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!("/home/dancxjo/conduit-4907-native-reuse/work/native-child-rank/native_binding_bytes_101.bin")).unwrap(),value_contracts,invariants:invariants.unwrap()});}
static LanguageParserWindow8RawBeam_PREPARED_NATIVE_LAWS: &[&[u8]] = &[
];
{let value_contracts={
        vec![
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate0.state.basis.text".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate0.state.basis.source_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate0.state.basis.analysis_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate0.state.relation0.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate0.state.relation1.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate0.state.relation2.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate0.state.relation3.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate0.state.relation4.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate0.state.relation5.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate0.state.relation6.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate0.state.relation7.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate1.state.basis.text".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate1.state.basis.source_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate1.state.basis.analysis_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate1.state.relation0.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate1.state.relation1.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate1.state.relation2.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate1.state.relation3.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate1.state.relation4.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate1.state.relation5.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate1.state.relation6.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate1.state.relation7.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate2.state.basis.text".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate2.state.basis.source_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate2.state.basis.analysis_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate2.state.relation0.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate2.state.relation1.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate2.state.relation2.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate2.state.relation3.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate2.state.relation4.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate2.state.relation5.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate2.state.relation6.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate2.state.relation7.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate3.state.basis.text".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate3.state.basis.source_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate3.state.basis.analysis_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate3.state.relation0.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate3.state.relation1.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate3.state.relation2.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate3.state.relation3.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate3.state.relation4.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate3.state.relation5.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate3.state.relation6.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".candidate3.state.relation7.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
        ]
    };let invariants:Result<Vec<conduit_plot::PortableExpressionProgram>,NativeBindingRefusal>=(||{
        let mut laws = Vec::with_capacity(LanguageParserWindow8RawBeam_PREPARED_NATIVE_LAWS.len());
        for encoded in LanguageParserWindow8RawBeam_PREPARED_NATIVE_LAWS {
            laws.push(conduit_plot::PortableExpressionProgram::from_canonical_bytes(encoded).map_err(NativeBindingRefusal::InvalidInvariantProgram)?);
        }
        Ok(laws)
    })();types.push(CheckedNativeType{name:"LanguageParserWindow8RawBeam".into(),identity:conduit_core::kind_id("type/LanguageParserWindow8RawBeam@2930a75e81400c7f79ae5b19257c8cd5fccd58f6fad0c95fa005be90b4081343"),value_type:conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!("/home/dancxjo/conduit-4907-native-reuse/work/native-child-rank/native_binding_bytes_247.bin")).unwrap(),value_contracts,invariants:invariants.unwrap()});}
static LanguageParserWindow8RawHypothesis_PREPARED_NATIVE_LAWS: &[&[u8]] = &[
];
{let value_contracts={
        vec![
            conduit_plot::NativeTypeValueContract { representation_path: ".state.basis.text".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".state.basis.source_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".state.basis.analysis_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".state.relation0.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".state.relation1.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".state.relation2.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".state.relation3.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".state.relation4.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".state.relation5.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".state.relation6.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".state.relation7.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
        ]
    };let invariants:Result<Vec<conduit_plot::PortableExpressionProgram>,NativeBindingRefusal>=(||{
        let mut laws = Vec::with_capacity(LanguageParserWindow8RawHypothesis_PREPARED_NATIVE_LAWS.len());
        for encoded in LanguageParserWindow8RawHypothesis_PREPARED_NATIVE_LAWS {
            laws.push(conduit_plot::PortableExpressionProgram::from_canonical_bytes(encoded).map_err(NativeBindingRefusal::InvalidInvariantProgram)?);
        }
        Ok(laws)
    })();types.push(CheckedNativeType{name:"LanguageParserWindow8RawHypothesis".into(),identity:conduit_core::kind_id("type/LanguageParserWindow8RawHypothesis@5451415a7813f65093dc7cfd9f383eca3f74ee16d0ccf91a8124fb74b3049f4a"),value_type:conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!("/home/dancxjo/conduit-4907-native-reuse/work/native-child-rank/native_binding_bytes_246.bin")).unwrap(),value_contracts,invariants:invariants.unwrap()});}
static LanguageParserWindow8RawState_PREPARED_NATIVE_LAWS: &[&[u8]] = &[
];
{let value_contracts={
        vec![
            conduit_plot::NativeTypeValueContract { representation_path: ".basis.text".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".basis.source_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".basis.analysis_revision".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".relation0.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".relation1.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".relation2.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".relation3.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".relation4.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".relation5.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".relation6.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
            conduit_plot::NativeTypeValueContract { representation_path: ".relation7.subtype".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 32, vec![]).expect("generated checked contract") },
        ]
    };let invariants:Result<Vec<conduit_plot::PortableExpressionProgram>,NativeBindingRefusal>=(||{
        let mut laws = Vec::with_capacity(LanguageParserWindow8RawState_PREPARED_NATIVE_LAWS.len());
        for encoded in LanguageParserWindow8RawState_PREPARED_NATIVE_LAWS {
            laws.push(conduit_plot::PortableExpressionProgram::from_canonical_bytes(encoded).map_err(NativeBindingRefusal::InvalidInvariantProgram)?);
        }
        Ok(laws)
    })();types.push(CheckedNativeType{name:"LanguageParserWindow8RawState".into(),identity:conduit_core::kind_id("type/LanguageParserWindow8RawState@24bb0b3ffaf2f7043900e06f60f3e139184112c82a04e30030e8a71c9c55a442"),value_type:conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!("/home/dancxjo/conduit-4907-native-reuse/work/native-child-rank/native_binding_bytes_132.bin")).unwrap(),value_contracts,invariants:invariants.unwrap()});}
static LanguageTextId_PREPARED_NATIVE_LAWS: &[&[u8]] = &[
];
{let value_contracts={
        vec![
            conduit_plot::NativeTypeValueContract { representation_path: "".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
        ]
    };let invariants:Result<Vec<conduit_plot::PortableExpressionProgram>,NativeBindingRefusal>=(||{
        let mut laws = Vec::with_capacity(LanguageTextId_PREPARED_NATIVE_LAWS.len());
        for encoded in LanguageTextId_PREPARED_NATIVE_LAWS {
            laws.push(conduit_plot::PortableExpressionProgram::from_canonical_bytes(encoded).map_err(NativeBindingRefusal::InvalidInvariantProgram)?);
        }
        Ok(laws)
    })();types.push(CheckedNativeType{name:"LanguageTextId".into(),identity:conduit_core::kind_id("type/LanguageTextId@55e85261877d7f291a3e7b6e26e7e7eb4afffcccf1f7a48f27f65149c75888da"),value_type:conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!("/home/dancxjo/conduit-4907-native-reuse/work/native-child-rank/native_binding_bytes_34.bin")).unwrap(),value_contracts,invariants:invariants.unwrap()});}
static LanguageTextRevisionId_PREPARED_NATIVE_LAWS: &[&[u8]] = &[
];
{let value_contracts={
        vec![
            conduit_plot::NativeTypeValueContract { representation_path: "".into(), contract: conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/text"), 64, vec![conduit_core::ValueConstraint::CanonicalMembership { members: vec![vec![]], negated: true }]).expect("generated checked contract") },
        ]
    };let invariants:Result<Vec<conduit_plot::PortableExpressionProgram>,NativeBindingRefusal>=(||{
        let mut laws = Vec::with_capacity(LanguageTextRevisionId_PREPARED_NATIVE_LAWS.len());
        for encoded in LanguageTextRevisionId_PREPARED_NATIVE_LAWS {
            laws.push(conduit_plot::PortableExpressionProgram::from_canonical_bytes(encoded).map_err(NativeBindingRefusal::InvalidInvariantProgram)?);
        }
        Ok(laws)
    })();types.push(CheckedNativeType{name:"LanguageTextRevisionId".into(),identity:conduit_core::kind_id("type/LanguageTextRevisionId@1fec2752b047f47b28888f46ed793d8834a34d6caec87223f8f6fe75c1faf935"),value_type:conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!("/home/dancxjo/conduit-4907-native-reuse/work/native-child-rank/native_binding_bytes_35.bin")).unwrap(),value_contracts,invariants:invariants.unwrap()});}
static LanguageUniversalDependencyRelation_PREPARED_NATIVE_LAWS: &[&[u8]] = &[
];
{let value_contracts={
        vec![
        ]
    };let invariants:Result<Vec<conduit_plot::PortableExpressionProgram>,NativeBindingRefusal>=(||{
        let mut laws = Vec::with_capacity(LanguageUniversalDependencyRelation_PREPARED_NATIVE_LAWS.len());
        for encoded in LanguageUniversalDependencyRelation_PREPARED_NATIVE_LAWS {
            laws.push(conduit_plot::PortableExpressionProgram::from_canonical_bytes(encoded).map_err(NativeBindingRefusal::InvalidInvariantProgram)?);
        }
        Ok(laws)
    })();types.push(CheckedNativeType{name:"LanguageUniversalDependencyRelation".into(),identity:conduit_core::kind_id("type/LanguageUniversalDependencyRelation@7ce27302d5b825277bbd754d0bce8bbc9e71f5d64bc4f0af184fc862dbd8cf2e"),value_type:conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!("/home/dancxjo/conduit-4907-native-reuse/work/native-child-rank/native_binding_bytes_18.bin")).unwrap(),value_contracts,invariants:invariants.unwrap()});}
let generated=generate_rust_bindings(&types,&RustBindingOptions{prepared_family_roots:["LanguageParserWindow8RawBeam".into()].into(),..Default::default()}).unwrap();std::fs::write("work/native-child-rank/bindings.rs",generated.source).unwrap();println!("types={} laws={}",types.len(),types.iter().map(|t|t.invariants.len()).sum::<usize>());}