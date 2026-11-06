//! Recursive typed staging templates; physical counts are selected at preparation.
use super::{
    generate::{
        generate_rust_bindings, rust_pascal_identifier, rust_snake_identifier, rust_type, unit_type,
    },
    *,
};
use crate::CheckedNativeType;
use alloc::{
    collections::{BTreeMap, BTreeSet},
    format,
    string::{String, ToString},
    vec::Vec,
};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape as Shape};
use core::fmt::Write;
mod branch;
mod emit;
type Error = PreparedStorageGenerationError;
type R<T> = Result<T, Error>;
struct Node {
    name: String,
    rust: String,
    fields: String,
    init: String,
    step: String,
    take: String,
    cost: String,
}
struct Layout<'a> {
    types: &'a [CheckedNativeType],
    options: &'a RustBindingOptions,
    names: BTreeMap<String, String>,
    sites: Vec<PreparedStorageCapacity>,
    dependencies: BTreeSet<String>,
    nodes: Vec<Node>,
    prefix: String,
}
/// Append a one-shot materializer whose runtime preparation profile is resolved
/// against exact checked metadata. Ordinary bindings and semantic maxima remain
/// unchanged. Preparation reserves every inactive alternative and physical
/// element; step and retained finish allocate, reallocate and deallocate nothing.
/// Both the returned Native value and retained storage must outlive that boundary.
/// Work is charged per actual leaf/header step, not per semantic maximum. Each
/// transition still validates the canonical input and traverses borrowed views;
/// it does not promise constant-time processing of arbitrary canonical bytes.
/// Boxed payloads and shapes reported unsupported by discovery refuse generation.
/// Physical templates, work and Text retain the existing 4096/4096/65536 ceilings.
/// `storage_bytes` conservatively includes typed staging, output backing,
/// canonical bytes and the retained profile, excluding allocator bookkeeping.
pub fn generate_rust_bindings_with_profiled_storage(
    types: &[CheckedNativeType],
    options: &RustBindingOptions,
    root_names: &[&str],
) -> R<RustBindingModule> {
    let mut module = generate_rust_bindings(types, options)?;
    let prefix = if options.type_prefix.is_empty() {
        String::new()
    } else {
        rust_pascal_identifier(&options.type_prefix)?
    };
    let names = types
        .iter()
        .map(|n| {
            Ok((
                n.identity.as_str().to_string(),
                format!("{prefix}{}", rust_pascal_identifier(&n.name)?),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, RustBindingGenerationError>>()?;
    let mut seen = BTreeSet::new();
    for root in root_names {
        if !seen.insert(*root) {
            return Err(Error::Unsupported("duplicate root".into()));
        }
        let native = types
            .iter()
            .find(|n| n.name == *root)
            .ok_or_else(|| Error::Missing((*root).into()))?;
        let schema = discover_prepared_storage_schema(types, &native.identity)
            .map_err(|e| Error::Unsupported(format!("profile discovery: {e:?}")))?;
        if !schema.unsupported().is_empty() {
            return Err(Error::Unsupported(format!("{:?}", schema.unsupported())));
        }
        let mut layout = Layout {
            types,
            options,
            names: names.clone(),
            sites: schema.sites().to_vec(),
            dependencies: BTreeSet::new(),
            nodes: Vec::new(),
            prefix: format!("{}ProfileNode", names[native.identity.as_str()]),
        };
        let node = layout.walk(&native.value_type, &mut Vec::new(), None)?;
        emit::root(&mut module.source, native, &layout, &node)?;
    }
    Ok(module)
}
impl Layout<'_> {
    fn dependency(&mut self, ty: &StructuredInfoType, id: &str) -> R<()> {
        let n = self
            .types
            .iter()
            .find(|n| n.identity.as_str() == id)
            .ok_or_else(|| Error::Missing(id.into()))?;
        if n.value_type != *ty {
            return Err(Error::Unsupported("conflicting Native metadata".into()));
        }
        self.dependencies.insert(id.into());
        Ok(())
    }
    fn cap(&self, path: &[NativePathStep]) -> R<usize> {
        self.sites
            .iter()
            .position(|s| s.path == path)
            .ok_or(Error::Bound)
    }
    fn walk(
        &mut self,
        ty: &StructuredInfoType,
        path: &mut Vec<NativePathStep>,
        constructor: Option<String>,
    ) -> R<String> {
        if path.len() > 64 || self.nodes.len() >= 4096 {
            return Err(Error::Bound);
        }
        let name = format!("{}{}", self.prefix, self.nodes.len());
        let rust = constructor
            .clone()
            .map(Ok)
            .unwrap_or_else(|| rust_type(ty, &self.names))?;
        // Reserve the ordinal before descending. Children are independent typed
        // storage, including all alternatives below a zero-capacity sequence.
        let index = self.nodes.len();
        self.nodes.push(Node {
            name: name.clone(),
            rust: rust.clone(),
            fields: String::new(),
            init: String::new(),
            step: String::new(),
            take: String::new(),
            cost: String::new(),
        });
        let mut node = Node {
            name: name.clone(),
            rust: rust.clone(),
            fields: String::new(),
            init: String::new(),
            step: String::new(),
            take: String::new(),
            cost: String::new(),
        };
        match ty.shape() {
            Shape::Leaf(kind) => {
                if kind.as_str() == "value/text" {
                    let cap = self.cap(path)?;
                    node.fields = "value:Option<String>,limit:usize".into();
                    node.init = format!("value:Some({{let value=String::with_capacity(caps[{cap}]);if value.capacity()!=caps[{cap}]{{return Err(E::Preparation)}}value}}),limit:caps[{cap}]");
                    node.step="if value.primitive_representation_bytes(\"value/text\").map_err(|_|E::Canonical)?.len()>self.limit{return Err(E::Pressure)} conduit_plot::rust_binding::prepared_decode_text(self.value.as_mut().ok_or(E::State)?,value)?; Ok(true)".into();
                    node.cost=format!("cost.text_bytes=caps[{cap}]; cost.storage_bytes=cost.storage_bytes.checked_add(caps[{cap}])?;");
                } else {
                    node.fields = format!("value:Option<{rust}>");
                    node.init = "value:None".into();
                    node.step=format!("self.value=Some(conduit_plot::rust_binding::prepared_decode_primitive::<{rust}>(value,{:?})?); Ok(true)",kind.as_str());
                }
                node.take = "self.value.take().expect(\"completed private slot\")".into();
            }
            Shape::Nominal {
                schema,
                representation,
            } => {
                self.dependency(ty, schema.as_str())?;
                path.push(NativePathStep::Representation);
                let child = self.walk(representation, path, None)?;
                path.pop();
                node.fields = format!("child:{child}");
                node.init = format!("child:{child}::new(caps)?");
                node.step =
                    "self.child.step(value.nominal_representation().map_err(|_|E::Canonical)?)"
                        .into();
                node.take = format!("{rust}(self.child.take())");
                node.cost = format!("cost=cost.checked_add({child}::cost(caps)?)?;");
            }
            Shape::Record { schema, fields } => {
                self.dependency(ty, schema.as_str())?;
                node.fields = "next:usize,".into();
                node.init = "next:0,".into();
                node.step = "match self.next {".into();
                let mut values = Vec::new();
                for (i, field) in fields.iter().enumerate() {
                    path.push(NativePathStep::Field(field.name().into()));
                    let child = self.walk(field.value_type(), path, None)?;
                    path.pop();
                    write!(node.fields, "f{i}:{child},").unwrap();
                    write!(node.init, "f{i}:{child}::new(caps)?,").unwrap();
                    write!(node.step,"{i}=>{{if self.f{i}.step(value.record_field({:?}).map_err(|_|E::Canonical)?.ok_or(E::Canonical)?)? {{self.next+=1;}} Ok(self.next=={})}},",field.name(),fields.len()).unwrap();
                    write!(node.cost, "cost=cost.checked_add({child}::cost(caps)?)?;").unwrap();
                    values.push(format!(
                        "{}:self.f{i}.take()",
                        rust_snake_identifier(field.name())?
                    ));
                }
                node.step.push_str("_=>Ok(true)}");
                node.take = format!("{rust}{{{}}}", values.join(","));
            }
            Shape::Sequence {
                element,
                maximum_items,
                ..
            } => {
                let cap = self.cap(path)?;
                path.push(NativePathStep::Each);
                let child = self.walk(element, path, None)?;
                path.pop();
                let element_rust = rust_type(element, &self.names)?;
                node.fields=format!("children:alloc::vec::Vec<{child}>, output:Option<BoundedSequence<{element_rust},{maximum_items}>>, length:Option<usize>, next:usize");
                node.init=format!("children:{{let mut children=alloc::vec::Vec::with_capacity(caps[{cap}]);if children.capacity()!=caps[{cap}]{{return Err(E::Preparation)}}for _ in 0..caps[{cap}]{{children.push({child}::new(caps)?);}}children}},output:Some({{let values=alloc::vec::Vec::<{element_rust}>::with_capacity(caps[{cap}]);if core::mem::size_of::<{element_rust}>()!=0 && values.capacity()!=caps[{cap}]{{return Err(E::Preparation)}}BoundedSequence::try_from_preallocated_vec(values).map_err(|_|E::Preparation)?}}),length:None,next:0");
                node.step="if self.length.is_none(){let length=value.collection_length().map_err(|_|E::Canonical)? as usize; if length>self.children.len(){return Err(E::Pressure)} self.length=Some(length);return Ok(length==0)} let length=self.length.ok_or(E::State)?;if self.next>=length{return Ok(true)} if self.children[self.next].step(value.collection_index(u16::try_from(self.next).map_err(|_|E::Canonical)?).map_err(|_|E::Canonical)?.ok_or(E::Canonical)?)? {self.next+=1;} Ok(self.next==length)".into();
                node.take="{let mut output=self.output.take().expect(\"private sequence backing\");for child in self.children.iter_mut().take(self.length.expect(\"completed sequence\")){if output.push_preallocated(child.take()).is_err(){unreachable!(\"checked actual length and prepared backing\")}} output}".into();
                node.cost=format!("let count=caps[{cap}];cost.sequence_items=count; cost.storage_bytes=cost.storage_bytes.checked_add(count.checked_mul(core::mem::size_of::<{element_rust}>())?)?;if count!=0{{cost=cost.checked_add({child}::cost(caps)?.checked_mul(count)?)?;}}");
            }
            Shape::Collection { element, length } => {
                path.push(NativePathStep::Each);
                let child = self.walk(element, path, None)?;
                path.pop();
                node.fields = format!("children:[{child};{length}], next:usize, started:bool");
                node.init = format!(
                    "children:{{let mut children:[Option<{child}>;{length}]=core::array::from_fn(|_|None);for child in &mut children{{*child=Some({child}::new(caps)?);}}children.map(|child|child.expect(\"prepared fixed element\"))}},next:0,started:false"
                );
                node.step=format!("if !self.started{{if value.collection_length().map_err(|_|E::Canonical)?!={length}{{return Err(E::Canonical)}} self.started=true;return Ok({length}==0)}} if self.next>={length}{{return Ok(true)}} if self.children[self.next].step(value.collection_index(u16::try_from(self.next).map_err(|_|E::Canonical)?).map_err(|_|E::Canonical)?.ok_or(E::Canonical)?)?{{self.next+=1;}} Ok(self.next=={length})");
                node.take = "core::array::from_fn(|i|self.children[i].take())".into();
                node.cost =
                    format!("cost=cost.checked_add({child}::cost(caps)?.checked_mul({length})?)?;");
            }
            Shape::Variant { .. } => branch::build(self, ty, path, &mut node)?,
        }
        self.nodes[index] = node;
        Ok(name)
    }
}
