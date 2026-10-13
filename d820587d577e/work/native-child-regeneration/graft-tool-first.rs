//! Reviewed, insertion-only regeneration against a completed checked OUT_DIR.
//! This tool does not check Source and its manifest must be independently pinned.
use conduit_plot::rust_binding::{generate_prepared_native_converters, semantic_core, PreparedNativeConverterShape, RustBindingOptions};
use quote::ToTokens;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
use syn::{spanned::Spanned, Expr, ImplItem, Item, ItemImpl, Type};

fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn prepared_name(item: &ItemImpl) -> Option<String> {
    if item.trait_.as_ref()?.1.segments.last()?.ident != "PreparedNativeRustBinding" { return None; }
    if let Type::Path(path) = item.self_ty.as_ref() { Some(path.path.segments.last()?.ident.to_string()) } else { None }
}
fn impls(file: &syn::File) -> BTreeMap<String, &ItemImpl> {
    let mut found = BTreeMap::new();
    for item in &file.items { if let Item::Impl(item) = item { if let Some(name) = prepared_name(item) { assert!(found.insert(name, item).is_none(), "duplicate prepared impl"); } } }
    found
}
fn literal_owned(expr: &Expr) -> String {
    let Expr::Lit(expr) = expr else { panic!("expected literal") };
    let syn::Lit::Str(value) = &expr.lit else { panic!("expected string") };
    value.value()
}
fn included_file(expr: &Expr) -> String {
    let Expr::Macro(included) = expr else { panic!("expected original include_bytes") };
    assert!(included.mac.path.is_ident("include_bytes"));
    let concat: Expr = syn::parse2(included.mac.tokens.clone()).unwrap();
    let Expr::Macro(concat) = concat else { panic!("expected concat") };
    assert!(concat.mac.path.is_ident("concat"));
    use syn::parse::Parser;
    let args = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated.parse2(concat.mac.tokens).unwrap();
    assert_eq!(args.len(), 2);
    let Expr::Macro(env) = &args[0] else { panic!("expected OUT_DIR") };
    assert!(env.mac.path.is_ident("env"));
    assert_eq!(syn::parse2::<syn::LitStr>(env.mac.tokens.clone()).unwrap().value(), "OUT_DIR");
    let relative = literal_owned(&args[1]);
    let name = relative.strip_prefix('/').unwrap();
    assert_eq!(Path::new(name).components().count(), 1);
    name.to_owned()
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(args.len(), 6, "ORIGINAL_OUT_DIR SELECTED_INPUT_RECEIPT NEW_OUT_DIR MANIFEST EXPECTED_HEADER_SHA256");
    let original = Path::new(&args[1]); let receipt_path = Path::new(&args[2]); let output = Path::new(&args[3]); let manifest_path = Path::new(&args[4]);
    assert!(!output.exists(), "never overwrite a completed output");
    let header_bytes = fs::read(original.join("semantic_types.rs")).unwrap(); assert_eq!(hash(&header_bytes), args[5]);
    let header = std::str::from_utf8(&header_bytes).unwrap();
    let parsed = syn::parse_file(header).unwrap(); let originals = impls(&parsed);
    let mut type_files = BTreeMap::new(); let mut identities = BTreeMap::new(); let mut descriptors = Vec::new();
    for item in &parsed.items {
        match item {
            Item::Const(item) if item.ident.to_string().ends_with("_SEMANTIC_TYPE") => { type_files.insert(item.ident.to_string(), included_file(&item.expr)); },
            Item::Const(item) if item.ident.to_string().ends_with("_SEMANTIC_TYPE_IDENTITY") => { identities.insert(item.ident.to_string(), literal_owned(&item.expr)); },
            Item::Static(item) if item.ident.to_string().ends_with("_PREPARED_NATIVE_DESCRIPTOR") => descriptors.push(item.ident.to_string().strip_suffix("_PREPARED_NATIVE_DESCRIPTOR").unwrap().to_owned()),
            _ => {},
        }
    }
    descriptors.sort(); assert_eq!(descriptors.len(), originals.len());
    let mut recovered = Vec::new();
    for name in &descriptors {
        assert!(originals.contains_key(name));
        let constant = format!("{}_SEMANTIC_TYPE", name.to_uppercase());
        let bytes = fs::read(original.join(&type_files[&constant])).unwrap();
        let value_type = semantic_core::StructuredInfoType::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(value_type.canonical_bytes().unwrap(), bytes);
        let identity = semantic_core::kind_id(&identities[&format!("{constant}_IDENTITY")]);
        recovered.push((name, identity, value_type));
    }
    // The independently recorded selected build.rs uses Default binding layout.
    // Any layout difference is additionally rejected by whole ordinary-impl equality.
    let shapes = recovered.iter().map(|(name, identity, value_type)| PreparedNativeConverterShape { name, identity, value_type }).collect::<Vec<_>>();
    let regenerated = generate_prepared_native_converters(&shapes, &RustBindingOptions::default()).unwrap();
    let mut edits = Vec::new(); let mut transformations = Vec::new();
    for converter in regenerated {
        let mut new: ItemImpl = syn::parse_str(&converter.source).unwrap();
        assert_eq!(prepared_name(&new).as_deref(), Some(converter.rust_name.as_str()));
        let old = originals[&converter.rust_name];
        assert!(!old.items.iter().any(|item| matches!(item, ImplItem::Fn(method) if method.sig.ident == "from_borrowed_prepared_with_children")));
        let index = new.items.iter().position(|item| matches!(item, ImplItem::Fn(method) if method.sig.ident == "from_borrowed_prepared_with_children")).unwrap();
        let scoped = new.items.remove(index); let range = scoped.span().byte_range();
        let scoped_source = converter.source.get(range).unwrap();
        let ordinary = old.to_token_stream().to_string();
        assert_eq!(ordinary, new.to_token_stream().to_string(), "ordinary converter/layout changed: {}", converter.rust_name);
        let offset = old.brace_token.span.close().byte_range().start;
        let insertion = format!("\n{scoped_source}\n");
        transformations.push(serde_json::json!({"rust_name":converter.rust_name,"original_impl_sha256":hash(header[old.span().byte_range()].as_bytes()),"ordinary_item_tokens_sha256":hash(ordinary.as_bytes()),"regenerated_ordinary_item_tokens_sha256":hash(new.to_token_stream().to_string().as_bytes()),"scoped_method_sha256":hash(scoped_source.as_bytes()),"original_insertion_byte_offset":offset}));
        edits.push((offset, insertion));
    }
    edits.sort_by_key(|edit| edit.0);
    let mut updated = String::with_capacity(header.len() + edits.iter().map(|edit| edit.1.len()).sum::<usize>());
    let mut prior = 0;
    for (offset, insertion) in &edits { assert!(*offset >= prior); updated.push_str(&header[prior..*offset]); updated.push_str(insertion); prior = *offset; }
    updated.push_str(&header[prior..]);
    // Independently remove every exact recorded insertion and recover every
    // original byte, including all bindings/default methods and metadata.
    let mut restored = Vec::with_capacity(header.len()); let mut original_prior = 0; let mut updated_prior = 0;
    for (offset, insertion) in &edits { let length = offset - original_prior; restored.extend_from_slice(&updated.as_bytes()[updated_prior..updated_prior + length]); updated_prior += length; assert_eq!(&updated[updated_prior..updated_prior+insertion.len()], insertion); updated_prior += insertion.len(); original_prior = *offset; }
    restored.extend_from_slice(&updated.as_bytes()[updated_prior..]); assert_eq!(restored, header_bytes);
    let updated_parsed = syn::parse_file(&updated).unwrap(); let updated_impls = impls(&updated_parsed); assert_eq!(updated_impls.len(), originals.len());
    for item in updated_impls.values() { assert_eq!(item.items.iter().filter(|item| matches!(item, ImplItem::Fn(method) if method.sig.ident == "from_borrowed_prepared_with_children")).count(), 1); }
    let receipt_bytes = fs::read(receipt_path).unwrap(); let receipt: serde_json::Value = serde_json::from_slice(&receipt_bytes).unwrap();
    let selected = receipt["files"].as_object().unwrap(); let mut source_inputs = BTreeMap::new();
    let mut runtime_input_deltas = BTreeMap::new();
    for (name, expected) in selected {
        let observed = hash(&fs::read(name).unwrap());
        let runtime_only = name.ends_with("/semantics/language/src/lib.rs") || name.ends_with("/semantics/language/src/parser_session_execution.rs");
        if runtime_only && observed != expected.as_str().unwrap() { runtime_input_deltas.insert(name.clone(), serde_json::json!({"original":expected,"current":observed,"scope":"runtime Rust only; normal Cargo recompilation still required"})); }
        else { assert_eq!(observed, expected.as_str().unwrap(), "generation input changed: {name}"); }
        source_inputs.insert(name.clone(), expected.as_str().unwrap().to_owned());
    }
    let sdk_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let generator_paths = ["architecture/plot/src/rust_binding/generate_prepared_converters.rs","architecture/plot/src/rust_binding/generate_prepared_family.rs","architecture/plot/src/rust_binding/generate.rs","architecture/plot/src/rust_binding/generate_options.rs","architecture/plot/src/rust_binding/prepared_family.rs","architecture/plot/src/rust_binding/prepared_family_admission.rs","architecture/core/src/structured_info/borrowed.rs","architecture/core/src/structured_info/borrowed/visit.rs","tools/prepared-native-converters/src/main.rs","tools/prepared-native-converters/Cargo.toml","tools/prepared-native-converters/Cargo.lock"];
    let generator_inputs = generator_paths.into_iter().map(|name| (name, hash(&fs::read(sdk_root.join(name)).unwrap()))).collect::<BTreeMap<_,_>>();
    fs::create_dir_all(output).unwrap();
    let mut original_files = BTreeMap::new(); let mut output_files = BTreeMap::new();
    for entry in fs::read_dir(original).unwrap() {
        let entry = entry.unwrap(); assert!(entry.file_type().unwrap().is_file(), "expected flat checked OUT_DIR");
        let name = entry.file_name().into_string().unwrap(); let bytes = fs::read(entry.path()).unwrap(); let digest = hash(&bytes); original_files.insert(name.clone(), digest.clone());
        if name == "semantic_types.rs" { fs::write(output.join(&name), updated.as_bytes()).unwrap(); output_files.insert(name, hash(updated.as_bytes())); }
        else { fs::write(output.join(&name), &bytes).unwrap(); assert_eq!(hash(&fs::read(output.join(&name)).unwrap()), digest); output_files.insert(name, digest); }
    }
    fs::write(manifest_path.with_extension("original-header.rs"), &header_bytes).unwrap();
    let manifest = serde_json::json!({"schema":"conduit.prepared-native-converter-graft.v1","original_header_sha256":hash(&header_bytes),"updated_header_sha256":hash(updated.as_bytes()),"original_selected_input_receipt_sha256":hash(&receipt_bytes),"original_selected_inputs":source_inputs,"unselected_runtime_input_deltas":runtime_input_deltas,"generator_inputs":generator_inputs,"original_output_files":original_files,"output_files":output_files,"counts":{"ordinary_types":type_files.len(),"prepared_descriptors":descriptors.len(),"scoped_converters":edits.len(),"metadata_blobs":output_files.keys().filter(|name| name.starts_with("native_binding_bytes_")).count(),"source_files":receipt["source_files"]},"transformations":transformations,"original_bytes_retained":true,"source_check_claim":false,"scope":"Representation-only insertion against completed checked output; whole manifest requires independent approval/pinning; no new Source check or runtime acceptance"});
    fs::write(manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    println!("PASS {} original Types/{} exact ordinary converters; all original bytes retained; {} full output files hashed/copied",type_files.len(),descriptors.len(),manifest["output_files"].as_object().unwrap().len());
}
