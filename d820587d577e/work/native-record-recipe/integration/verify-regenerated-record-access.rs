//! Reviewed, insertion-only regeneration against a completed checked OUT_DIR.
//! This tool does not check Source and its manifest must be independently pinned.
use conduit_plot::rust_binding::{
    generate_prepared_native_converters, semantic_core, PreparedNativeConverterShape,
    RustBindingOptions,
};
use quote::ToTokens;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
use syn::{spanned::Spanned, Expr, ImplItem, Item, ItemImpl, Type};

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn prepared_name(item: &ItemImpl) -> Option<String> {
    if item.trait_.as_ref()?.1.segments.last()?.ident != "PreparedNativeRustBinding" {
        return None;
    }
    if let Type::Path(path) = item.self_ty.as_ref() {
        Some(path.path.segments.last()?.ident.to_string())
    } else {
        None
    }
}
fn impls(file: &syn::File) -> BTreeMap<String, &ItemImpl> {
    let mut found = BTreeMap::new();
    for item in &file.items {
        if let Item::Impl(item) = item {
            if let Some(name) = prepared_name(item) {
                assert!(
                    found.insert(name, item).is_none(),
                    "duplicate prepared impl"
                );
            }
        }
    }
    found
}
fn literal_owned(expr: &Expr) -> String {
    let Expr::Lit(expr) = expr else {
        panic!("expected literal")
    };
    let syn::Lit::Str(value) = &expr.lit else {
        panic!("expected string")
    };
    value.value()
}
fn included_file(expr: &Expr) -> String {
    let Expr::Macro(included) = expr else {
        panic!("expected original include_bytes")
    };
    assert!(included.mac.path.is_ident("include_bytes"));
    let concat: Expr = syn::parse2(included.mac.tokens.clone()).unwrap();
    let Expr::Macro(concat) = concat else {
        panic!("expected concat")
    };
    assert!(concat.mac.path.is_ident("concat"));
    use syn::parse::Parser;
    let args = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
        .parse2(concat.mac.tokens)
        .unwrap();
    assert_eq!(args.len(), 2);
    let Expr::Macro(env) = &args[0] else {
        panic!("expected OUT_DIR")
    };
    assert!(env.mac.path.is_ident("env"));
    assert_eq!(
        syn::parse2::<syn::LitStr>(env.mac.tokens.clone())
            .unwrap()
            .value(),
        "OUT_DIR"
    );
    let relative = literal_owned(&args[1]);
    let name = relative.strip_prefix('/').unwrap();
    assert_eq!(Path::new(name).components().count(), 1);
    name.to_owned()
}
fn main() {
 let args=std::env::args().collect::<Vec<_>>(); assert_eq!(args.len(),3);
 let original=Path::new(&args[1]); let updated=Path::new(&args[2]);
 let header=fs::read_to_string(original.join("semantic_types.rs")).unwrap();
 let new_header=fs::read_to_string(updated.join("semantic_types.rs")).unwrap();
 let parsed=syn::parse_file(&header).unwrap();let new_parsed=syn::parse_file(&new_header).unwrap();
 let originals=impls(&parsed);let new_impls=impls(&new_parsed);
 let mut type_files=BTreeMap::new(); let mut identities=BTreeMap::new();
 for item in &parsed.items { if let Item::Const(item)=item {
  if item.ident.to_string().ends_with("_SEMANTIC_TYPE"){type_files.insert(item.ident.to_string(),included_file(&item.expr));}
  if item.ident.to_string().ends_with("_SEMANTIC_TYPE_IDENTITY"){identities.insert(item.ident.to_string(),literal_owned(&item.expr));}
 }}
 let mut recovered=Vec::new();
 for name in originals.keys(){let constant=format!("{}_SEMANTIC_TYPE",name.to_uppercase()); let bytes=fs::read(original.join(&type_files[&constant])).unwrap(); let ty=semantic_core::StructuredInfoType::from_canonical_bytes(&bytes).unwrap(); assert_eq!(ty.canonical_bytes().unwrap(),bytes);let identity=semantic_core::kind_id(&identities[&format!("{constant}_IDENTITY")]);recovered.push((name,identity,ty));}
 let shapes=recovered.iter().map(|(name,identity,value_type)|PreparedNativeConverterShape{name,identity,value_type}).collect::<Vec<_>>();
 let regenerated=generate_prepared_native_converters(&shapes,&RustBindingOptions::default()).unwrap();assert_eq!(regenerated.len(),originals.len());assert_eq!(new_impls.len(),originals.len());
 for converter in &regenerated {let new:ItemImpl=syn::parse_str(&converter.source).unwrap();assert_eq!(new.to_token_stream().to_string(),new_impls[&converter.rust_name].to_token_stream().to_string(),"{}",converter.rust_name);}
 println!("{{\"original_header_sha256\":\"{}\",\"updated_header_sha256\":\"{}\",\"exact_regenerated_full_prepared_impls\":{},\"source_check_claim\":false}}",hash(header.as_bytes()),hash(new_header.as_bytes()),regenerated.len());
}
