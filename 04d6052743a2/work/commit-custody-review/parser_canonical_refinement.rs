//! Exact record-field preservation between raw Source output and Native refinement.
//! This changes only record schemas. It cannot add/drop/rename fields or choose
//! parser state. A result is not ancestry: the Session retains the parent Source
//! execution and performs full prepared Native target admission before use.
use crate::parser_canonical_composition::composer_requests;
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{
    validate_canonical_structured_value, PreparedStructuredComposer, StructuredInfoType,
    StructuredInfoTypeShape, ValidatedCanonicalStructuredValue,
};
use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeRustBinding};
use core::marker::PhantomData;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ParserRefinementLimits {
    pub maximum_node_bytes: usize,
    pub maximum_retained_requested_bytes: usize,
    pub maximum_preparation_requested_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct ParserRefinementReceipt {
    pub retained_requested_bytes_bound: usize,
    pub preparation_requested_bytes_bound: usize,
}
#[derive(Debug)]
pub(crate) enum ParserRefinementRefusal {
    Descriptor,
    Type,
    Pressure,
    Fields,
    Value,
}
struct Field {
    name: String,
    child: Box<Node>,
}
enum Node {
    Reuse,
    Record {
        composer: PreparedStructuredComposer,
        fields: Vec<Field>,
    },
}
fn add(a: usize, b: usize) -> Result<usize, ParserRefinementRefusal> {
    a.checked_add(b).ok_or(ParserRefinementRefusal::Pressure)
}
fn mul(a: usize, b: usize) -> Result<usize, ParserRefinementRefusal> {
    a.checked_mul(b).ok_or(ParserRefinementRefusal::Pressure)
}
fn request_bound(
    source: &StructuredInfoType,
    target: &StructuredInfoType,
    output: usize,
) -> Result<usize, ParserRefinementRefusal> {
    use ParserRefinementRefusal as R;
    if source == target {
        return Ok(0);
    }
    let (
        StructuredInfoTypeShape::Record { fields: source, .. },
        StructuredInfoTypeShape::Record {
            fields: target_fields,
            ..
        },
    ) = (source.shape(), target.shape())
    else {
        return Err(R::Type);
    };
    if source.len() != target_fields.len() || source.len() > 64 {
        return Err(R::Fields);
    }
    let mut bytes = composer_requests(target, output).map_err(|_| R::Pressure)?;
    bytes = add(bytes, mul(source.len(), core::mem::size_of::<Field>())?)?;
    for (source, target) in source.iter().zip(target_fields) {
        if source.name() != target.name() {
            return Err(R::Fields);
        }
        bytes = add(
            bytes,
            add(source.name().len(), core::mem::size_of::<Node>())?,
        )?;
        bytes = add(
            bytes,
            request_bound(source.value_type(), target.value_type(), output)?,
        )?;
    }
    Ok(bytes)
}
fn encoded_request_bound(
    source: &[u8],
    target: &[u8],
    output: usize,
) -> Result<usize, ParserRefinementRefusal> {
    use crate::parser_canonical_schema::{shape, Shape};
    use ParserRefinementRefusal as R;
    if source == target {
        return Ok(0);
    }
    let (Shape::Record(source_fields), Shape::Record(target_fields)) = (
        shape(source).map_err(|_| R::Type)?,
        shape(target).map_err(|_| R::Type)?,
    ) else {
        return Err(R::Type);
    };
    if source_fields.len() != target_fields.len() || source_fields.len() > 64 {
        return Err(R::Fields);
    }
    let mut bytes = crate::parser_canonical_composition::encoded_composer_requests(target, output)
        .map_err(|_| R::Pressure)?;
    bytes = add(
        bytes,
        mul(source_fields.len(), core::mem::size_of::<Field>())?,
    )?;
    for (source, target) in source_fields.zip(target_fields) {
        let (source_name, source_type) = source.map_err(|_| R::Type)?;
        let (target_name, target_type) = target.map_err(|_| R::Type)?;
        if source_name != target_name {
            return Err(R::Fields);
        }
        bytes = add(bytes, add(source_name.len(), core::mem::size_of::<Node>())?)?;
        bytes = add(
            bytes,
            encoded_request_bound(source_type, target_type, output)?,
        )?;
    }
    Ok(bytes)
}
fn prepare_node(
    source: &StructuredInfoType,
    target: &StructuredInfoType,
    output: usize,
) -> Result<Node, ParserRefinementRefusal> {
    use ParserRefinementRefusal as R;
    if source == target {
        return Ok(Node::Reuse);
    }
    let (
        StructuredInfoTypeShape::Record { fields: source, .. },
        StructuredInfoTypeShape::Record {
            fields: target_fields,
            ..
        },
    ) = (source.shape(), target.shape())
    else {
        return Err(R::Type);
    };
    if source.len() != target_fields.len() || source.len() > 64 {
        return Err(R::Fields);
    }
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(source.len())
        .map_err(|_| R::Pressure)?;
    for (source, target) in source.iter().zip(target_fields) {
        if source.name() != target.name() {
            return Err(R::Fields);
        }
        fields.push(Field {
            name: source.name().into(),
            child: Box::new(prepare_node(
                source.value_type(),
                target.value_type(),
                output,
            )?),
        });
    }
    let composer = PreparedStructuredComposer::new(target, output).map_err(|_| R::Type)?;
    Ok(Node::Record { composer, fields })
}
impl Node {
    fn compose<'a>(
        &'a mut self,
        source: ValidatedCanonicalStructuredValue<'a>,
    ) -> Result<ValidatedCanonicalStructuredValue<'a>, ParserRefinementRefusal> {
        use ParserRefinementRefusal as R;
        match self {
            Self::Reuse => Ok(source),
            Self::Record { composer, fields } => {
                let count = fields.len();
                let mut values = [source; 64];
                for (index, field) in fields.iter_mut().enumerate() {
                    let child = source
                        .record_field(&field.name)
                        .map_err(|_| R::Value)?
                        .ok_or(R::Fields)?;
                    values[index] = field.child.compose(child)?;
                }
                let output = composer.record(&values[..count]).map_err(|_| R::Value)?;
                validate_canonical_structured_value(output).map_err(|_| R::Value)
            }
        }
    }
}
pub(crate) struct PreparedParserCanonicalRefinement<S, T> {
    plan: Node,
    source_type: &'static [u8],
    target_type: &'static [u8],
    output: Vec<u8>,
    maximum_bytes: usize,
    receipt: ParserRefinementReceipt,
    binding: PhantomData<fn(S) -> T>,
}
impl<S: PreparedNativeRustBinding, T: PreparedNativeRustBinding>
    PreparedParserCanonicalRefinement<S, T>
{
    pub(crate) fn prepare(
        family: &PreparedNativeFamily,
        limits: ParserRefinementLimits,
    ) -> Result<Self, ParserRefinementRefusal> {
        Self::prepare_fields(family, &[], &[], limits)
    }
    /// Selects exact nested schemas only after complete root descriptor readiness.
    /// This performs representation refinement, never parser authorization. The
    /// driver must retain the original full parent Source execution and freshly
    /// admit the resulting complete Native query before any target consumes it.
    pub(crate) fn prepare_fields(
        family: &PreparedNativeFamily,
        source_path: &[&str],
        target_path: &[&str],
        limits: ParserRefinementLimits,
    ) -> Result<Self, ParserRefinementRefusal> {
        use ParserRefinementRefusal as R;
        if limits.maximum_node_bytes == 0
            || limits.maximum_node_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err(R::Pressure);
        }
        if !family.contains_descriptor(S::PREPARED_DESCRIPTOR)
            || !family.contains_descriptor(T::PREPARED_DESCRIPTOR)
        {
            return Err(R::Descriptor);
        }
        let source_type = crate::parser_canonical_schema::select_field(
            S::PREPARED_DESCRIPTOR.type_bytes,
            source_path,
        )
        .map_err(|_| R::Type)?;
        let target_type = crate::parser_canonical_schema::select_field(
            T::PREPARED_DESCRIPTOR.type_bytes,
            target_path,
        )
        .map_err(|_| R::Type)?;
        let source_decode =
            StructuredInfoType::canonical_decode_storage_bound(source_type).map_err(|_| R::Type)?;
        let target_decode =
            StructuredInfoType::canonical_decode_storage_bound(target_type).map_err(|_| R::Type)?;
        let decode = add(source_decode, target_decode)?;
        let retained = add(
            encoded_request_bound(source_type, target_type, limits.maximum_node_bytes)?,
            limits.maximum_node_bytes,
        )?;
        let preparation = add(decode, retained)?;
        if retained > limits.maximum_retained_requested_bytes
            || preparation > limits.maximum_preparation_requested_bytes
        {
            return Err(R::Pressure);
        }
        // Full recursive composition work is reserved before either Type decode.
        let source = StructuredInfoType::from_canonical_bytes(source_type).map_err(|_| R::Type)?;
        let target = StructuredInfoType::from_canonical_bytes(target_type).map_err(|_| R::Type)?;
        if add(
            request_bound(&source, &target, limits.maximum_node_bytes)?,
            limits.maximum_node_bytes,
        )? != retained
        {
            return Err(R::Type);
        }
        let plan = prepare_node(&source, &target, limits.maximum_node_bytes)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(limits.maximum_node_bytes)
            .map_err(|_| R::Pressure)?;
        Ok(Self {
            plan,
            source_type,
            target_type,
            output,
            maximum_bytes: limits.maximum_node_bytes,
            receipt: ParserRefinementReceipt {
                retained_requested_bytes_bound: retained,
                preparation_requested_bytes_bound: preparation,
            },
            binding: PhantomData,
        })
    }
    pub(crate) fn receipt(&self) -> ParserRefinementReceipt {
        self.receipt
    }
    pub(crate) fn compose<'a>(
        &'a mut self,
        source: ValidatedCanonicalStructuredValue<'a>,
    ) -> Result<&'a [u8], ParserRefinementRefusal> {
        use ParserRefinementRefusal as R;
        if source.type_bytes() != self.source_type {
            return Err(R::Type);
        }
        let value = self.plan.compose(source)?;
        if value.type_bytes() != self.target_type {
            return Err(R::Type);
        }
        let length = add(value.type_bytes().len(), value.value_node().len())?;
        if length > self.maximum_bytes {
            return Err(R::Pressure);
        }
        self.output.clear();
        self.output.extend_from_slice(value.type_bytes());
        self.output.extend_from_slice(value.value_node());
        Ok(&self.output)
    }
}
