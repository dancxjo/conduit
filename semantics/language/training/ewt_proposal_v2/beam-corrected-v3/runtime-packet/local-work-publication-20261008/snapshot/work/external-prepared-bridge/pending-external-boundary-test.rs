#[test]
fn external_descendants_count_toward_complete_root_and_generated_union() {
    use alloc::boxed::Box;
    use core::fmt::Write;
    fn descriptor(ty:&crate::CheckedNativeType, children:Vec<&'static NativeFamilyTypeDescriptor>)->&'static NativeFamilyTypeDescriptor {
        assert!(ty.invariants.is_empty());
        let contracts=ty.value_contracts.iter().map(|contract| {
            assert!(contract.contract.constraints.is_empty());
            NativeFamilyContractDescriptor { representation_path:Box::leak(contract.representation_path.clone().into_boxed_str()),value_kind:Box::leak(contract.contract.value_kind.as_str().into()),maximum_bytes:contract.contract.maximum_bytes,constraints:&[] }
        }).collect::<Vec<_>>();
        Box::leak(Box::new(NativeFamilyTypeDescriptor {type_bytes:Box::leak(ty.value_type.canonical_bytes().unwrap().into_boxed_slice()),laws:&[],contracts:Box::leak(contracts.into_boxed_slice()),children:Box::leak(children.into_boxed_slice()),external_edges:&[],conversion_profile:match ty.value_type.shape(){conduit_core::StructuredInfoTypeShape::Nominal{..}=>NativeFamilyConversionProfile::Nominal,conduit_core::StructuredInfoTypeShape::Record{..}=>NativeFamilyConversionProfile::Record,_=>panic!("fixture")},maximum_inline_bytes:128}))
    }
    let mut source=alloc::string::String::new();
    for group in 0..4 {
        for item in 0..63 { writeln!(source,"type Imported{group}Item{item} = U8\n").unwrap(); }
        writeln!(source,"type Imported{group} = {{").unwrap();
        for item in 0..62 {writeln!(source,"field{item}: Imported{group}Item{item}").unwrap();}
        writeln!(source,"}}\ntype Local{group} = {{ value: Imported{group} }}\n").unwrap();
    }
    writeln!(source,"type ExtraRoot = U8\ntype ImportedOversized = {{").unwrap();
    for item in 0..63 {writeln!(source,"field{item}: Imported0Item{item}").unwrap();}
    writeln!(source,"}}\ntype LocalOversized = {{ value: ImportedOversized }}\n").unwrap();
    let checked=crate::check_syntax_document(&crate::parse_syntax_document(&source),&crate::StartupCatalog::new()).unwrap();
    let ty=|name:&str|checked.native_types.iter().find(|ty|ty.name==name).unwrap();
    let mut external_types=Vec::new();let mut identities=Vec::new();let mut paths=Vec::new();let mut descriptors=Vec::new();
    for group in 0..4 {
        let root=ty(&alloc::format!("Imported{group}"));
        let children=(0..62).map(|item|descriptor(ty(&alloc::format!("Imported{group}Item{item}")),Vec::new())).collect();
        descriptors.push(descriptor(root,children));external_types.push(root.value_type.clone());identities.push(root.identity.as_str());paths.push(alloc::format!("dependency::Imported{group}"));
    }
    let oversized=ty("ImportedOversized");let children=(0..63).map(|item|descriptor(ty(&alloc::format!("Imported0Item{item}")),Vec::new())).collect();
    descriptors.push(descriptor(oversized,children));external_types.push(oversized.value_type.clone());identities.push(oversized.identity.as_str());paths.push("dependency::ImportedOversized".into());
    let bindings=identities.iter().zip(&paths).map(|(semantic_identity,path)|ExternalNativeRustBinding{semantic_identity,rust_type_path:path}).collect::<Vec<_>>();
    let prepared=bindings.iter().zip(&descriptors).map(|(binding,descriptor)|ExternalPreparedNativeRustBinding{semantic_identity:binding.semantic_identity,rust_type_path:binding.rust_type_path,descriptor}).collect::<Vec<_>>();
    let local=checked.native_types.iter().filter(|ty|ty.name.starts_with("Local")||ty.name=="ExtraRoot").cloned().collect::<Vec<_>>();
    let options=RustBindingOptions{prepared_family_roots:(0..4).map(|group|alloc::format!("Local{group}")).collect(),..Default::default()};
    // Four complete roots each consist of one local, one external and62 imported
    // children. All256 Types count even though only four descriptors are local.
    let generated=generate_rust_bindings_with_forms_and_external_prepared_bindings(&local,&[],&external_types,&bindings,&prepared,&options).unwrap();
    assert_eq!(generated.source.matches("_PREPARED_NATIVE_DESCRIPTOR: ").count(),4);
    let mut over_union=options.clone();over_union.prepared_family_roots.insert("ExtraRoot".into());
    assert!(matches!(generate_rust_bindings_with_forms_and_external_prepared_bindings(&local,&[],&external_types,&bindings,&prepared,&over_union),Err(ExternalRustBindingGenerationError::Generation(RustBindingGenerationError::InvalidSemanticType))));
    let over_root=RustBindingOptions{prepared_family_roots:["LocalOversized".into()].into(),..Default::default()};
    assert!(matches!(generate_rust_bindings_with_forms_and_external_prepared_bindings(&local,&[],&external_types,&bindings,&prepared,&over_root),Err(ExternalRustBindingGenerationError::Generation(RustBindingGenerationError::InvalidSemanticType))));
}
