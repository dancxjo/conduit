//! Preparation-only projection into the existing checked S1 Native vocabulary.
//! These bytes describe checking; they cannot restore compiler owners or grants.
use super::*;
use crate::parse_syntax_document;
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
/// Typed outcome of an immutable Source-check request.
pub enum SourceCheckReply<'a> {
    Checked(&'a SourceCheckInfoV1),
    Refused(SourceCheckRefusal),
    NotPrepared,
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
    /// Populate a staged authoring catalogue from the actual retained Native
    /// owners, including the whole declared Type closure. Names are not proofs.
    pub fn install_types(&self, startup: &mut StartupCatalog) -> Result<()> {
        let mut staged = startup.clone();
        for native in &self.document.native_types {
            let semantic = native.value_type.canonical_bytes()?;
            let owner = crate::rust_binding::check_prepared_decode_native(
                &self.document,
                native.identity.as_str(),
                &semantic,
                &native.value_contracts,
                &native.invariants,
            )
            .map_err(|_| SourceCheckNativeRefusal::Schema)?;
            staged
                .insert_checked_native_type(native.name.clone(), owner)
                .map_err(|_| SourceCheckNativeRefusal::Schema)?;
        }
        *startup = staged;
        Ok(())
    }
    pub fn request_kind(&self) -> Result<conduit_core::KindId> {
        self.port_kind("SourceCheckRequest")
    }
    pub fn reply_kind(&self) -> Result<conduit_core::KindId> {
        self.port_kind("SourceCheckReply")
    }
    pub fn report_kind(&self) -> Result<conduit_core::KindId> {
        self.port_kind("SourceCheckInfoV1")
    }
    fn port_kind(&self, name: &str) -> Result<conduit_core::KindId> {
        self.document
            .native_types
            .iter()
            .find(|native| native.name == name)
            .ok_or(SourceCheckNativeRefusal::Schema)?
            .value_type
            .profile()
            .map(|profile| profile.value_kind().clone())
            .map_err(SourceCheckNativeRefusal::Value)
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
            report_value(ty, report)
        })
    }
    pub fn encode_reply(
        &self,
        reply: SourceCheckReply<'_>,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>> {
        if let SourceCheckReply::Checked(report) = &reply {
            report
                .validate_bounds()
                .map_err(SourceCheckNativeRefusal::Report)?;
        }
        self.encode("SourceCheckReply", maximum_bytes, |ty| match reply {
            SourceCheckReply::Checked(report) => case(ty, "checked", |ty| report_value(ty, report)),
            SourceCheckReply::NotPrepared => case(ty, "not_prepared", |ty| leaf(ty, &[])),
            SourceCheckReply::Refused(refusal) => case(ty, "refused", |ty| {
                case(
                    ty,
                    match refusal {
                        SourceCheckRefusal::InvalidSnapshot => "invalid_snapshot",
                        SourceCheckRefusal::SourceBytes => "source_bytes",
                        SourceCheckRefusal::StaleSource => "stale_source",
                        SourceCheckRefusal::UnknownGrammar => "unknown_grammar",
                        SourceCheckRefusal::UnknownContext => "unknown_context",
                        SourceCheckRefusal::EntryBytes => "entry_bytes",
                        SourceCheckRefusal::Limits => "limits",
                        SourceCheckRefusal::InvalidDiagnosticSpan => "invalid_diagnostic_span",
                        SourceCheckRefusal::DiagnosticOverflow => "diagnostic_overflow",
                        SourceCheckRefusal::DependencyOverflow => "dependency_overflow",
                    },
                    |ty| leaf(ty, &[]),
                )
            }),
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
        self.admit_bytes(name, &bytes, maximum_bytes)?;
        Ok(bytes)
    }
    /// Preparation-only validation against the retained exact S1 declaration,
    /// not a schema or law vector supplied by the returned bytes.
    pub fn admit_reply(&self, bytes: &[u8], maximum_bytes: usize) -> Result<()> {
        self.admit_bytes("SourceCheckReply", bytes, maximum_bytes)
    }
    fn admit_bytes(&self, name: &str, bytes: &[u8], maximum_bytes: usize) -> Result<()> {
        if maximum_bytes == 0
            || maximum_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || bytes.len() > maximum_bytes
        {
            return Err(SourceCheckNativeRefusal::Bytes);
        }
        let native = self
            .document
            .native_types
            .iter()
            .find(|native| native.name == name)
            .ok_or(SourceCheckNativeRefusal::Schema)?;
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
            .begin(bytes)
            .map_err(SourceCheckNativeRefusal::Admission)?;
        while admission
            .step()
            .map_err(SourceCheckNativeRefusal::Admission)?
            != PreparedNativeProgress::Admitted
        {}
        Ok(())
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

fn report_value(ty: &Type, report: &SourceCheckInfoV1) -> Result<Value> {
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
            "checked_plot_id" => optional_text(ty, report.candidate.checked_plot_id.as_deref()),
            "expanded_plot_id" => optional_text(ty, report.candidate.expanded_plot_id.as_deref()),
            _ => Err(SourceCheckNativeRefusal::Schema),
        }),
        "diagnostics" => sequence(ty, &report.diagnostics, diagnostic),
        "dependencies" => dependencies(ty, &report.dependencies),
        "plan" | "play" => case(ty, "absent", |ty| leaf(ty, &[])),
        _ => Err(SourceCheckNativeRefusal::Schema),
    })
}
