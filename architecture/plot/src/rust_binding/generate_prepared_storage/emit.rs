//! Appended one-shot staging state; fields are private and never Native values.
use super::*;
use core::fmt::Write;
pub(super) fn emit(
    out: &mut String,
    native: &CheckedNativeType,
    rust: &str,
    layout: &Layout,
    types: &[CheckedNativeType],
    names: &BTreeMap<String, String>,
) -> R<()> {
    let storage = format!("{rust}PreparedStorage");
    let retained = if layout.branched {
        format!("{rust}PreparedRetained<'d>")
    } else {
        "conduit_plot::rust_binding::PreparedDecodeGate<'d>".into()
    };
    if layout.branched {
        writeln!(out,"pub struct {rust}PreparedRetained<'d> {{ gate: conduit_plot::rust_binding::PreparedDecodeGate<'d>,").unwrap();
        for (i, s) in layout.slots.iter().enumerate() {
            if matches!(s.kind, SlotKind::Text(_)) {
                writeln!(out, "_unused{i}: Option<String>,").unwrap();
            }
        }
        writeln!(out,"}} impl {rust}PreparedRetained<'_> {{ pub fn canonical_capacity(&self)->usize{{self.gate.canonical_capacity()}} }}").unwrap();
    }
    let text_bytes: usize = layout
        .slots
        .iter()
        .map(|s| match s.kind {
            SlotKind::Text(n) => n,
            _ => 0,
        })
        .sum();
    if text_bytes > 65536 {
        return Err(PreparedStorageGenerationError::Bound);
    }
    writeln!(out, "impl {rust} {{ pub const PREPARED_STORAGE_LEAF_STEPS: usize = {}; pub const PREPARED_STORAGE_TEXT_BYTES: usize = {text_bytes}; }}",layout.slots.len()).unwrap();
    writeln!(
        out,
        "pub struct {storage}<'d> {{ gate: conduit_plot::rust_binding::PreparedDecodeGate<'d>,"
    )
    .unwrap();
    for (i, s) in layout.slots.iter().enumerate() {
        writeln!(
            out,
            "f{i}: {},",
            if matches!(s.kind, SlotKind::Text(_)) && !layout.branched {
                s.rust.clone()
            } else {
                format!("Option<{}>", s.rust)
            }
        )
        .unwrap();
    }
    writeln!(out,"}}\nimpl {rust} {{ pub fn prepare_storage<'d>(document: &'d conduit_plot::CheckedSyntaxDocument, bounds: conduit_plot::rust_binding::PreparedDecodeBounds) -> Result<{storage}<'d>,conduit_plot::rust_binding::PreparedDecodeRefusal> {{").unwrap();
    writeln!(
        out,
        "use conduit_plot::rust_binding::PreparedDecodeRefusal as E;"
    )
    .unwrap();
    writeln!(
        out,
        "if bounds.text_bytes < {text_bytes} {{return Err(E::Preparation);}}"
    )
    .unwrap();
    for id in &layout.dependencies {
        let n = types.iter().find(|n| n.identity.as_str() == id).unwrap();
        let bytes = n
            .value_type
            .canonical_bytes()
            .map_err(|_| PreparedStorageGenerationError::Bound)?;
        let name = &names[id];
        writeln!(out,"{} conduit_plot::rust_binding::check_prepared_decode_native(document,{id:?}, &{bytes:?}, &{name}::value_contracts(), &{name}::invariants().map_err(|_| E::Preparation)?)?;",if id==native.identity.as_str(){"let checked ="}else{"let _ ="}).unwrap();
    }
    writeln!(out,"Ok({storage} {{ gate: conduit_plot::rust_binding::PreparedDecodeGate::new(checked,{},bounds)?,",layout.slots.len()).unwrap();
    for (i, s) in layout.slots.iter().enumerate() {
        match s.kind {
            SlotKind::Text(n) => writeln!(
                out,
                "f{i}: {},",
                if layout.branched {
                    format!("Some(String::with_capacity({n}))")
                } else {
                    format!("String::with_capacity({n})")
                }
            )
            .unwrap(),
            _ => writeln!(out, "f{i}: None,").unwrap(),
        }
    }
    writeln!(out, "}}) }} }}\nimpl<'d> {storage}<'d> {{").unwrap();
    writeln!(out,"pub fn step(&mut self,witness:&conduit_plot::rust_binding::AdmittedNativeCanonical<'_>)->Result<conduit_plot::rust_binding::PreparedDecodeProgress,conduit_plot::rust_binding::PreparedDecodeRefusal>{{").unwrap();
    writeln!(out,"use conduit_plot::rust_binding::PreparedDecodeRefusal as E; let (ordinal,value)=self.gate.step_value(witness)?; let result: Result<(),E> = (|| {{ match ordinal {{").unwrap();
    for (i, s) in layout.slots.iter().enumerate() {
        if !s.guard.is_empty() {
            writeln!(out, "{i} => {{ if {} {{ let leaf = {};", s.guard, s.path).unwrap();
        } else {
            writeln!(out, "{i} => {{ let leaf = {};", s.path).unwrap();
        }
        match &s.kind{
        SlotKind::Text(_)=>writeln!(out,"conduit_plot::rust_binding::prepared_decode_text({},leaf)?;",if layout.branched {format!("self.f{i}.as_mut().ok_or(E::State)?")} else {format!("&mut self.f{i}")}).unwrap(),
        SlotKind::Primitive(kind)=>writeln!(out,"self.f{i}=Some(conduit_plot::rust_binding::prepared_decode_primitive::<{}>(leaf,{kind:?})?);",s.rust).unwrap(),
        SlotKind::Tag(tags)=>{writeln!(out,"self.f{i}=Some(match leaf.representation().map_err(|_| E::Canonical)?.variant_tag().map_err(|_| E::Canonical)? {{").unwrap();for(index,tag)in tags.iter().enumerate(){writeln!(out,"{tag:?} => {index},").unwrap();}writeln!(out,"_ => return Err(E::Canonical), }});").unwrap();}
        SlotKind::Variant(cases)=>{writeln!(out,"self.f{i}=Some(match leaf.representation().map_err(|_| E::Canonical)?.variant_tag().map_err(|_| E::Canonical)? {{").unwrap();for(tag,variant)in cases{writeln!(out,"{tag:?} => {}::{variant},",s.rust).unwrap();}writeln!(out,"_ => return Err(E::Canonical), }});").unwrap();}
    }
        if !s.guard.is_empty() {
            writeln!(out, "}}").unwrap();
        }
        writeln!(out, "}},").unwrap();
    }
    writeln!(out,"_=>return Err(E::State), }} Ok(()) }})(); if let Err(error)=result {{return Err(self.gate.fail(error));}} Ok(self.gate.advance()) }}").unwrap();
    writeln!(out,"#[allow(clippy::result_large_err)]
pub fn finish(self,witness:&conduit_plot::rust_binding::AdmittedNativeCanonical<'_>)->Result<{rust},(conduit_plot::rust_binding::PreparedDecodeRefusal,Self)> {{ self.finish_retained(witness).map(|(value,retained)| {{ drop(retained); value }}) }}").unwrap();
    writeln!(out,"#[allow(clippy::result_large_err)]
pub fn finish_retained(mut self,witness:&conduit_plot::rust_binding::AdmittedNativeCanonical<'_>)->Result<({rust},{retained}),(conduit_plot::rust_binding::PreparedDecodeRefusal,Self)> {{").unwrap();
    let remainder = if layout.branched {
        let fields = layout
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s.kind, SlotKind::Text(_)))
            .map(|(i, _)| format!("_unused{i}:self.f{i}"))
            .collect::<Vec<_>>()
            .join(",");
        format!("{rust}PreparedRetained {{gate:self.gate,{fields}}}")
    } else {
        "self.gate".into()
    };
    writeln!(out,"use conduit_plot::rust_binding::PreparedDecodeRefusal as E; if let Err(error)=self.gate.finish(witness) {{return Err((error,self));}} let result: Result<{rust},E>=(|| Ok({}))(); match result {{Ok(value)=>Ok((value,{remainder})),Err(_)=>unreachable!(\"private completed staging slots\"),}} }}",layout.expression).unwrap();
    writeln!(out,"pub fn cancel(&mut self){{self.gate.cancel();}} pub fn canonical_capacity(&self)->usize{{self.gate.canonical_capacity()}} }}").unwrap();
    Ok(())
}
