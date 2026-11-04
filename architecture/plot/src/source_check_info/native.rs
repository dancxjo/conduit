//! Preparation-only projection into the existing checked S1 Native vocabulary.
//! These bytes describe checking; they cannot restore compiler owners or grants.
use super::*;
use crate::rust_binding::{PreparedNativeAdmission, PreparedNativeBounds, PreparedNativeProgress};
use conduit_core::{
    StructuredFieldValue, StructuredInfoType as Type, StructuredInfoTypeShape as Shape,
    StructuredInfoValue as Value,
};

type Result<T> = core::result::Result<T, SourceCheckNativeRefusal>;
#[derive(Debug)]
pub enum SourceCheckNativeRefusal {
    Schema,
    Value(conduit_core::StructuredInfoRefusal),
    Report(SourceCheckRefusal),
    Admission(crate::rust_binding::PreparedNativeRefusal),
    Bytes,
}
impl From<conduit_core::StructuredInfoRefusal> for SourceCheckNativeRefusal {
    fn from(value: conduit_core::StructuredInfoRefusal) -> Self {
        Self::Value(value)
    }
}
/// An authoring/service preparation object. Encoding allocates and executes
/// checked Native laws; it must not be called from a generic Play step. The
/// caller admits its preparation workspace independently of the output bound.
pub struct SourceCheckNativeSchema {
    document: crate::CheckedSyntaxDocument,
}
impl SourceCheckNativeSchema {
    pub fn prepare() -> Result<Self> {
        let document = crate::check_syntax_document(
            &parse_syntax_document(include_str!(
                "../../../../semantics/language/source_check_info.conduit"
            )),
            &StartupCatalog::new(),
        )
        .map_err(|_| SourceCheckNativeRefusal::Schema)?;
        Ok(Self { document })
    }
    pub fn encode_request(
        &self,
        request: &SourceCheckRequest,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>> {
        request
            .context
            .validate()
            .map_err(SourceCheckNativeRefusal::Report)?;
        request
            .limits
            .validate()
            .map_err(SourceCheckNativeRefusal::Report)?;
        self.encode("SourceCheckRequest", maximum_bytes, |ty| {
            record(ty, |name, ty| match name {
                "snapshot" => snapshot(ty, &request.snapshot),
                "context" => context(ty, &request.context),
                "entry" => text(ty, &request.entry),
                "limits" => limits(ty, request.limits),
                _ => Err(SourceCheckNativeRefusal::Schema),
            })
        })
    }
    pub fn encode_report(
        &self,
        report: &SourceCheckInfoV1,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>> {
        report
            .validate_bounds()
            .map_err(SourceCheckNativeRefusal::Report)?;
        self.encode("SourceCheckInfoV1", maximum_bytes, |ty| {
            record(ty, |name, ty| match name {
                "schema_version" => leaf(ty, &report.schema_version.to_le_bytes()),
                "snapshot" => snapshot(ty, &report.snapshot),
                "context" => context(ty, &report.context),
                "requested_entry" => text(ty, &report.requested_entry),
                "limits" => limits(ty, report.limits),
                "check" => case(
                    ty,
                    match report.check {
                        CheckOutcome::Accepted => "accepted",
                        CheckOutcome::Rejected => "rejected",
                    },
                    |ty| leaf(ty, &[]),
                ),
                "candidate" => record(ty, |name, ty| match name {
                    "checked_plot_id" => {
                        optional_text(ty, report.candidate.checked_plot_id.as_deref())
                    }
                    "expanded_plot_id" => {
                        optional_text(ty, report.candidate.expanded_plot_id.as_deref())
                    }
                    _ => Err(SourceCheckNativeRefusal::Schema),
                }),
                "diagnostics" => sequence(ty, &report.diagnostics, diagnostic),
                "dependencies" => dependencies(ty, &report.dependencies),
                "plan" | "play" => case(ty, "absent", |ty| leaf(ty, &[])),
                _ => Err(SourceCheckNativeRefusal::Schema),
            })
        })
    }
    fn encode(
        &self,
        name: &str,
        maximum_bytes: usize,
        value: impl FnOnce(&Type) -> Result<Value>,
    ) -> Result<Vec<u8>> {
        if maximum_bytes == 0 || maximum_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(SourceCheckNativeRefusal::Bytes);
        }
        let native = self
            .document
            .native_types
            .iter()
            .find(|n| n.name == name)
            .ok_or(SourceCheckNativeRefusal::Schema)?;
        let bytes = value(&native.value_type)?.canonical_bytes()?;
        if bytes.len() > maximum_bytes {
            return Err(SourceCheckNativeRefusal::Bytes);
        }
        let mut admission = PreparedNativeAdmission::new(
            &self.document,
            &native.identity,
            PreparedNativeBounds {
                canonical_bytes: maximum_bytes,
                scopes: 4096,
                invariants: 4096,
                contracts: 4096,
            },
        )
        .map_err(SourceCheckNativeRefusal::Admission)?;
        admission
            .begin(&bytes)
            .map_err(SourceCheckNativeRefusal::Admission)?;
        while admission
            .step()
            .map_err(SourceCheckNativeRefusal::Admission)?
            != PreparedNativeProgress::Admitted
        {}
        Ok(bytes)
    }
}
fn leaf(ty: &Type, bytes: &[u8]) -> Result<Value> {
    if let Shape::Nominal { representation, .. } = ty.shape() {
        Ok(Value::nominal(ty.clone(), leaf(representation, bytes)?)?)
    } else {
        Ok(Value::leaf(ty.clone(), bytes.to_vec())?)
    }
}
fn text(ty: &Type, text: &str) -> Result<Value> {
    leaf(ty, text.as_bytes())
}
fn record(ty: &Type, build: impl Fn(&str, &Type) -> Result<Value>) -> Result<Value> {
    let Shape::Record { fields, .. } = ty.shape() else {
        return Err(SourceCheckNativeRefusal::Schema);
    };
    Ok(Value::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                Ok(StructuredFieldValue::new(
                    field.name(),
                    build(field.name(), field.value_type())?,
                )?)
            })
            .collect::<Result<Vec<_>>>()?,
    )?)
}
fn case(ty: &Type, tag: &str, build: impl FnOnce(&Type) -> Result<Value>) -> Result<Value> {
    let Shape::Variant { cases, .. } = ty.shape() else {
        return Err(SourceCheckNativeRefusal::Schema);
    };
    let payload = cases
        .iter()
        .find(|c| c.tag() == tag)
        .ok_or(SourceCheckNativeRefusal::Schema)?;
    Ok(Value::variant(
        ty.clone(),
        tag,
        build(payload.payload_type())?,
    )?)
}
fn optional_text(ty: &Type, value: Option<&str>) -> Result<Value> {
    match value {
        Some(value) => case(ty, "some", |ty| text(ty, value)),
        None => case(ty, "none", |ty| leaf(ty, &[])),
    }
}
fn sequence<T>(
    ty: &Type,
    items: &[T],
    build: impl Fn(&Type, &T) -> Result<Value>,
) -> Result<Value> {
    let Shape::Sequence { element, .. } = ty.shape() else {
        return Err(SourceCheckNativeRefusal::Schema);
    };
    Ok(Value::sequence(
        ty.clone(),
        items
            .iter()
            .map(|item| build(element, item))
            .collect::<Result<Vec<_>>>()?,
    )?)
}
fn snapshot(ty: &Type, source: &SourceSnapshotRevision) -> Result<Value> {
    record(ty, |name, ty| match name {
        "owner" => text(ty, &source.owner),
        "generation" => leaf(ty, &source.generation.to_le_bytes()),
        "sha256" => text(ty, &source.sha256),
        "source_document_id" => text(ty, &source.source_document_id),
        _ => Err(SourceCheckNativeRefusal::Schema),
    })
}
fn context(ty: &Type, context: &CheckContextRevision) -> Result<Value> {
    record(ty, |name, ty| {
        text(
            ty,
            match name {
                "grammar_edition" => &context.grammar_edition,
                "catalog" => &context.catalog,
                "kinds_fores" => &context.kinds_fores,
                "types_laws" => &context.types_laws,
                "backs" => &context.backs,
                _ => return Err(SourceCheckNativeRefusal::Schema),
            },
        )
    })
}
fn u32_value(ty: &Type, value: usize) -> Result<Value> {
    let value = u32::try_from(value).map_err(|_| SourceCheckNativeRefusal::Bytes)?;
    leaf(ty, &value.to_le_bytes())
}
fn limits(ty: &Type, limits: SourceCheckLimits) -> Result<Value> {
    record(ty, |name, ty| {
        u32_value(
            ty,
            match name {
                "diagnostics" => limits.diagnostics,
                "diagnostic_bytes" => limits.diagnostic_bytes,
                "dependencies" => limits.dependencies,
                "dependency_bytes" => limits.dependency_bytes,
                _ => return Err(SourceCheckNativeRefusal::Schema),
            },
        )
    })
}
fn diagnostic(ty: &Type, diagnostic: &SourceCheckDiagnostic) -> Result<Value> {
    record(ty, |name, ty| match name {
        "code" => text(ty, &diagnostic.code),
        "summary" => text(ty, &diagnostic.summary),
        "source_document_id" => text(ty, &diagnostic.source_document_id),
        "content_hash" => text(ty, &diagnostic.content_hash),
        "primary_span" => match &diagnostic.primary_span {
            None => case(ty, "none", |ty| leaf(ty, &[])),
            Some(span) => case(ty, "some", |ty| {
                record(ty, |name, ty| {
                    u32_value(
                        ty,
                        match name {
                            "start" => span.start,
                            "end" => span.end,
                            "line" => span.line,
                            "column" => span.column,
                            "end_line" => span.end_line,
                            "end_column" => span.end_column,
                            _ => return Err(SourceCheckNativeRefusal::Schema),
                        },
                    )
                })
            }),
        },
        _ => Err(SourceCheckNativeRefusal::Schema),
    })
}
fn dependencies(ty: &Type, dependencies: &DependencyProvenance) -> Result<Value> {
    match dependencies {
        DependencyProvenance::WholeContext => case(ty, "whole_context", |ty| leaf(ty, &[])),
        DependencyProvenance::Observed {
            checking,
            expansion,
        } => case(ty, "observed", |ty| {
            record(ty, |name, ty| {
                let reads = match name {
                    "checking" => checking,
                    "expansion" => expansion,
                    _ => return Err(SourceCheckNativeRefusal::Schema),
                };
                sequence(ty, reads, |ty, read| {
                    record(ty, |name, ty| {
                        text(
                            ty,
                            match name {
                                "table" => &read.table,
                                "key" => &read.key,
                                _ => return Err(SourceCheckNativeRefusal::Schema),
                            },
                        )
                    })
                })
            })
        }),
    }
}
