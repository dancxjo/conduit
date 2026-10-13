use conduit_core::*;
use core::cmp::Ordering;
fn custom_family(name:&str)->QuantityFamilyDefinition {
    QuantityFamilyDefinition::point_delta(name,&format!("{name}Delta"),DimensionDefinition::new(&[DimensionTerm {anchor:DimensionDefinition::anchor("custom-temperature").unwrap(),power:1}]).unwrap()).unwrap()
}
fn origin_unit(family:QuantityFamilyDefinition,symbol:&str)->Unit {
    Unit::new_math_role(family,symbol,MathValue::ONE,MathValue::ZERO,PrefixPolicy::NONE,QuantityRole::Point).unwrap()
}
#[test]
fn self_contained_custom_affine_and_delta_laws_survive_transport() {
    let family=custom_family("Position");let root=origin_unit(family,"root");
    let unit=Unit::related_math_role(&root,"mark",MathValue::rational("5","9").unwrap(),MathValue::parse_decimal("273.15").unwrap(),PrefixPolicy::NONE,QuantityRole::Point).unwrap();
    let point=Quantity::new(9,unit.clone());
    let delta=Quantity::from_math_role(MathValue::integer(9),&unit,QuantityRole::Delta).unwrap();
    assert_eq!(point.convert_to_unit(&root).unwrap(),MathValue::parse_decimal("278.15").unwrap());
    assert_eq!(delta.convert_to_unit(&root).unwrap(),MathValue::integer(5));
    assert_eq!(point.compare(&delta),Err(QuantityConversionRefusal::IncompatibleQuantityRoles));
    assert_eq!(Quantity::decode(delta.encoded()).unwrap(),delta);
    let authored="PositionDelta(9, mark)";
    assert!(delta.matches_literal_evidence(authored));assert!(!delta.matches_literal_evidence("9mark"));
    let receipt=ExactQuantityConversionReceipt::from_checked(delta.clone(),root.clone(),authored,"root").unwrap();
    assert_eq!(receipt.result().unwrap().coordinate(),&MathValue::integer(5));
    let wrapper=QuantityConfigurationValue::new(delta.clone(),authored.into()).unwrap();
    assert_eq!(serde_json::from_str::<QuantityConfigurationValue>(&serde_json::to_string(&wrapper).unwrap()).unwrap(),wrapper);
    let role=quantity_role_info_id(family,QuantityRole::Delta).unwrap();
    let contract=CheckedValueContract::new(kind_id(&role),MAX_QUANTITY_ENCODED_BYTES as u32,vec![ValueConstraint::QuantityRange {minimum:Some(Box::new(delta.clone())),maximum:None,minimum_endpoint:IntervalEndpoint::Inclusive,maximum_endpoint:IntervalEndpoint::Inclusive}]).unwrap();
    assert!(contract.validate(point.encoded()).is_err());assert_eq!(contract.validate(delta.encoded()),Ok(()));
    assert_eq!(validate_primitive_info(&role,point.encoded()),Err(PrimitiveInfoRefusal::WrongQuantityRole));
    let unrelated=origin_unit(custom_family("Other"),"other");
    assert_eq!(point.compare(&Quantity::new(9,unrelated)),Err(QuantityConversionRefusal::IncompatibleQuantityFamilies));
}
#[test]
fn independent_origins_and_explicit_prefix_policies_preserve_affine_roles() {
    let family=custom_family("CustomTemperature");let origin=origin_unit(family,"base");
    let affine=Unit::related_math_role(&origin,"affine",MathValue::ONE,MathValue::integer(2000),PrefixPolicy::new(&[-3,3],&[10],2).unwrap(),QuantityRole::Point).unwrap();
    let milli=affine.with_decimal_prefix("maffine",-3).unwrap();
    assert_eq!(Quantity::new(1,milli).convert_to_unit(&origin).unwrap(),MathValue::parse_decimal("2000.000001").unwrap());
    let binary=affine.with_binary_prefix("Kiaffine",10).unwrap();
    assert_eq!(Quantity::new(1,binary).convert_to_unit(&origin).unwrap(),MathValue::integer(1_050_576));
    assert!(origin.with_decimal_prefix("kbase",3).is_err());
    let independent=origin_unit(family,"independent").with_named_origin("different").unwrap();
    assert_eq!(Quantity::new(1,affine).convert(&independent),Err(QuantityConversionRefusal::UnlinkedOrigin));
}
#[test]
fn large_coordinate_and_relation_cancellation_have_no_i128_boundary() {
    let family=custom_family("Large");let origin=origin_unit(family,"origin");
    let huge=MathValue::parse_decimal("1e1000").unwrap();
    let translated=Unit::related_math_role(&origin,"translated",MathValue::ONE,huge.clone(),PrefixPolicy::NONE,QuantityRole::Point).unwrap();
    let cancelled=Unit::related_math_role(&translated,"cancelled",MathValue::ONE,huge.checked_neg().unwrap(),PrefixPolicy::NONE,QuantityRole::Point).unwrap();
    assert!(cancelled.exact_point_offset().is_zero());
    let quantity=Quantity::from_math(huge.checked_add(&MathValue::ONE).unwrap(),&cancelled).unwrap();
    assert_eq!(quantity.compare(&quantity).unwrap(),Some(Ordering::Equal));
    assert_eq!(Quantity::decode(quantity.encoded()).unwrap(),quantity);
    assert!(quantity.to_i64(&origin).is_err());
}
#[test]
fn profile_custody_and_wire_forgery_are_checked_independently_of_meaning() {
    let family=custom_family("Profiled");let unit=origin_unit(family,"unit");let quantity=Quantity::new(1,unit.clone());
    let profile=quantity.as_ref().physical_profile();
    let kind=profile.info_id(family,QuantityRole::Point).unwrap();
    assert_eq!(validate_primitive_info(&kind,quantity.encoded()),Ok(()));
    assert!(intrinsic_numeric_maximum_bytes(&kind).unwrap()<1024);
    let broader=Quantity::new(1,unit.reprofile(PortableNumericProfile::MAXIMAL).unwrap());
    assert_eq!(quantity.as_ref().identity(),broader.as_ref().identity());
    assert!(validate_primitive_info(&kind,broader.encoded()).is_err());
    let mut forged=quantity.encode();let last=forged.len()-1;forged[last]^=1;
    assert!(QuantityRef::validate(&forged).is_err());
    assert!(QuantityRef::validate(&[0;9]).is_err());assert!(UnitRef::validate(&[0;3]).is_err());
}
#[test]
fn source_default_turn_and_symbolic_radian_are_exactly_related() {
    assert_eq!(builtin_angle_unit(),Unit::Turn);
    let degrees=Quantity::new(90,Unit::Degree);
    assert_eq!(degrees.convert_to_unit(&Unit::Turn).unwrap(),MathValue::rational("1","4").unwrap());
    let radians=Quantity::from_math(MathValue::pi().checked_div(&MathValue::integer(2)).unwrap(),&Unit::Radian).unwrap();
    assert_eq!(radians.compare(&degrees).unwrap(),Some(Ordering::Equal));
    assert_eq!(Quantity::new(1,Unit::Turn).convert_to_unit(&Unit::Radian).unwrap(),MathValue::integer(2).checked_mul(&MathValue::pi()).unwrap());
}
#[test]
fn changed_root_law_is_a_distinct_snapshot_even_for_named_origins() {
    let family=custom_family("Snapshot");
    let first=origin_unit(family,"root").with_named_origin("shared-name").unwrap();
    let changed=Unit::new_math_role(family,"root",MathValue::integer(2),MathValue::ZERO,PrefixPolicy::NONE,QuantityRole::Point).unwrap().with_named_origin("shared-name").unwrap();
    assert_ne!(first.reference_anchor(),changed.reference_anchor());
    assert_eq!(Quantity::new(1,first).compare(&Quantity::new(1,changed)),Err(QuantityConversionRefusal::UnlinkedOrigin));
}
#[test]
fn computed_quantity_configuration_rederives_graph_and_preserves_source_custody() {
    let scalar=SCALAR_INFO_ID.to_string();let unit=Unit::Radian;
    let graph=NumericComputation::new(vec![
        NumericComputationNode::literal(scalar.clone(),None,&MathValue::pi()).unwrap(),
        NumericComputationNode::literal(scalar.clone(),None,&MathValue::integer(2)).unwrap(),
        NumericComputationNode {semantic_kind:scalar,unit:None,reference_unit:None,operation:NumericComputationOperation::Apply {operation:NumericOperation::Divide,operands:vec![0,1]}},
        NumericComputationNode {semantic_kind:ANGLE_INFO_ID.into(),unit:Some(Box::new(unit.clone())),reference_unit:None,operation:NumericComputationOperation::Apply {operation:NumericOperation::QuantityConstruction,operands:vec![2]}},
    ],3).unwrap();
    let amount=MathValue::pi().checked_div(&MathValue::integer(2)).unwrap();
    let quantity=Quantity::from_math(amount,&unit).unwrap();
    let source="Angle(pi / 2, rad)";let ast=[19;32];
    let value=QuantityConfigurationValue::from_checked_computation(quantity.clone(),source.into(),ast,&graph).unwrap();
    assert_eq!(value.computation().unwrap().source_ast_identity(),ast);
    assert_eq!(value.value(),quantity);
    assert!(QuantityConfigurationValue::new(quantity.clone(),source.into()).is_none());
    assert!(QuantityConfigurationValue::from_checked_computation(Quantity::new(1,unit),source.into(),ast,&graph).is_none());
    let encoded=serde_json::to_string(&value).unwrap();
    assert_eq!(serde_json::from_str::<QuantityConfigurationValue>(&encoded).unwrap(),value);
    let mut forged:serde_json::Value=serde_json::from_str(&encoded).unwrap();forged["source"]=serde_json::Value::String("Angle(pi / 3, rad)".into());
    assert!(serde_json::from_value::<QuantityConfigurationValue>(forged).is_err());
}

#[test]
fn difference_only_unit_never_invents_a_point_origin() {
    let delta_unit=UnitDefinition::related_math_role(&Unit::Celsius,"Δ°C",MathValue::ONE,MathValue::ZERO,PrefixPolicy::NONE,QuantityRole::Delta).unwrap();
    assert!(delta_unit.admits_role(QuantityRole::Delta));
    assert!(!delta_unit.admits_role(QuantityRole::Point));
    let admitted_delta=Quantity::from_math_role(MathValue::integer(21),&delta_unit,QuantityRole::Delta).unwrap();
    let mut forged=admitted_delta.encode();
    forged[1]=1;
    let identity=semantic_digest_parts("value/quantity@2",&[&[1],admitted_delta.coordinate().encoded(),&delta_unit.identity()]);
    let identity_start=forged.len()-32;
    forged[identity_start..].copy_from_slice(&identity);
    assert!(matches!(QuantityRef::validate(&forged),Err(QuantityNumericRefusal::InvalidRole)));
    assert!(Quantity::from_math_role(MathValue::integer(21),&delta_unit,QuantityRole::Point).is_err());
    assert_eq!(Quantity::parse_plot_literal("21°C").unwrap().role(),QuantityRole::Point);
    assert_eq!(Quantity::parse_plot_literal("21°C").unwrap().convert_to_unit(&delta_unit),Err(QuantityConversionRefusal::IncompatibleQuantityRoles));
    let difference=Quantity::from_math_role(MathValue::integer(21),&Unit::Celsius,QuantityRole::Delta).unwrap();
    assert_eq!(difference.convert_to_unit(&delta_unit),Ok(MathValue::integer(21)));
    assert_eq!(difference.convert_to_unit(&Unit::Kelvin),Ok(MathValue::integer(21)));
    assert!(UnitDefinition::related_math_role(&delta_unit,"invented-point",MathValue::ONE,MathValue::ZERO,PrefixPolicy::NONE,QuantityRole::Point).is_err());
}
