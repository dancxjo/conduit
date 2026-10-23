//! Exact borrowed graph schema. Indexed resolution does no tree search.
use super::super::Refusal;
use conduit_core::{PortDescriptor, PortDirection, PortTemporal};
use patchbay_graph::*;
#[path = "nested/mod.rs"]
mod nested;
pub(super) use nested::Nested;
#[derive(Clone, Copy)]
pub(super) enum Shape<'a> {
    Null,
    Bool(bool),
    U64(u64),
    I64(i64),
    DecimalU64(u64),
    DecimalI64(i64),
    String(&'a str),
    Object(&'static [&'static str]),
    Sequence(usize),
    RawSequence(usize),
}
#[derive(Clone, Copy)]
pub(super) enum Node<'a> {
    Envelope(Option<&'a PatchbayGraph>),
    Reply(Option<&'a PatchbayGraph>),
    Payload(Option<&'a PatchbayGraph>),
    Nested(Nested<'a>),
    Str(&'a str),
    U64(u64),
    I64(i64),
    Bool(bool),
    Null,
    Descriptor(&'a PortDescriptor),
    Temporal(PortTemporal),
    Flow(bool),
    Graph(&'a PatchbayGraph),
    Front(&'a PatchbayFrontPort),
    Port(&'a PatchbayPort),
    Composition(&'a PatchbayComposition),
    Binding(&'a PatchbayCompositionBinding),
    Gear(&'a PatchbayGear),
    Cord(&'a PatchbayCord),
    Origin(&'a PatchbayCordSource),
    Span(&'a conduit_plot::Span),
    Fronts(&'a [PatchbayFrontPort]),
    Ports(&'a [PatchbayPort]),
    Compositions(&'a [PatchbayComposition]),
    Bindings(&'a [PatchbayCompositionBinding]),
    Gears(&'a [PatchbayGear]),
    Cords(&'a [PatchbayCord]),
    Controls(&'a [FaceControl]),
    Strings(&'a [alloc::string::String]),
}
impl<'a> Node<'a> {
    pub(super) fn shape(self) -> Result<Shape<'a>, Refusal> {
        use Node::*;
        Ok(match self {
            Envelope(_) => Shape::RawSequence(3),
            Reply(_) => Shape::Object(&["Graph"]),
            Payload(_) => Shape::Object(&["graph"]),
            Nested(v) => return v.shape(),
            Str(v) => Shape::String(v),
            U64(v) => Shape::U64(v),
            I64(v) => Shape::I64(v),
            Bool(v) => Shape::Bool(v),
            Null => Shape::Null,
            Descriptor(v) => Shape::Object(if v.abnormal_kind.is_some() {
                &[
                    "port_id",
                    "value_kind",
                    "direction",
                    "temporal",
                    "abnormal_kind",
                ]
            } else {
                &["port_id", "value_kind", "direction", "temporal"]
            }),
            Temporal(PortTemporal::Value) => Shape::String("Value"),
            Temporal(PortTemporal::Current) => Shape::String("Current"),
            Temporal(PortTemporal::Flow { .. }) => Shape::Object(&["Flow"]),
            Flow(_) => Shape::Object(&["closes"]),
            Graph(_) => Shape::Object(&[
                "source_document_id",
                "checked_plot_id",
                "expanded_plot_id",
                "plot_name",
                "front_inputs",
                "front_outputs",
                "compositions",
                "gears",
                "cords",
            ]),
            Front(_) => Shape::Object(&["identity", "descriptor", "value_contract"]),
            Port(_) => Shape::Object(&["identity", "gear_id", "descriptor", "value_contract"]),
            Composition(_) => Shape::Object(&[
                "identity",
                "gear_name",
                "back_name",
                "checked_plot_id",
                "inputs",
                "outputs",
                "input_bindings",
                "output_bindings",
            ]),
            Binding(_) => Shape::Object(&["front_port", "internal_port"]),
            Gear(_) => Shape::Object(&[
                "identity",
                "gear_id",
                "kind_id",
                "kind_contract_revision",
                "source_plot",
                "plot_path",
                "inputs",
                "outputs",
                "controls",
            ]),
            Cord(_) => Shape::Object(&[
                "source_origin",
                "identity",
                "source_port",
                "sink_port",
                "value_kind",
                "temporal",
            ]),
            Origin(_) => Shape::Object(&[
                "source_document_id",
                "checked_plot_id",
                "source_plot",
                "plot_path",
                "source_span",
                "origin",
            ]),
            Span(_) => Shape::Object(&["start", "end", "line", "column", "end_line", "end_column"]),
            Fronts(v) => Shape::Sequence(v.len()),
            Ports(v) => Shape::Sequence(v.len()),
            Compositions(v) => Shape::Sequence(v.len()),
            Bindings(v) => Shape::Sequence(v.len()),
            Gears(v) => Shape::Sequence(v.len()),
            Cords(v) => Shape::Sequence(v.len()),
            Controls(v) => Shape::Sequence(v.len()),
            Strings(v) => Shape::Sequence(v.len()),
        })
    }
    pub(super) fn child(self, i: usize) -> Result<Self, Refusal> {
        use Node::*;
        let bad = || Refusal::Order;
        Ok(match self {
            Envelope(v) => match i {
                0 => U64(2),
                1 => Str("CAPSEQ1"),
                2 => Reply(v),
                _ => return Err(bad()),
            },
            Reply(v) if i == 0 => Payload(v),
            Payload(v) if i == 0 => v.map_or(Null, Graph),
            Nested(v) => return v.child(i),
            Descriptor(v) => match i {
                0 => Str(v.port_id.as_str()),
                1 => Str(v.value_kind.as_str()),
                2 => Str(match v.direction {
                    PortDirection::Input => "Input",
                    PortDirection::Output => "Output",
                }),
                3 => Temporal(v.temporal),
                4 => Str(v.abnormal_kind.as_ref().ok_or_else(bad)?.as_str()),
                _ => return Err(bad()),
            },
            Temporal(PortTemporal::Flow { closes }) if i == 0 => Flow(closes),
            Flow(v) if i == 0 => Bool(v),
            Graph(v) => match i {
                0 => Str(v.source_document_id.as_str()),
                1 => Str(v.checked_plot_id.as_str()),
                2 => Str(v.expanded_plot_id.as_str()),
                3 => Str(&v.plot_name),
                4 => Fronts(&v.front_inputs),
                5 => Fronts(&v.front_outputs),
                6 => Compositions(&v.compositions),
                7 => Gears(&v.gears),
                8 => Cords(&v.cords),
                _ => return Err(bad()),
            },
            Front(v) => match i {
                0 => Str(&v.identity),
                1 => Descriptor(&v.descriptor),
                2 => v
                    .value_contract
                    .as_ref()
                    .map_or(Null, |c| Nested(nested::Nested::Contract(c))),
                _ => return Err(bad()),
            },
            Port(v) => match i {
                0 => Str(&v.identity),
                1 => Str(v.gear_id.as_str()),
                2 => Descriptor(&v.descriptor),
                3 => v
                    .value_contract
                    .as_ref()
                    .map_or(Null, |c| Nested(nested::Nested::Contract(c))),
                _ => return Err(bad()),
            },
            Composition(v) => match i {
                0 => Str(&v.identity),
                1 => Str(&v.gear_name),
                2 => Str(&v.back_name),
                3 => Str(v.checked_plot_id.as_str()),
                4 => Fronts(&v.inputs),
                5 => Fronts(&v.outputs),
                6 => Bindings(&v.input_bindings),
                7 => Bindings(&v.output_bindings),
                _ => return Err(bad()),
            },
            Binding(v) => match i {
                0 => Str(&v.front_port),
                1 => Str(&v.internal_port),
                _ => return Err(bad()),
            },
            Gear(v) => match i {
                0 => Str(&v.identity),
                1 => Str(v.gear_id.as_str()),
                2 => Str(v.kind_id.as_str()),
                3 => Str(v.kind_contract_revision.as_str()),
                4 => Str(&v.source_plot),
                5 => Strings(&v.plot_path),
                6 => Ports(&v.inputs),
                7 => Ports(&v.outputs),
                8 => Controls(&v.controls),
                _ => return Err(bad()),
            },
            Cord(v) => match i {
                0 => v.source_origin.as_ref().map_or(Null, Origin),
                1 => Str(&v.identity),
                2 => Str(&v.source_port),
                3 => Str(&v.sink_port),
                4 => Str(v.value_kind.as_str()),
                5 => Temporal(v.temporal),
                _ => return Err(bad()),
            },
            Origin(v) => match i {
                0 => Str(v.source_document_id.as_str()),
                1 => Str(v.checked_plot_id.as_str()),
                2 => Str(&v.source_plot),
                3 => Strings(&v.plot_path),
                4 => Span(&v.source_span),
                5 => Str(match v.origin {
                    conduit_plot::ExpandedConnectionOrigin::CordStages => "CordStages",
                    conduit_plot::ExpandedConnectionOrigin::RelationalOperand => {
                        "RelationalOperand"
                    }
                    conduit_plot::ExpandedConnectionOrigin::SemanticExpression => {
                        "SemanticExpression"
                    }
                }),
                _ => return Err(bad()),
            },
            Span(v) => match i {
                0 => U64(u64::try_from(v.start).map_err(|_| Refusal::Bounds)?),
                1 => U64(u64::try_from(v.end).map_err(|_| Refusal::Bounds)?),
                2 => U64(u64::try_from(v.line).map_err(|_| Refusal::Bounds)?),
                3 => U64(u64::try_from(v.column).map_err(|_| Refusal::Bounds)?),
                4 => U64(u64::try_from(v.end_line).map_err(|_| Refusal::Bounds)?),
                5 => U64(u64::try_from(v.end_column).map_err(|_| Refusal::Bounds)?),
                _ => return Err(bad()),
            },
            Fronts(v) => Front(v.get(i).ok_or_else(bad)?),
            Ports(v) => Port(v.get(i).ok_or_else(bad)?),
            Compositions(v) => Composition(v.get(i).ok_or_else(bad)?),
            Bindings(v) => Binding(v.get(i).ok_or_else(bad)?),
            Gears(v) => Gear(v.get(i).ok_or_else(bad)?),
            Cords(v) => Cord(v.get(i).ok_or_else(bad)?),
            Controls(v) => Nested(nested::Nested::Control(v.get(i).ok_or_else(bad)?)),
            Strings(v) => Str(v.get(i).ok_or_else(bad)?),
            _ => return Err(bad()),
        })
    }
}
impl Node<'_> {
    pub(super) fn child_work(self) -> u32 {
        match self {
            Self::Nested(Nested::Quantity(_)) => 1 + 2 * conduit_core::QUANTITY_ENCODED_LEN as u32,
            Self::Nested(Nested::Unit(_)) => 1 + 2 * conduit_core::UNIT_ENCODED_LEN as u32,
            _ => 1,
        }
    }
}
const _: () = assert!(
    super::MAX_DEPTH as u32 + 2 * conduit_core::QUANTITY_ENCODED_LEN as u32 + 64
        < super::RESOLVE_RESERVE
);
