//! Emit typed private staging and the only witness-gated publication boundary.
use super::*;
pub(super) fn root(
    out: &mut String,
    native: &CheckedNativeType,
    layout: &Layout<'_>,
    root_node: &str,
) -> R<()> {
    for n in &layout.nodes {
        writeln!(out, "#[allow(dead_code)] struct {}{{{}}}", n.name, n.fields).unwrap();
        writeln!(out,"#[allow(dead_code,unused_mut,unused_variables,unused_imports)] impl {}{{ fn new(caps:&[usize])->Result<Self,conduit_plot::rust_binding::PreparedDecodeRefusal>{{use conduit_plot::rust_binding::PreparedDecodeRefusal as E;Ok(Self{{{}}})}} fn cost(caps:&[usize])->Option<conduit_plot::rust_binding::PreparedStorageCost>{{let mut cost=conduit_plot::rust_binding::PreparedStorageCost::node(core::mem::size_of::<Self>());{} Some(cost)}}",n.name,n.init,n.cost).unwrap();
        writeln!(out,"fn step(&mut self,value:conduit_core::ValidatedCanonicalStructuredValue<'_>)->Result<bool,conduit_plot::rust_binding::PreparedDecodeRefusal>{{use conduit_plot::rust_binding::PreparedDecodeRefusal as E; {} }} fn take(&mut self)->{}{{ {} }} }}",n.step,n.rust,n.take).unwrap();
    }
    let rust = &layout.names[native.identity.as_str()];
    let storage = format!("{rust}ProfiledStorage");
    writeln!(out,"pub struct {storage}<'d>{{gate:conduit_plot::rust_binding::PreparedDynamicDecodeGate<'d>,root:{root_node},profile:conduit_plot::rust_binding::PreparedStorageProfile,cost:conduit_plot::rust_binding::PreparedStorageCost,published:bool}}").unwrap();
    writeln!(out,"impl {rust}{{pub fn prepare_profiled_storage<'d>(document:&'d conduit_plot::CheckedSyntaxDocument,requested:&conduit_plot::rust_binding::PreparedStorageProfile,bounds:conduit_plot::rust_binding::PreparedStorageBounds)->Result<{storage}<'d>,conduit_plot::rust_binding::PreparedDecodeRefusal>{{use conduit_plot::rust_binding::{{PreparedDecodeRefusal as E,PreparedStorageLimit as L}};let mut checked_types=alloc::vec::Vec::new();").unwrap();
    for id in &layout.dependencies {
        let n = layout
            .types
            .iter()
            .find(|n| n.identity.as_str() == id)
            .ok_or(Error::Bound)?;
        let bytes = n.value_type.canonical_bytes().map_err(|_| Error::Bound)?;
        let name = &layout.names[id];
        writeln!(out,"let dependency=conduit_plot::rust_binding::check_prepared_decode_native(document,{id:?},&{bytes:?},&{name}::value_contracts(),&{name}::invariants().map_err(|_|E::Preparation)?)?;checked_types.push(dependency.clone());").unwrap();
        if id == native.identity.as_str() {
            writeln!(out, "let checked=dependency;").unwrap();
        }
    }
    writeln!(out,"let schema=conduit_plot::rust_binding::discover_prepared_storage_schema(&checked_types,&checked.identity).map_err(|_|E::Preparation)?;let profile=conduit_plot::rust_binding::resolve_prepared_storage_profile(&schema,requested).map_err(|_|E::Preparation)?;let caps:alloc::vec::Vec<usize>=profile.capacities.iter().map(|site|match site.limit{{L::SequenceItems(n)|L::TextBytes(n)=>n}}).collect();").unwrap();
    writeln!(out,"let mut cost={root_node}::cost(&caps).ok_or(E::Preparation)?;cost.storage_bytes=cost.storage_bytes.checked_add(core::mem::size_of::<{storage}<'d>>()).ok_or(E::Preparation)?;let cost=cost.admit(&profile,bounds)?;let gate=conduit_plot::rust_binding::PreparedDynamicDecodeGate::new(checked,cost.maximum_steps,bounds.decode)?;Ok({storage}{{gate,root:{root_node}::new(&caps)?,profile,cost,published:false}})}}}}").unwrap();
    writeln!(out,"impl<'d>{storage}<'d>{{pub fn profile(&self)->&conduit_plot::rust_binding::PreparedStorageProfile{{&self.profile}}pub fn cost(&self)->conduit_plot::rust_binding::PreparedStorageCost{{self.cost}}pub fn canonical_capacity(&self)->usize{{self.gate.canonical_capacity()}}pub fn cancel(&mut self){{self.gate.cancel();}}").unwrap();
    writeln!(out,"pub fn step(&mut self,witness:&conduit_plot::rust_binding::AdmittedNativeCanonical<'_>)->Result<conduit_plot::rust_binding::PreparedDecodeProgress,conduit_plot::rust_binding::PreparedDecodeRefusal>{{use conduit_plot::rust_binding::PreparedDecodeRefusal as E;if self.published{{return Err(E::State)}}let value=self.gate.step_value(witness)?;match self.root.step(value){{Ok(complete)=>self.gate.advance(complete),Err(reason)=>Err(self.gate.fail(reason))}}}}").unwrap();
    writeln!(out,"#[allow(clippy::result_large_err)]pub fn finish_retained(mut self,witness:&conduit_plot::rust_binding::AdmittedNativeCanonical<'_>)->Result<({rust},Self),(conduit_plot::rust_binding::PreparedDecodeRefusal,Self)>{{if self.published{{return Err((conduit_plot::rust_binding::PreparedDecodeRefusal::State,self))}}if let Err(reason)=self.gate.finish(witness){{return Err((reason,self))}}let value=self.root.take();self.published=true;Ok((value,self))}}").unwrap();
    writeln!(out,"#[allow(clippy::result_large_err)]pub fn finish(self,witness:&conduit_plot::rust_binding::AdmittedNativeCanonical<'_>)->Result<{rust},(conduit_plot::rust_binding::PreparedDecodeRefusal,Self)>{{self.finish_retained(witness).map(|(value,retained)|{{drop(retained);value}})}}}}").unwrap();
    Ok(())
}
