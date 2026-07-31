use conduit_core::{
    CanonicalDescriptor, CanonicalValue, FieldDisposition, Id, MapField, TypeContractRef,
};

use crate::{
    FORMAT_MAX_NAME_BYTES, FORMAT_MAX_SCALAR_BYTES, FORMAT_MAX_VALUES,
    FORMAT_VALUES_MAX_ENCODED_BYTES,
};

/// Broad semantic family of a standard type definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardTypeFamily {
    Fundamental,
    Structural,
    TimeAndIdentity,
    Operational,
    Network,
    Filesystem,
    Process,
    Cryptography,
}

/// Representation semantics of a type definition.
///
/// This describes meaning; it is not a claim that a host has a representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardRepresentation {
    PortableScalar,
    Mathematical,
    FixedWidth { bits: u16, signed: bool },
    Structural,
    Domain,
}

/// One standard type or generic type constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StandardTypeDefinition {
    pub id: Id<'static>,
    pub human_name: &'static str,
    pub family: StandardTypeFamily,
    /// Number of type arguments. Zero denotes a concrete type contract.
    pub parameters: u8,
    pub representation: StandardRepresentation,
}

/// A host/provider claim about one concrete type representation.
///
/// This is deliberately separate from [`STANDARD_TYPE_CATALOG`]. A definition
/// is universally discoverable; support, limits, authority, and placement are
/// host observations made elsewhere.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeRepresentationSupport<'a> {
    pub type_id: Id<'a>,
    /// Finite signed magnitude supported for mathematical integer types.
    pub maximum_integer_bits: Option<u16>,
    pub maximum_value_bytes: u64,
    pub maximum_collection_items: u32,
}

macro_rules! concrete {
    ($id:literal, $name:literal, $family:ident, $representation:expr) => {
        StandardTypeDefinition {
            id: Id($id),
            human_name: $name,
            family: StandardTypeFamily::$family,
            parameters: 0,
            representation: $representation,
        }
    };
}

macro_rules! constructor {
    ($id:literal, $name:literal, $parameters:literal) => {
        StandardTypeDefinition {
            id: Id($id),
            human_name: $name,
            family: StandardTypeFamily::Structural,
            parameters: $parameters,
            representation: StandardRepresentation::Structural,
        }
    };
}

/// Allocator-free standard type universe. Generic entries name constructors;
/// a concrete specialization receives its own exact descriptor.
pub static STANDARD_TYPE_CATALOG: &[StandardTypeDefinition] = &[
    concrete!(
        "std/unit",
        "unit",
        Fundamental,
        StandardRepresentation::PortableScalar
    ),
    concrete!(
        "std/bool",
        "boolean",
        Fundamental,
        StandardRepresentation::PortableScalar
    ),
    concrete!(
        "std/integer",
        "mathematical signed integer",
        Fundamental,
        StandardRepresentation::Mathematical
    ),
    concrete!(
        "std/natural",
        "mathematical nonnegative integer",
        Fundamental,
        StandardRepresentation::Mathematical
    ),
    concrete!(
        "std/float",
        "floating-point number",
        Fundamental,
        StandardRepresentation::Domain
    ),
    concrete!(
        "std/decimal",
        "exact decimal number",
        Fundamental,
        StandardRepresentation::Mathematical
    ),
    concrete!(
        "std/text",
        "Unicode text",
        Fundamental,
        StandardRepresentation::Domain
    ),
    concrete!(
        "std/format-values",
        "bounded ordered and named formatter values",
        Structural,
        StandardRepresentation::Structural
    ),
    concrete!(
        "std/bytes",
        "byte sequence",
        Fundamental,
        StandardRepresentation::Structural
    ),
    concrete!(
        "std/i8",
        "signed 8-bit integer",
        Fundamental,
        StandardRepresentation::FixedWidth {
            bits: 8,
            signed: true
        }
    ),
    concrete!(
        "std/i16",
        "signed 16-bit integer",
        Fundamental,
        StandardRepresentation::FixedWidth {
            bits: 16,
            signed: true
        }
    ),
    concrete!(
        "std/i32",
        "signed 32-bit integer",
        Fundamental,
        StandardRepresentation::FixedWidth {
            bits: 32,
            signed: true
        }
    ),
    concrete!(
        "std/i64",
        "signed 64-bit integer",
        Fundamental,
        StandardRepresentation::FixedWidth {
            bits: 64,
            signed: true
        }
    ),
    concrete!(
        "std/i128",
        "signed 128-bit integer",
        Fundamental,
        StandardRepresentation::FixedWidth {
            bits: 128,
            signed: true
        }
    ),
    concrete!(
        "std/u8",
        "unsigned 8-bit integer",
        Fundamental,
        StandardRepresentation::FixedWidth {
            bits: 8,
            signed: false
        }
    ),
    concrete!(
        "std/u16",
        "unsigned 16-bit integer",
        Fundamental,
        StandardRepresentation::FixedWidth {
            bits: 16,
            signed: false
        }
    ),
    concrete!(
        "std/u32",
        "unsigned 32-bit integer",
        Fundamental,
        StandardRepresentation::FixedWidth {
            bits: 32,
            signed: false
        }
    ),
    concrete!(
        "std/u64",
        "unsigned 64-bit integer",
        Fundamental,
        StandardRepresentation::FixedWidth {
            bits: 64,
            signed: false
        }
    ),
    concrete!(
        "std/u128",
        "unsigned 128-bit integer",
        Fundamental,
        StandardRepresentation::FixedWidth {
            bits: 128,
            signed: false
        }
    ),
    constructor!("std/option", "optional value", 1),
    constructor!("std/result", "success or error", 2),
    constructor!("std/list", "finite list", 1),
    concrete!(
        "std/list/text",
        "finite list of Unicode text",
        Structural,
        StandardRepresentation::Structural
    ),
    constructor!("std/map", "finite key-value map", 2),
    concrete!(
        "std/record",
        "named-field record",
        Structural,
        StandardRepresentation::Structural
    ),
    concrete!(
        "std/variant",
        "tagged variant",
        Structural,
        StandardRepresentation::Structural
    ),
    constructor!("std/reference", "typed reference", 1),
    concrete!(
        "std/reference/any",
        "kind-erased semantic reference",
        Structural,
        StandardRepresentation::Domain
    ),
    concrete!(
        "std/duration",
        "duration",
        TimeAndIdentity,
        StandardRepresentation::Domain
    ),
    concrete!(
        "std/instant",
        "monotonic instant",
        TimeAndIdentity,
        StandardRepresentation::Domain
    ),
    concrete!(
        "std/timestamp",
        "civil timestamp",
        TimeAndIdentity,
        StandardRepresentation::Domain
    ),
    concrete!(
        "std/id",
        "stable identifier",
        TimeAndIdentity,
        StandardRepresentation::Domain
    ),
    concrete!(
        "std/error",
        "error",
        Operational,
        StandardRepresentation::Structural
    ),
    concrete!(
        "std/terminal",
        "terminal observation",
        Operational,
        StandardRepresentation::Structural
    ),
    concrete!(
        "std/health",
        "health observation",
        Operational,
        StandardRepresentation::Structural
    ),
    concrete!(
        "std/progress",
        "bounded progress observation",
        Operational,
        StandardRepresentation::Structural
    ),
    concrete!(
        "std/validation-decision",
        "typed structural validation decision",
        Operational,
        StandardRepresentation::Structural
    ),
    concrete!(
        "supervision/decision",
        "supervision decision",
        Operational,
        StandardRepresentation::Structural
    ),
    concrete!(
        "net/ip/address",
        "IP address",
        Network,
        StandardRepresentation::Domain
    ),
    concrete!(
        "net/socket/address",
        "socket address",
        Network,
        StandardRepresentation::Domain
    ),
    concrete!(
        "net/http/method",
        "HTTP method",
        Network,
        StandardRepresentation::Domain
    ),
    concrete!(
        "net/http/request",
        "bounded HTTP request",
        Network,
        StandardRepresentation::Structural
    ),
    concrete!(
        "net/http/response",
        "bounded HTTP response",
        Network,
        StandardRepresentation::Structural
    ),
    concrete!(
        "net/http/status",
        "HTTP status",
        Network,
        StandardRepresentation::Domain
    ),
    concrete!(
        "net/http/headers",
        "bounded HTTP headers",
        Network,
        StandardRepresentation::Structural
    ),
    concrete!(
        "fs/path",
        "filesystem path",
        Filesystem,
        StandardRepresentation::Domain
    ),
    concrete!(
        "process/exit-status",
        "process exit status",
        Process,
        StandardRepresentation::Domain
    ),
    concrete!(
        "crypto/digest",
        "cryptographic digest",
        Cryptography,
        StandardRepresentation::Domain
    ),
];

const fn semantic_field(name: &'static str, value: CanonicalValue<'static>) -> MapField<'static> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

static TEXT_DESCRIPTOR_FIELDS: &[MapField<'static>] = &[
    semantic_field("encoding", CanonicalValue::Identifier(Id("utf-8"))),
    semantic_field(
        "value",
        CanonicalValue::Identifier(Id("unicode-scalar-sequence")),
    ),
    semantic_field(
        "normalization",
        CanonicalValue::Identifier(Id("none-required")),
    ),
    semantic_field("invalid_encoding", CanonicalValue::Identifier(Id("reject"))),
    semantic_field(
        "byte_bound",
        CanonicalValue::Identifier(Id("plan-value-envelope")),
    ),
];

static INTEGER_DESCRIPTOR_FIELDS: &[MapField<'static>] = &[
    semantic_field(
        "domain",
        CanonicalValue::Identifier(Id("mathematical-signed-integer")),
    ),
    semantic_field("range", CanonicalValue::Identifier(Id("unbounded"))),
    semantic_field(
        "canonical_encoding",
        CanonicalValue::Identifier(Id("minimal-twos-complement-big-endian")),
    ),
    semantic_field(
        "representation_bound",
        CanonicalValue::Identifier(Id("host-and-plan-explicit")),
    ),
    semantic_field(
        "overflow",
        CanonicalValue::Identifier(Id("typed-terminal-no-truncation")),
    ),
];

static FORMAT_VALUE_KINDS: &[CanonicalValue<'static>] = &[
    CanonicalValue::Identifier(Id("std/text")),
    CanonicalValue::Identifier(Id("std/bool")),
    CanonicalValue::Identifier(Id("std/integer")),
];

static FORMAT_VALUES_DESCRIPTOR_FIELDS: &[MapField<'static>] = &[
    semantic_field(
        "collection",
        CanonicalValue::Identifier(Id("ordered-optional-unique-names")),
    ),
    semantic_field(
        "maximum_values",
        CanonicalValue::Integer(FORMAT_MAX_VALUES as i128),
    ),
    semantic_field(
        "maximum_name_bytes",
        CanonicalValue::Integer(FORMAT_MAX_NAME_BYTES as i128),
    ),
    semantic_field(
        "maximum_scalar_bytes",
        CanonicalValue::Integer(FORMAT_MAX_SCALAR_BYTES as i128),
    ),
    semantic_field(
        "maximum_encoded_bytes",
        CanonicalValue::Integer(FORMAT_VALUES_MAX_ENCODED_BYTES as i128),
    ),
    semantic_field("supported_kinds", CanonicalValue::Set(FORMAT_VALUE_KINDS)),
    semantic_field(
        "encoding",
        CanonicalValue::Identifier(Id("conduit-format-values")),
    ),
    semantic_field(
        "unsupported_kind",
        CanonicalValue::Identifier(Id("typed-terminal")),
    ),
];

/// Returns the exact host-language-neutral descriptor for one type definition.
#[must_use]
pub fn standard_type_descriptor(
    definition: &StandardTypeDefinition,
) -> CanonicalDescriptor<'static> {
    let body = match definition.id.as_str() {
        "std/text" => CanonicalValue::Map(TEXT_DESCRIPTOR_FIELDS),
        "std/integer" => CanonicalValue::Map(INTEGER_DESCRIPTOR_FIELDS),
        "std/format-values" => CanonicalValue::Map(FORMAT_VALUES_DESCRIPTOR_FIELDS),
        _ => CanonicalValue::Null,
    };
    CanonicalDescriptor {
        kind: definition.id,
        schema_version: 0,
        body,
    }
}

/// Finds a standard type definition by exact catalog path.
#[must_use]
pub fn standard_type(id: &str) -> Option<&'static StandardTypeDefinition> {
    STANDARD_TYPE_CATALOG
        .iter()
        .find(|definition| definition.id.as_str() == id)
}

/// Constructs the exact reference for a concrete standard type.
///
/// Constructors require type arguments and therefore do not have a concrete
/// reference by themselves.
#[must_use]
pub fn standard_type_reference(id: &str) -> Option<TypeContractRef<'static>> {
    let definition = standard_type(id)?;
    if definition.parameters != 0 {
        return None;
    }
    let descriptor = standard_type_descriptor(definition);
    Some(TypeContractRef {
        contract_id: definition.id,
        schema_version: 0,
        semantic_hash: descriptor
            .semantic_hash()
            .expect("standard type descriptor is canonical"),
    })
}
